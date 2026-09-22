<p align="center">
  <img alt="Cassandra by Evelyn De Morgan" src="assets/Evelyn_de_Morgan-Cassandra-crop.jpg">
</p>

<h1 align="center">Cassandra</h1>

A fast, Rust-based CLI tool for reading Linux system logs. Supports multiple log formats and sources with flexible output options.

## Features

- **Multiple log sources**: `journald`, `auth.log`/`secure`, `syslog`/`messages`, `wtmp`
- **Flexible output**: standard table, compact, summary columns, or key/value pairs
- **Cross-distro**: auto-detects common log paths (Debian/Ubuntu, RHEL/Fedora, etc.)
- **No root required**: use `cap.sh` to grant read access to protected logs
- **Built with Rust 2024 edition**

## Supported Log Sources

| Command | Description | Typical Paths |
|---------|-------------|---------------|
| `sys` | System logs (RFC 3164) | `/var/log/syslog`, `/var/log/messages` |
| `auth` | Authentication logs | `/var/log/auth.log`, `/var/log/secure` |
| `wtmp` | Login records (binary) | `/var/log/wtmp` |
| `journal` | systemd journal | via `systemd-journald` socket |

> **Note**: Text log parsing follows RFC 3164 (BSD Syslog). RFC 5424 is not yet supported.

## Installation

### From Source

Requires Rust 1.88+ (edition 2024):

```sh
# Install Rust if needed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build release binary
cargo build --release
```

The binary will be at `target/release/cassandra`. For a debug build: `cargo build` (output at `target/debug/cassandra`).

### Granting Log Access (Recommended)

Most log files under `/var/log/` are root-only. Instead of running as root, grant the binary the `cap_dac_read_search` capability:

```sh
# For release build
sudo ./cap.sh

# For debug build
sudo ./cap.sh target/debug/cassandra
```

> Re-run `cap.sh` after every rebuild—the capability is stripped on recompile.

## Usage

```sh
cassandra [OPTIONS] <COMMAND>
```

### Commands

```sh
cassandra sys                    # System logs
cassandra auth                   # Authentication logs
cassandra wtmp                   # Login records
cassandra journal                # User journal
cassandra journal --scope system # System journal
```

### Display Options (Global)

These flags are **mutually exclusive** and work with any command:

| Flag | Description |
|------|-------------|
| `--standard` | Default tabular view (default, paged via `less -S -R` when TTY) |
| `-c`,`--compact <WIDTH>` | Truncate each column to `WIDTH` characters |
| `-s`,`--summary <COLS>` | Show only comma-separated columns, e.g. `--summary time,msg` |
| `-k, --key <KEY>` | Render a single field as key/value pairs |
| `-R, --raw` | Raw tab-separated, no wrapping/truncation, no pager (streaming, grep-friendly) |
| `--no-pager` | Disable pager even when TTY (print directly) |

```sh
# Compact view, max 40 chars per column
cassandra auth --compact 40

# Only show timestamp and message columns
cassandra sys --summary timestamp,message

# Key/value output for a single field
cassandra journal --key message
```

### Other Options

| Flag | Description |
|------|-------------|
| `-l, --lines <N>` | Limit output to last N lines |
| `-r, --reverse` | Reverse output order (newest first, flag, no value) |
| `-F, --rows <COL=VAL>` | Filter rows `col=val` comma-separated, e.g. `--rows host=myhost,process=sshd` |
| `-e, --search <REGEX>` | Regex search across all fields, e.g. `--search 'error|failed'` |
| `-S, --since <TIME>` | Since time (UTC) `2024-01-01`, `15 days ago`, `2h ago`, `now-2h`, `today` |
| `-U, --until <TIME>` | Until time (UTC) |
| `-g, --gravity <LEVEL>` | Journal only: `critical` (0-3), `medium` (4-5), `low` (6-7) |
| `--list-columns` | List available columns for the subcommand (journal respects `--scope`) |

```sh
# Last 50 auth entries, chronological order
cassandra auth -l 50

# Last 20 journal entries, newest first
cassandra journal -l 20 -r

# Time filtering (UTC)
cassandra sys --since '2h ago' --until '2024-01-01'
cassandra journal --since '15 days ago' --gravity low

# Pager: table output is paged via `less -S -R` when TTY for horizontal scroll
# Use --raw for pipe/grep (no pager, tab-separated, no wrapping/truncation)
cassandra sys --search 'Failed.*password' --raw | grep myhost
cassandra journal --summary message,priority --raw | cut -f1

# Disable pager explicitly
cassandra sys --no-pager -l 100 | cat
```

Pager respects `$PAGER` (default `less -S -R`) and `NO_PAGER`/`CI` env. Table overflow no longer inserts `\n` inside words (`Disabled` + 60-char truncated); filtering (`--search`/`--rows`/`--since`) happens before truncation.

Run `cassandra --help` or `cassandra <command> --help` for the full reference.

## Design Decisions

- **RFC 3164 only**: Legacy BSD Syslog format. Most distros still emit this locally; RFC 5424 support is planned.
- **CLI first**: TUI/async interface is planned for future releases.
- **Binary capabilities over sudo**: `cap_dac_read_search` is safer than running as root.

## Project Structure

```
src/
├── main.rs            # Entry point
├── lib.rs             # Library root (module exports)
├── cli.rs             # CLI parsing (clap)
├── models.rs          # Shared models & re-exports
├── display.rs         # Display module root
├── parsers.rs         # Parsers module root
├── utils.rs           # Utils module root
├── display/
│   ├── table_cli.rs   # Table rendering (comfy-table, Disabled + truncated)
│   ├── pager.rs       # Pager helper (less -S -R, respects $PAGER/NO_PAGER)
│   └── theme.rs       # Themes (Rose Pine Moon / clap styling)
├── models/
│   ├── auth.rs        # Auth record model
│   ├── journal.rs     # Journal record & scope model
│   ├── sys.rs         # Syslog record model
│   └── wtmp.rs        # Wtmp record model
├── parsers/
│   ├── selector.rs    # Parser dispatch (try_iter streaming)
│   ├── sys.rs         # RFC 3164 syslog parser (try_iter)
│   ├── auth.rs        # Auth/secure parser (RFC 3164, try_iter)
│   ├── wtmp.rs        # Binary wtmp parser (try_iter)
│   └── journal.rs     # systemd journal parser (try_iter, native seek)
└── utils/
    ├── discovery.rs   # File detection & metadata
    └── time.rs        # humantime + chrono time parsing (since/until)
```

## Dependencies

- `clap` — CLI parsing
- `anstyle` — Styling/ANSI colors (themes)
- `comfy-table` — Table rendering (Disabled, truncated 60)
- `regex` — Log line parsing
- `systemd` — Journal access
- `chrono` + `humantime` — `since`/`until` time parsing
- `pager` — `less -S -R` via `$PAGER` (no extra crate, `std::process`)

## License

GPL-3.0 — see [LICENSE](LICENSE).
