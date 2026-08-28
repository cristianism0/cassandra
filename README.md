<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/lunete-dark.gif">
    <source media="(prefers-color-scheme: light)" srcset="assets/lunete-light.gif">
    <img alt="lunete" src="assets/lunete-dark.gif">
  </picture>
</p>

# lunete

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

The binary will be at `target/release/lunete`. For a debug build: `cargo build` (output at `target/debug/lunete`).

### Granting Log Access (Recommended)

Most log files under `/var/log/` are root-only. Instead of running as root, grant the binary the `cap_dac_read_search` capability:

```sh
# For release build
sudo ./cap.sh

# For debug build
sudo ./cap.sh target/debug/lunete
```

> Re-run `cap.sh` after every rebuild—the capability is stripped on recompile.

## Usage

```sh
lunete [OPTIONS] <COMMAND>
```

### Commands

```sh
lunete sys                    # System logs
lunete auth                   # Authentication logs
lunete wtmp                   # Login records
lunete journal                # User journal (default)
lunete journal --scope system # System journal
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
lunete auth --compact 40

# Only show timestamp and message columns
lunete sys --summary time,msg

# Key/value output for a single field
lunete journal --key message
```

### Other Options

| Flag | Description |
|------|-------------|
| `-l, --lines <N>` | Limit output to last N lines |
| `-r, --reverse` | Reverse output order (oldest first) |
| `--rows <COLS>` | *(planned)* Select rows by index |
| `--gravity <LEVEL>` | *(accepted, not implemented)* Journal priority filter: critical, medium, low |
| `--search <TEXT>` | *(accepted, not implemented)* Grep-like filtering |

```sh
# Last 50 auth entries, newest first
lunete auth -l 50

# Last 20 journal entries, oldest first
lunete journal -l 20 -r
```

> **Note**: `--gravity` and `--search` are parsed but not yet wired to filtering logic.

Run `lunete --help` or `lunete <command> --help` for the full reference.

## Design Decisions

- **RFC 3164 only**: Legacy BSD Syslog format. Most distros still emit this locally; RFC 5424 support is planned.
- **CLI first**: TUI/async interface is planned for future releases.
- **Binary capabilities over sudo**: `cap_dac_read_search` is safer than running as root.

## Project Structure

```
src/
├── main.rs           # Entry point
├── cli.rs            # CLI parsing (clap)
├── models.rs         # Data structures & traits
├── display/
│   └── table_cli.rs  # Table rendering (comfy-table)
├── parsers/
│   ├── selector.rs   # Parser dispatch
│   ├── sys.rs        # RFC 3164 syslog parser
│   ├── auth.rs       # Auth/secure parser (RFC 3164)
│   ├── wtmp.rs       # Binary wtmp parser
│   └── journal.rs    # systemd journal parser
└── utils/
    └── discovery.rs  # File detection & metadata
```

## Dependencies

- `clap` — CLI parsing
- `comfy-table` — Table rendering
- `regex` — Log line parsing
- `systemd` — Journal access

## License

GPL-3.0 — see [LICENSE](LICENSE).