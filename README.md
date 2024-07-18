# ALPHA-g-Data-Handler

[![Build Status](https://github.com/ALPHA-g-Experiment/data-handler/actions/workflows/build.yml/badge.svg)](https://github.com/ALPHA-g-Experiment/data-handler/actions/workflows/build.yml)

Web application for the ALPHA-g experiment. This is a UI wrapper for the
[`alpha-g-analysis`](https://github.com/ALPHA-g-Experiment/alpha-g/tree/main/analysis)
package and the
[`analysis-scripts`](https://github.com/ALPHA-g-Experiment/analysis-scripts/tree/main)
repository.

## Installation

Before you start, make sure you have the following installed on your system:
Python 3.9+,
[`cargo`](https://doc.rust-lang.org/cargo/getting-started/installation.html),
and the `git` command.

Install the application using the following commands:

```bash
cargo install --git https://github.com/ALPHA-g-Experiment/data-handler.git
alpha-g-data-handler update
```

Finally, you can start the server as (use `--help` to see all the available
options):

```bash
AG_JWT_SECRET=secret alpha-g-data-handler serve -a 0.0.0.0:8080 -d /path/to/midas/files
```

### Note

To allow file downloads from the server you need to set the `AG_JWT_SECRET`
environment variable. Additionally, the application will manage the following
directories for you (might vary depending on OS):

- `$HOME/.alpha-g-data-handler`: Contains the internally-managed
  `alpha-g-analysis` and `analysis-scripts`. Please do not modify this directory
  manually (only use `alpha-g-data-handler update` if you want to update these
  packages).

- `$HOME/.cache/alpha-g-data-handler`: Contains the cached data files. This
  directory can be safely deleted if you want to clear the cache.

- `/tmp/alpha-g-data-handler`: Contains the temporary files generated by the
  application. This directory can be safely deleted if you want to clear the
  temporary files.
