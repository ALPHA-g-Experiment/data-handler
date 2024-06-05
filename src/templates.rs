use anyhow::{ensure, Context, Result};
use askama_axum::Template;
use serde_json::Value;

#[derive(Template)]
#[template(path = "run_info.html")]
pub(super) struct RunInfoTemplate {
    run_number: u32,
    start_time: String,
    stop_time: String,
    operator_comment: String,
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

        Ok(Self {
            run_number,
            start_time,
            stop_time,
            operator_comment,
        })
    }
}
