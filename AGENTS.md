# lunete

This agents.md file is destined to pass instructions about the code design and patterns used in this project.

## Structure
The project try to keep an CLI stable to a TUI implementation. Only implement new features on the TUI if the CLI is stable. Unless, its a TUI only feature.

- `parsers/`: All the parsers should be make with a trait implementation, following the pattern already used in this module on `parsers/selector.rs`. All new parsers should also be included in this module.
- `models/`: New enum models used by parsers and error treatments should be added on the respective named file.
- `utils`: all paths traversing and file information for Finfo struct should be done in this module. This includes sockets binds.

## Practices
- This project will rely on CLI and TUI application. The CLI should remain sync and the TUI will be constructed with ratatui and will use tokio as async lib.
- Keep the dependencies clean: clap, systemd, comfy-table, regex, anstyle.
- Only implement optimization after an full working application. Does not tries to implement premature optimization.
- Does not try to get elevated permissions to read reserved logs during the runtime, the repository has an DAC script to gave permissions to read security files, this need to be reapplied after an new cargo run/release since DAC capabilities do not persist across new rebuilds.
- Does not spam comments on the code, unless on trade-off or a 'may broken' feature. Keep it explicit.
- Avoid the use of `unwrap()` and `.expect()` unless the panic is unreachable or there is an explicit fall back.
- Prefer to write error handlings with custom `enums` and using the "Error: ... \nHint:... \nDetails: ... (Err)" convention. Use the "Hint" if it can be infered by the funcion that caused the error. If there is an explicit fallback or default value to be attach, prefer to use `.unwrap_or()` function.
- Use `cargo clippy` to see safe modifications and linter recommendations and prefer to use it.

## Dependencies
- tokio -> async
- comfy-table -> CLI display
- regex -> parse files
- systemd -> socket connection
- clap -> CLI parsing
- anstyle -> CLI colors

## AI usage
The original repository author writes the core logic (parsers, models, trait implementation and async orchestration) by hand for study purposes, using AI assistance to make the output/display options and post-work optimizations. This is the maintainer and author preference and not an project rule. Contributors and their agents are free to use AI assistance, since they are following architecture/style conventions described above.
