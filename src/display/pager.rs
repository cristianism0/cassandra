use std::env::var;
use std::io::{self, IsTerminal, Write};
use std::process::{Command, Stdio};

pub fn pager_or_print(output: &str, no_pager: bool) {
    if no_pager || should_skip_pager() {
        print!("{output}");
    }

    let pager_cmd = var("PAGER").unwrap_or_else(|_| "less".to_string());
    if pager_cmd == "cat" || pager_cmd.is_empty() {
        print!("{output}");
    }

    let mut parts = pager_cmd.split_whitespace();
    let prog = parts.next().unwrap_or("less");
    let mut cmd = Command::new(prog);
    for arg in parts {
        cmd.arg(arg);
    }

    if prog.contains("less") {
        let has_s = cmd.get_args().any(|a| a.to_string_lossy().contains('S'));
        let has_r = cmd.get_args().any(|a| a.to_string_lossy().contains('R'));
        if !has_s {
            cmd.arg("-S");
        }
        if !has_r {
            cmd.arg("-R");
        }
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

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
    if var("NO_PAGER").is_ok() {
        return true;
    }
    if var("CI").is_ok() {
        return true;
    }
    !io::stdout().is_terminal()
}
