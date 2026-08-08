# paparse

Extract basic track stats from [Poweramp](https://powerampapp.com/) into CSV or JSON.

Poweramp allows users to export some data and settings to a `.poweramp_backup` file. This tool can be used for extracting the file path, title and play count for every track recorded. *under some circumstances*, specifically if your file names follow the `<artist> - <title>` format, you can use the `--structured` flag to extract title, artist and play count instead.
The limited set of data points is because of the data provided in the files.

## Install

### Prebuilt release

Grab the latest binary for your platform from the [Releases page](https://github.com/AivChn/paparse/releases) and put it somewhere on your `PATH`.

### Build from source

```bash
git clone https://github.com/AivChn/paparse.git
cd paparse
cargo build --release
```

The binary will be at `target/release/paparse`.

## Usage

```
paparse [OPTIONS] <FILE>
```

| Argument | Description |
|---|---|
| `<FILE>` | Path to a Poweramp `.db` file or `.poweramp_backup` archive |
| `-o, --output <PATH>` | Output file name. Defaults to the input file's name |
| `--structured` | Split the track name into `artist` and `title` instead of a single `name` field. Only works if your files are named `[artist] - [title]` |

### Examples

Extract a backup to CSV (default), using the backup's name for the output file:

```bash
paparse stats.poweramp_backup
# -> stats.csv
```


Extract to a specific output file:

```bash
paparse stats.poweramp_backup -o my_export.json
# -> my_export.json
```

Split artist and title (works if your files are named `<artist> - <title>.ext`):

```bash
paparse stats.poweramp_backup --structured
# -> stats.csv, with path, title, artist, listen_count columns
```

## Output

By default, each row has three fields, taken as is from the db file:

| Field | Description |
|---|---|
| `path` | Full path to the track file |
| `name` | column `readable_name`, usually is equivalent to track title |
| `listen_count` | Number of times the track has been played |

With `--structured`, the parser will try to extract the artist from the path, assuming `<artist> - <title>.ext`. Giving you `path`, `artist`, `title`, `listen_count` instead. If parsing the path fails, artist will be empty.
