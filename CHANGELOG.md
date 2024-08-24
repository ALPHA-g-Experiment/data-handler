# Changelog

Note that this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html) and all notable
changes will be documented in this file.

<!-- next-header -->

## [Unreleased] - ReleaseDate

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
[Unreleased]: https://github.com/ALPHA-g-Experiment/data-handler/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ALPHA-g-Experiment/data-handler/compare/5ab78a7...v0.1.0
