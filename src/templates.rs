use anyhow::{ensure, Context, Result};
use askama_axum::Template;
use serde::Serialize;
use serde_json::Value;

// The `alpha-g-chronobox-timestamps`'s CSV output has a `board` column that is
// a human-readable identifier for a specific chronobox board.
// It is our responsibility (too high-level to be reasonable to expect this from
// e.g. the `alpha-g-detector` crate) to map this "human-readable" identifier to
// whatever is needed to query the ODB.
//
// The human-readable identifiers are guaranteed to not change within
// semver-compatible versions of `alpha-g-analysis`.
const BOARDS: [&str; 4] = ["cb01", "cb02", "cb03", "cb04"];
// These can be used to get the ODB channels
// (array of names with index as channel number):
// `/Equipment/<board>/Settings/names`

#[derive(Clone, Debug, Serialize)]
struct ChronoboxChannel {
    board: String,
    number: u8,
    description: String,
}

#[derive(Template)]
#[template(path = "run_info.html")]
pub(super) struct RunInfoTemplate {
    run_number: u32,
    start_time: String,
    stop_time: String,
    operator_comment: String,
    // These are in the order in which they are displayed in the template.
    // It is easier to sort them here however we want instead of fighting with
    // HTML/JS and e.g. tom-select.
    cb_channels: Vec<ChronoboxChannel>,
}
impl RunInfoTemplate {
    // askama doesn't support closures or mutable variables in templates. So it
    // is just easier to delegate any "complex" logic to a method here.
    //
    // This method is used to get the label for a channel at a specific index.
    // Return `None` whenever we don't want to display a channel in the
    // selection (and the template will just skip it).
    //
    // Current implementation hides channels that have the same name. These
    // usually don't have anything connected and they just clutter the
    // selection.
    // If we ever want to display them, we can return them as e.g.
    // `description (board)`
    // instead of `None` to distinguish repetitions.
    fn cb_label_at(&self, i: &usize) -> Option<String> {
        let channel = self.cb_channels.get(*i)?;

        if self
            .cb_channels
            .iter()
            .filter(|c| c.description.eq_ignore_ascii_case(&channel.description))
            .count()
            > 1
        {
            None
        } else {
            Some(channel.description.clone())
        }
    }
}

impl RunInfoTemplate {
    pub(super) fn try_from_odb(odb: &Value) -> Result<Self> {
        let run_number = odb
            .pointer("/Runinfo/Run number")
            .and_then(Value::as_u64)
            .context("failed to get run number")?
            .try_into()
            .context("failed to convert run number to u32")?;
        let start_time = odb
            .pointer("/Runinfo/Start time binary")
            .and_then(Value::as_str)
            .and_then(|s| s.strip_prefix("0x"))
            .context("failed to get binary start time")
            .and_then(|s| i64::from_str_radix(s, 16).map_err(|e| anyhow::anyhow!(e)))
            .context("failed to parse start time as u64")?;
        let stop_time = odb
            .pointer("/Runinfo/Stop time binary")
            .and_then(Value::as_str)
            .and_then(|s| s.strip_prefix("0x"))
            .context("failed to get binary stop time")
            .and_then(|s| i64::from_str_radix(s, 16).map_err(|e| anyhow::anyhow!(e)))
            .context("failed to parse stop time as u64")?;
        ensure!(start_time < stop_time, "start time after stop time");
        let operator_comment = odb
            .pointer("/Experiment/Edit on start/Comment")
            .and_then(Value::as_str)
            .context("failed to get comment")?
            .to_string();

        let start_time = time::OffsetDateTime::from_unix_timestamp(start_time)
            .context("failed to convert start time to `OffsetDateTime`")?
            .format(&time::format_description::well_known::Rfc2822)
            .context("failed to format start time")?;
        let stop_time = time::OffsetDateTime::from_unix_timestamp(stop_time)
            .context("failed to convert stop time to `OffsetDateTime`")?
            .format(&time::format_description::well_known::Rfc2822)
            .context("failed to format stop time")?;

        let mut cb_channels = Vec::new();
        for board in BOARDS {
            let names = odb
                .pointer(&format!("/Equipment/{board}/Settings/names"))
                .and_then(Value::as_array)
                .with_context(|| format!("failed to get channels array for `{board}`"))?;
            for (i, name) in names.iter().enumerate() {
                cb_channels.push(ChronoboxChannel {
                    board: board.to_string(),
                    number: u8::try_from(i).with_context(|| {
                        format!("failed to convert channel number `{i}` to u8 for `{board}`")
                    })?,
                    description: name
                        .as_str()
                        .with_context(|| {
                            format!("failed to get channel name for `{board}` at `{i}`")
                        })?
                        .to_string(),
                });
            }
        }
        cb_channels.sort_by(|a, b| {
            a.description
                .to_lowercase()
                .cmp(&b.description.to_lowercase())
        });

        Ok(Self {
            run_number,
            start_time,
            stop_time,
            operator_comment,
            cb_channels,
        })
    }
}
