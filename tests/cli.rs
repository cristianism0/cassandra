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

#[test]
fn list_columns_per_subcommand() {
    for (sub, expected) in [
        ("sys", "message"),
        ("auth", "caller"),
        ("wtmp", "ut_user"),
        ("journal", "hostname"),
    ] {
        let out = Command::new(bin())
            .args([sub, "--list-columns"])
            .output()
            .expect("run list-columns");
        assert!(
            out.status.success(),
            "list-columns for {sub} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains(expected),
            "list-columns for {sub} should contain {expected}: {stdout}"
        );
    }
}

#[test]
fn invalid_search_regex_is_rejected() {
    let out = Command::new(bin())
        .args(["sys", "--search", "["])
        .output()
        .expect("run with invalid regex");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Invalid search regex"), "stderr: {stderr}");
}

#[test]
fn invalid_rows_rejected() {
    let out = Command::new(bin())
        .args(["sys", "--rows", "badfilter"])
        .output()
        .expect("run with invalid rows");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Invalid --rows"), "stderr: {stderr}");
}

#[test]
fn invalid_rows_unknown_column_rejected() {
    // unknown column should be rejected before file access
    let out = Command::new(bin())
        .args(["sys", "--rows", "nope=val"])
        .output()
        .expect("run with unknown column");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("unknown column"), "stderr: {stderr}");
}

#[test]
fn journal_gravity_help_lists_values() {
    let out = Command::new(bin())
        .args(["journal", "--help"])
        .output()
        .expect("journal help");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("gravity"), "help should mention gravity: {stdout}");
    assert!(stdout.contains("critical"), "help should list critical: {stdout}");
}

#[test]
fn invalid_since_is_rejected() {
    let out = Command::new(bin())
        .args(["sys", "--since", "not-a-date"])
        .output()
        .expect("run with invalid since");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Invalid --since"), "stderr: {stderr}");
}

#[test]
fn invalid_until_is_rejected() {
    let out = Command::new(bin())
        .args(["sys", "--until", "bad"])
        .output()
        .expect("run with invalid until");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Invalid --until"), "stderr: {stderr}");
}

#[test]
fn since_after_until_is_rejected() {
    let out = Command::new(bin())
        .args(["sys", "--since", "2024-12-31", "--until", "2024-01-01"])
        .output()
        .expect("run with since after until");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("since"), "stderr: {stderr}");
}

#[test]
fn since_help_mentions_human_time() {
    let out = Command::new(bin())
        .args(["sys", "--help"])
        .output()
        .expect("sys help");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("since"), "help should mention since: {stdout}");
    assert!(stdout.contains("until"), "help should mention until: {stdout}");
}

#[test]
fn wtmp_ignores_since() {
    // wtmp should ignore --since/--until and still list columns
    let out = Command::new(bin())
        .args(["wtmp", "--since", "2024-01-01", "--list-columns"])
        .output()
        .expect("wtmp ignore since");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ut_user"), "should list wtmp columns: {stdout}");
}
