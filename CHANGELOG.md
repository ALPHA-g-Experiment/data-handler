# Changelog

Note that this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html) and all notable
changes will be documented in this file.

<!-- next-header -->

## [Unreleased] - ReleaseDate

### Added

- The `Sequencer Events` CSV download was changed to a full `Spill Log` CSV.
  This contains all the information from the sequencer events, but also includes
  the counts during each spill for all Chronobox channels.
- Download tabs now have a small spinner to indicate that processing is ongoing.
- It is now possible to specify (via the `--pattern` flag) a filename pattern
  regex for the MIDAS files given a run number. This is useful in the case that
  there are both `*.mid` and `*.mid.lz4` files in the data directory. The
  default pattern will match any `runXXXXXsubYYY.mid*` file. To, for example,
  match only `*.mid` files, add a `$` anchor to the end of the default pattern.

## [0.1.1] - 2024-08-24

### Added

- The client now displays the name of the binaries (instead of internal debug
  implementation name) when it is giving updates on requested messages.
- Added a `--version` flag to the CLI.

### Fixed

- Keep websocket connection alive when the server is running behind a reverse
  proxy by sending a periodic "ping" message.
- Retry to load run information when the latest run file was not on EOS yet.

## [0.1.0] - 2024-08-23

First usable version of the data handler.

<!-- next-url -->
[Unreleased]: https://github.com/ALPHA-g-Experiment/data-handler/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/ALPHA-g-Experiment/data-handler/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/ALPHA-g-Experiment/data-handler/compare/5ab78a7...v0.1.0
