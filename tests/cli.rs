use std::process::Command;

fn bin() -> String {
    env!("CARGO_BIN_EXE_cassandra").to_string()
}

#[test]
fn help_exits_zero_and_mentions_commands() {
    let out = Command::new(bin())
        .arg("--help")
        .output()
        .expect("run cassandra --help");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("sys"), "help should list sys: {stdout}");
    assert!(stdout.contains("auth"), "help should list auth: {stdout}");
    assert!(stdout.contains("wtmp"), "help should list wtmp: {stdout}");
    assert!(stdout.contains("journal"), "help should list journal: {stdout}");
}

#[test]
fn version_exits_zero() {
    let out = Command::new(bin())
        .arg("--version")
        .output()
        .expect("run cassandra --version");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("cassandra"), "unexpected version output: {stdout}");
}

#[test]
fn subcommand_help_exits_zero() {
    for sub in ["sys", "auth", "wtmp", "journal"] {
        let out = Command::new(bin())
            .args([sub, "--help"])
            .output()
            .expect("run subcommand --help");
        assert!(
            out.status.success(),
            "help for {sub} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn lines_zero_is_rejected_before_file_access() {
    // Deterministic: run_cli exits(2) on lines == 0 without touching /var/log.
    let out = Command::new(bin())
        .args(["auth", "-l", "0"])
        .output()
        .expect("run cassandra auth -l 0");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn invalid_flag_fails() {
    let out = Command::new(bin())
        .arg("--bogus-flag")
        .output()
        .expect("run with invalid flag");
    assert!(!out.status.success());
}
