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

Requires Rust 1.85+ (edition 2024):

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
| `--standard` | Default tabular view (default) |
| `-c`,`--compact <WIDTH>` | Truncate each column to `WIDTH` characters |
| `-s`,`--summary <COLS>` | Show only comma-separated columns, e.g. `--summary time,msg` |
| `-k, --key <KEY>` | Render a single field as key/value pairs |

```sh
# Compact view, max 40 chars per column
cassandra auth --compact 40

# Only show timestamp and message columns
cassandra sys --summary time,msg

# Key/value output for a single field
cassandra journal --key message
```

### Other Options

| Flag | Description |
|------|-------------|
| `-l, --lines <N>` | Limit output to last N lines |
| `-r, --reverse` | Reverse output order (newest first) |
| `--rows <COLS>` | *(planned — not yet in the CLI)* Select rows by index |
| `--gravity <LEVEL>` | *(planned — not yet in the CLI)* Journal priority filter: critical, medium, low |
| `--search <TEXT>` | *(planned — not yet in the CLI)* Grep-like filtering |

```sh
# Last 50 auth entries, chronological order
cassandra auth -l 50

# Last 20 journal entries, oldest first
cassandra journal -l 20 -r
```

> **Note**: `--rows`, `--gravity` and `--search` are planned but not yet exposed by the CLI.

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
│   ├── table_cli.rs   # Table rendering (comfy-table)
│   └── theme.rs       # Themes (Rose Pine Moon / clap styling)
├── models/
│   ├── auth.rs        # Auth record model
│   ├── journal.rs     # Journal record & scope model
│   ├── sys.rs         # Syslog record model
│   └── wtmp.rs        # Wtmp record model
├── parsers/
│   ├── selector.rs    # Parser dispatch
│   ├── sys.rs         # RFC 3164 syslog parser
│   ├── auth.rs        # Auth/secure parser (RFC 3164)
│   ├── wtmp.rs        # Binary wtmp parser
│   └── journal.rs     # systemd journal parser
└── utils/
    └── discovery.rs   # File detection & metadata
```

## Dependencies

- `clap` — CLI parsing
- `anstyle` — Styling/ANSI colors (themes)
- `comfy-table` — Table rendering
- `regex` — Log line parsing
- `systemd` — Journal access

## License

GPL-3.0 — see [LICENSE](LICENSE).