<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/lunete-dark.gif">
    <source media="(prefers-color-scheme: light)" srcset="assets/lunete-light.gif">
    <img alt="lunete" src="assets/lunete-dark.gif">
  </picture>
</p>

Lunete is rust-based CLI application that can be used for read linux system logs. It has support for:
- `journald`
- `auth.log`
- `syslog`
- `messages`
- `secure`

### Design Decisions
- RFC 3164 (BSD Syslog) - Legacy
This RFC is obsolete and was substituted for RFC 5424. Yet, for a more easier approach, for local information, the legacy RFC 3164 still being using in most Linux Distributions. This project supports RFC 3164 only.
- CLI first, TUI async after. *Coming soon!*

## Building from source

There are no prebuilt releases yet, so you need to compile `lunete` yourself. You will need a recent Rust toolchain (edition 2024, so use Rust 1.85+):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

cargo build --release
```

The compiled binary will be at `target/release/lunete`. For a faster, unoptimized debug build use `cargo build` (binary at `target/debug/lunete`).

## Capabilities

Most log files (`/var/log/auth.log`, `/var/log/secure`, `/var/log/wtmp`, etc.) are only readable by `root`. Instead of running `lunete` as root, you can grant the binary the `cap_dac_read_search` capability, which lets it traverse/read files regardless of DAC permissions.

The repository ships a helper script, `cap.sh`, that sets the capability on the binary:

```sh
sudo ./cap.sh

# or point it at a different path, e.g. the debug build
sudo ./cap.sh target/debug/lunete
```

`cap.sh` accepts the binary path as its first argument (defaulting to `target/debug/lunete`). Adjust the path to wherever your compiled `lunete` lives. After this, you can run `lunete` as a normal user and it will be able to read the protected logs.

> Note: the capability is lost whenever you recompile the binary, so re-run `cap.sh` after a fresh `cargo build`.

## Usage

```
lunete [OPTIONS] <COMMAND>
```

### Commands

| Command    | Description                                                          |
|------------|----------------------------------------------------------------------|
| `sys`      | Read system logs (`/var/log/syslog`, `/var/log/messages`, ...).      |
| `auth`     | Read authentication logs (`/var/log/auth.log`, `/var/log/secure`).   |
| `wtmp`     | Read the `wtmp` login records (`/var/log/wtmp`).                     |
| `journal`  | Read `journald` entries. Use `--scope user` (default) or `system`.   |

```sh
lunete sys
lunete auth
lunete wtmp
lunete journal --scope system
```

### Display options (global)

These flags work with any command and are mutually exclusive:

| Flag                      | Description                                                                 |
|---------------------------|-----------------------------------------------------------------------------|
| `--standard`              | Default tabular view.                                                       |
| `--compact <WIDTH>`       | Truncate each column to `WIDTH` characters.                                 |
| `--summary <COLS>`        | Show only the given comma-separated columns, e.g. `--summary time,msg`.     |
| `-k, --key <KEY>`         | Render a single field as a key/value pair.                                  |

```sh
# compact view, max 40 chars per column
lunete auth --compact 40

# only show the timestamp and message columns
lunete sys --summary time,msg

# key/value for a single field
lunete journal --key message
```

Run `lunete --help` or `lunete <command> --help` for the full, up-to-date reference.
