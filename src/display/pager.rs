use std::io::{self, IsTerminal, Write};
use std::process::{Command, Stdio};

/// Write `output` to stdout directly or via `less` pager.
///
/// - Only pages when `stdout` is a TTY, `no_pager` is false, and `PAGER` != "cat"
/// - Respects `$PAGER` (fallback `less`), adds `-S -R` for horizontal scroll + colors
/// - If `less` is not found, falls back to plain `println!`
/// - For pipe/CI (`!is_terminal`) or `--no-pager`, prints directly
pub fn pager_or_print(output: &str, no_pager: bool) {
    if no_pager || should_skip_pager() {
        print!("{output}");
        return;
    }

    // Check if output is small enough to not need pager? For now always pager when tty.
    // We paginate when output has many lines or is wider than terminal.
    // Simple heuristic: if lines > terminal height, page. Otherwise still page to get -S.
    // For now, page whenever tty — matches `journalctl` default.
    let pager_cmd = std::env::var("PAGER").unwrap_or_else(|_| "less".to_string());
    if pager_cmd == "cat" || pager_cmd.is_empty() {
        print!("{output}");
        return;
    }

    // Split pager_cmd into program + args (e.g. "less -S" or "most")
    let mut parts = pager_cmd.split_whitespace();
    let prog = parts.next().unwrap_or("less");
    let mut cmd = Command::new(prog);
    for arg in parts {
        cmd.arg(arg);
    }
    // Ensure -S (chop) and -R (raw) when using less and not already specified
    if prog.contains("less") {
        let has_s = cmd
            .get_args()
            .any(|a| a.to_string_lossy().contains('S'));
        let has_r = cmd
            .get_args()
            .any(|a| a.to_string_lossy().contains('R'));
        if !has_s {
            cmd.arg("-S");
        }
        if !has_r {
            cmd.arg("-R");
        }
        // -F quit if one screen, -X don't clear on exit — optional, keep simple
    }
    cmd.stdin(Stdio::piped()).stdout(Stdio::inherit()).stderr(Stdio::inherit());

    match cmd.spawn() {
        Ok(mut child) => {
            if let Some(stdin) = child.stdin.take() {
                let mut handle = stdin;
                // Ignore broken pipe if user quits early (q)
                let _ = handle.write_all(output.as_bytes());
                // handle flush and drop closes stdin, child will exit
            }
            let _ = child.wait();
        }
        Err(_) => {
            // Fallback if pager not found
            print!("{output}");
        }
    }
}

fn should_skip_pager() -> bool {
    // Don't page when not a TTY (pipe/CI) or when NO_PAGER env is set
    if std::env::var("NO_PAGER").is_ok() {
        return true;
    }
    if std::env::var("CI").is_ok() {
        // In CI, don't page even if is_terminal is true in some runners
        return true;
    }
    // Check stdout is terminal
    !io::stdout().is_terminal()
}
