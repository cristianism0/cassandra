use std::io::{self, IsTerminal, Write};
use std::process::{Command, Stdio};

/// Write `output` to stdout directly or via `less` pager.
/// - Only pages when `stdout` is a TTY, `no_pager` is false, and `PAGER` != "cat"
/// - Respects `$PAGER` (fallback `less`), adds `-S -R` for horizontal scroll + colors
/// - For pipe/CI (`!is_terminal`) or `--no-pager`, prints directly
pub fn pager_or_print(output: &str, no_pager: bool) {
    if no_pager || should_skip_pager() {
        print!("{output}");
        return;
    }

    let pager_cmd = std::env::var("PAGER").unwrap_or_else(|_| "less".to_string());
    if pager_cmd == "cat" || pager_cmd.is_empty() {
        print!("{output}");
        return;
    }

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
    }
    cmd.stdin(Stdio::piped()).stdout(Stdio::inherit()).stderr(Stdio::inherit());

    match cmd.spawn() {
        Ok(mut child) => {
            if let Some(stdin) = child.stdin.take() {
                let mut handle = stdin;
                let _ = handle.write_all(output.as_bytes());
            }
            let _ = child.wait();
        }
        Err(_) => {
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
    !io::stdout().is_terminal()
}
