use comfy_table::presets::{ASCII_FULL, ASCII_FULL_CONDENSED};
use std::fmt::Write as _;

use crate::models::{
    FromLogEntry, LogEntry,
    journal::{JOURNAL_KERNEL_COL, JournalScope},
};

use crate::display::{TableDisplay, TableMode};

pub fn group<T: FromLogEntry>(entries: &[LogEntry]) -> Vec<&T> {
    entries.iter().filter_map(T::from_entry).collect()
}

fn column_indices(headers: &[&'static str], names: &[impl AsRef<str>]) -> Vec<usize> {
    names
        .iter()
        .filter_map(|n| {
            headers
                .iter()
                .position(|h| h.eq_ignore_ascii_case(n.as_ref()))
        })
        .collect()
}

fn build_table_with<T: TableDisplay>(
    rows: &[&T],
    keep_columns: Option<&[usize]>,
    max_col_width: Option<usize>,
) -> String {
    let all_headers = T::headers();
    let (headers, col_map) = if let Some(keep) = keep_columns {
        let filtered: Vec<usize> = (0..all_headers.len())
            .filter(|i| keep.contains(i))
            .collect();
        let h: Vec<String> = filtered
            .iter()
            .map(|&i| all_headers[i].to_string())
            .collect();
        (h, filtered)
    } else {
        let h: Vec<String> = all_headers
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        let m: Vec<usize> = (0..all_headers.len()).collect();
        (h, m)
    };

    let style = match max_col_width {
        Some(_) => ASCII_FULL_CONDENSED,
        None => ASCII_FULL,
    };

    let mut table = comfy_table::Table::new();
    table
        .load_style(style)
        .enforce_styling()
        .set_header(headers);

    for row in rows {
        let fields = row.fields();
        let cells: Vec<String> = col_map.iter().map(|&i| fields[i].clone()).collect();
        table.add_row(cells);
    }
    table.trim_fmt()
}

fn render_key_value<T: TableDisplay>(rows: Vec<&T>, hide_columns: Option<&[usize]>) -> String {
    let mut output = String::new();
    let all_headers = T::headers();
    let keep: Option<Vec<usize>> = hide_columns.map(|hide| {
        (0..all_headers.len())
            .filter(|i| !hide.contains(i))
            .collect()
    });
    for (i, row) in rows.into_iter().enumerate() {
        let _ = writeln!(output, "[ Entry {} ]", i + 1);
        output.push_str(&build_table_with(&[row], keep.as_deref(), None));
        output.push('\n');
    }
    output
}

#[must_use]
pub fn build_raw<T: TableDisplay + FromLogEntry>(entries: &[LogEntry]) -> Option<String> {
    let rows = group::<T>(entries);
    if rows.is_empty() {
        return None;
    }
    let headers = T::headers();
    let mut out = String::new();
    out.push_str(&headers.join("\t"));
    out.push('\n');
    for r in rows {
        out.push_str(&r.fields().join("\t"));
        out.push('\n');
    }
    Some(out.trim_end().to_string())
}

#[must_use]
pub fn build_table<T: TableDisplay + FromLogEntry>(
    entries: &[LogEntry],
    mode: &TableMode,
) -> Option<String> {
    if let TableMode::Raw = mode {
        return build_raw::<T>(entries);
    }
    let rows = group::<T>(entries);
    if rows.is_empty() {
        return None;
    }
    if let TableMode::KeyValue = mode {
        return Some(render_key_value(rows, None));
    }
    let (keep_columns, max_col_width) = match mode {
        TableMode::Standard => (None, None),
        TableMode::Compact { max_col_width } => (None, Some(*max_col_width)),
        TableMode::Summary { columns } => {
            let headers = T::headers();
            let keep = column_indices(&headers, columns);
            let keep = if keep.is_empty() { None } else { Some(keep) };
            (keep, None)
        }
        TableMode::KeyValue | TableMode::Raw => unreachable!(),
    };
    Some(build_table_with(
        &rows,
        keep_columns.as_deref(),
        max_col_width,
    ))
}

#[must_use]
pub fn build_journal_table<T: TableDisplay + FromLogEntry>(
    entries: &[LogEntry],
    scope: &JournalScope,
    mode: &TableMode,
) -> Option<String> {
    if let TableMode::Raw = mode {
        // To keep raw grep-friendly and complete, show all headers.
        return build_raw::<T>(entries);
    }
    let rows = group::<T>(entries);
    if rows.is_empty() {
        return None;
    }
    let headers = T::headers();
    let hide_columns =
        matches!(scope, JournalScope::System).then(|| column_indices(&headers, JOURNAL_KERNEL_COL));

    if let TableMode::KeyValue = mode {
        return Some(render_key_value(rows, hide_columns.as_deref()));
    }

    let (keep_columns, max_col_width) = match mode {
        TableMode::Summary { columns } => {
            let keep = column_indices(&headers, columns);
            let keep_opt = if keep.is_empty() { None } else { Some(keep) };
            (keep_opt, None)
        }
        TableMode::Compact { max_col_width } => {
            let keep = hide_columns.as_ref().map(|h| {
                (0..headers.len())
                    .filter(|i| !h.contains(i))
                    .collect::<Vec<usize>>()
            });
            (keep, Some(*max_col_width))
        }
        TableMode::Standard => {
            let keep = hide_columns.as_ref().map(|h| {
                (0..headers.len())
                    .filter(|i| !h.contains(i))
                    .collect::<Vec<usize>>()
            });
            (keep, None)
        }
        TableMode::KeyValue | TableMode::Raw => unreachable!(),
    };
    Some(build_table_with(
        &rows,
        keep_columns.as_deref(),
        max_col_width,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::TableMode;
    use crate::models::journal::JournalScope;
    use crate::models::{auth::AuthRecord, journal::JournalRecord, sys::SysRecord};

    fn sys_entries() -> Vec<LogEntry> {
        vec![
            LogEntry::Sys(SysRecord {
                priority: Some("34".to_string()),
                host: "myhost".to_string(),
                process: "proc".to_string(),
                timestamp: "Oct 11 22:14:15".to_string(),
                message: "hello world, this is a long message".to_string(),
            }),
            LogEntry::Sys(SysRecord {
                priority: None,
                host: "other".to_string(),
                process: "cron".to_string(),
                timestamp: "Oct 11 22:14:16".to_string(),
                message: "second".to_string(),
            }),
        ]
    }

    fn journal_entry() -> Vec<LogEntry> {
        vec![LogEntry::Journal(Box::new(JournalRecord {
            message: "msg".to_string(),
            priority: Some("3".to_string()),
            code_file: None,
            code_func: None,
            code_line: None,
            syslog_facility: None,
            syslog_identifier: None,
            tid: None,
            audit_loginuid: None,
            audit_session: None,
            boot_id: None,
            gid: None,
            hostname: Some("myhost".to_string()),
            machine_id: None,
            pid: None,
            runtime_scope: None,
            selinux_context: None,
            source_monotonic_timestamp: None,
            source_boottime_timestamp: None,
            source_realtime_timestamp: None,
            systemd_cgroup: None,
            systemd_owner_uid: None,
            systemd_slice: None,
            systemd_unit: None,
            systemd_user_slice: None,
            transport: Some("journal".to_string()),
            uid: None,
        }))]
    }

    #[test]
    fn column_indices_case_insensitive_and_missing() {
        let headers = ["message", "host", "process"];
        let got = column_indices(&headers, &["HOST".to_string(), "missing".to_string()]);
        assert_eq!(got, vec![1]);
    }

    #[test]
    fn group_filters_by_type() {
        let entries = sys_entries();
        let sys: Vec<&SysRecord> = group(&entries);
        let auth: Vec<&AuthRecord> = group(&entries);
        assert_eq!(sys.len(), 2);
        assert!(auth.is_empty());
    }

    #[test]
    fn build_table_standard_has_headers_and_rows() {
        let entries = sys_entries();
        build_table::<SysRecord>(&entries, &TableMode::Standard).expect("table");
        assert!(SysRecord::headers().contains(&"timestamp"));
        let rows: Vec<&SysRecord> = group(&entries);
        assert_eq!(rows.len(), 2);
        assert!(
            rows[0].fields().iter().any(|f| f.contains("hello world")),
            "row fields must carry the parsed message"
        );
    }

    #[test]
    fn build_table_empty_returns_none() {
        let empty: Vec<LogEntry> = vec![];
        assert!(build_table::<SysRecord>(&empty, &TableMode::Standard).is_none());
    }

    #[test]
    fn build_table_compact_renders() {
        let entries = sys_entries();
        let out = build_table::<SysRecord>(&entries, &TableMode::Compact { max_col_width: 8 })
            .expect("table");
        assert!(!out.is_empty());
    }

    #[test]
    fn build_table_summary_keeps_only_requested() {
        let entries = sys_entries();
        let columns = vec!["message".to_string()];
        let keep = column_indices(&SysRecord::headers(), &columns);
        assert_eq!(keep, vec![4]);
        let rows: Vec<&SysRecord> = group(&entries);
        assert_eq!(
            rows[0].fields()[keep[0]],
            "hello world, this is a long message".to_string()
        );
    }

    #[test]
    fn summary_unknown_column_selects_nothing() {
        let keep = column_indices(&SysRecord::headers(), &["no-such-column".to_string()]);
        assert!(keep.is_empty());
    }

    #[test]
    fn build_table_keyvalue_marks_entries() {
        let entries = sys_entries();
        let out = build_table::<SysRecord>(&entries, &TableMode::KeyValue).expect("table");
        assert!(out.contains("[ Entry 1 ]"));
        assert!(out.contains("[ Entry 2 ]"));
    }

    #[test]
    fn journal_hide_list_covers_hostname() {
        let headers = JournalRecord::headers();
        let host_idx = headers
            .iter()
            .position(|h| *h == "hostname")
            .expect("hostname header exists");
        let hidden = column_indices(&headers, JOURNAL_KERNEL_COL);
        assert!(
            hidden.contains(&host_idx),
            "system scope must hide the hostname column"
        );
    }

    #[test]
    fn build_journal_user_scope_renders() {
        let entries = journal_entry();
        build_journal_table::<JournalRecord>(&entries, &JournalScope::User, &TableMode::Standard)
            .expect("table");
    }

    #[test]
    fn build_journal_system_scope_hides_kernel_cols() {
        let entries = journal_entry();
        let out = build_journal_table::<JournalRecord>(
            &entries,
            &JournalScope::System,
            &TableMode::Standard,
        )
        .expect("table");
        assert!(!out.contains("hostname"));
    }
}
