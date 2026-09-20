//! Functional tests through the public library API:
//! temp log file -> Finfo -> parser_selector -> build_table.

use cassandra::display::table_cli::build_table;
use cassandra::display::{TableDisplay, TableMode};
use cassandra::models::{Finfo, LogEntry, LogSource, PathStatus, sys::SysRecord};
use cassandra::parsers::selector::parser_selector;
use std::sync::atomic::{AtomicU64, Ordering};

static CTR: AtomicU64 = AtomicU64::new(0);

fn write_tmp(name: &str, contents: &str) -> std::path::PathBuf {
    let id = CTR.fetch_add(1, Ordering::SeqCst);
    let mut p = std::env::temp_dir();
    p.push(format!(
        "cassandra-functional-{}-{id}-{name}",
        std::process::id()
    ));
    std::fs::write(&p, contents).expect("write tmp fixture");
    p
}

fn finfo_for(path: std::path::PathBuf, source: LogSource) -> Finfo {
    Finfo {
        path,
        source,
        pstatus: PathStatus::Found,
        data: None,
    }
}

#[test]
fn sys_file_to_table_end_to_end() {
    let p = write_tmp(
        "sys.log",
        "Oct 11 22:14:15 myhost proc[10]: first message\nOct 11 22:14:16 myhost proc[10]: second\n",
    );
    let fi = finfo_for(p.clone(), LogSource::Sys);
    let entries = parser_selector(&fi, None, false).expect("parse ok");
    assert_eq!(entries.len(), 2);

    let table = build_table::<SysRecord>(&entries, &TableMode::Standard).expect("table");
    assert!(table.contains("first message"));
    assert!(table.contains("second"));
    let _ = std::fs::remove_file(&p);
}

#[test]
fn sys_lines_limit_and_compact_end_to_end() {
    let p = write_tmp(
        "sys2.log",
        "Oct 11 22:14:15 h p: one\nOct 11 22:14:16 h p: two\nOct 11 22:14:17 h p: three\n",
    );
    let fi = finfo_for(p.clone(), LogSource::Sys);
    let entries = parser_selector(&fi, Some(1), false).expect("parse ok");
    assert_eq!(entries.len(), 1);
    match &entries[0] {
        LogEntry::Sys(r) => assert_eq!(r.message, "three"),
        other => panic!("expected sys, got {other:?}"),
    }
    let table = build_table::<SysRecord>(
        &entries,
        &TableMode::Compact { max_col_width: 10 },
    )
    .expect("table");
    assert!(table.contains("three"));
    let _ = std::fs::remove_file(&p);
}

#[test]
fn summary_with_unknown_column_falls_back_to_full_table() {
    // Documents current behavior: unknown columns -> keep empty -> full table.
    let entries = vec![LogEntry::Sys(SysRecord {
        priority: None,
        timestamp: "Oct 11 22:14:15".to_string(),
        host: "h".to_string(),
        process: "p".to_string(),
        message: "m".to_string(),
    })];
    let table = build_table::<SysRecord>(
        &entries,
        &TableMode::Summary {
            columns: vec!["no-such-column".to_string()],
        },
    )
    .expect("table");
    assert!(table.contains("timestamp"));
}

#[test]
fn headers_and_fields_stay_in_sync() {
    assert_eq!(
        SysRecord::headers().len(),
        SysRecord {
            priority: None,
            timestamp: String::new(),
            host: String::new(),
            process: String::new(),
            message: String::new(),
        }
        .fields()
        .len()
    );
}
