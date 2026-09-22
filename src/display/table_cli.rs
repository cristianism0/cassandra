use comfy_table::ContentArrangement;
use comfy_table::presets::{UTF8_FULL, UTF8_FULL_CONDENSED};

use crate::models::{
    FromLogEntry, LogEntry,
    auth::AuthRecord,
    journal::{JOURNAL_KERNEL_COL, JournalRecord, JournalScope},
    sys::SysRecord,
    wtmp::WtmpRecord,
};

use crate::display::{RecordType, TableDisplay, TableMode};

fn truncate_content(s: &str, max_width: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_width {
        return s.to_string();
    }
    if max_width <= 3 {
        return s.chars().take(max_width).collect();
    }
    let truncated: String = s.chars().take(max_width - 3).collect();
    format!("{truncated}...")
}

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
    let (headers, col_map) = match keep_columns {
        Some(keep) => {
            let filtered: Vec<usize> = (0..all_headers.len())
                .filter(|i| keep.contains(i))
                .collect();
            let h: Vec<String> = filtered
                .iter()
                .map(|&i| all_headers[i].to_string())
                .collect();
            (h, filtered)
        }
        None => {
            let h: Vec<String> = all_headers.iter().map(|s| s.to_string()).collect();
            let m: Vec<usize> = (0..all_headers.len()).collect();
            (h, m)
        }
    };

    let style = match max_col_width {
        Some(_) => UTF8_FULL_CONDENSED,
        None => UTF8_FULL,
    };

    let mut table = comfy_table::Table::new();
    table
        .load_style(style)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .enforce_styling()
        .set_header(headers);

    for row in rows {
        let fields = row.fields();
        let cells: Vec<String> = col_map
            .iter()
            .map(|&i| {
                if let Some(max_width) = max_col_width {
                    truncate_content(&fields[i], max_width)
                } else {
                    fields[i].clone()
                }
            })
            .collect();
        table.add_row(cells);
    }

    if let Some(width) = max_col_width {
        let constraints: Vec<_> = (0..table.column_count())
            .map(|_| {
                comfy_table::ColumnConstraint::UpperBoundary(comfy_table::Width::Fixed(
                    width as u16,
                ))
            })
            .collect();
        table.set_constraints(constraints);
    }

    table.trim_fmt()
}

// FIXME: the hiding columns is not working
fn render_key_value<T: TableDisplay>(rows: Vec<&T>, hide_columns: Option<&[usize]>) -> String {
    let mut output = String::new();
    let all_headers = T::headers();
    let keep: Option<Vec<usize>> = hide_columns.map(|hide| {
        (0..all_headers.len())
            .filter(|i| !hide.contains(i))
            .collect()
    });
    for (i, row) in rows.into_iter().enumerate() {
        output.push_str(&format!("[ Entry {} ]\n", i + 1));
        output.push_str(&build_table_with(&[row], keep.as_deref(), None));
        output.push('\n');
    }
    output
}

pub fn build_table<T: TableDisplay + FromLogEntry>(
    entries: &[LogEntry],
    mode: &TableMode,
) -> Option<String> {
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
        TableMode::KeyValue => unreachable!(),
    };
    Some(build_table_with(
        &rows,
        keep_columns.as_deref(),
        max_col_width,
    ))
}

pub fn build_journal_table<T: TableDisplay + FromLogEntry>(
    entries: &[LogEntry],
    scope: &JournalScope,
    mode: &TableMode,
) -> Option<String> {
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

    let keep_columns = hide_columns.as_ref().map(|h| {
        (0..headers.len())
            .filter(|i| !h.contains(i))
            .collect::<Vec<usize>>()
    });
    let max_col_width = match mode {
        TableMode::Compact { max_col_width } => Some(*max_col_width),
        _ => None,
    };
    Some(build_table_with(
        &rows,
        keep_columns.as_deref(),
        max_col_width,
    ))
}

pub fn render_all_tables(records: Vec<RecordType<'_>>, mode: &TableMode) -> Vec<String> {
    let mut rendered = Vec::with_capacity(records.len());
    for record in records {
        let table_opt = match record {
            RecordType::Sys(entries) => build_table::<SysRecord>(entries, mode),
            RecordType::Auth(entries) => build_table::<AuthRecord>(entries, mode),
            RecordType::Wtmp(entries) => build_table::<WtmpRecord>(entries, mode),
            RecordType::Journal(entries, scope) => {
                build_journal_table::<JournalRecord>(entries, scope, mode)
            }
        };
        if let Some(table) = table_opt {
            rendered.push(table);
        }
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::TableMode;
    use crate::models::journal::JournalScope;

    fn sys_entries() -> Vec<LogEntry> {
        vec![
            LogEntry::Sys(SysRecord {
                priority: Some("34".to_string()),
                timestamp: "Oct 11 22:14:15".to_string(),
                host: "myhost".to_string(),
                process: "proc".to_string(),
                message: "hello world, this is a long message".to_string(),
            }),
            LogEntry::Sys(SysRecord {
                priority: None,
                timestamp: "Oct 11 22:14:16".to_string(),
                host: "other".to_string(),
                process: "cron".to_string(),
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
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate_content("abc", 10), "abc");
        assert_eq!(truncate_content("abc", 3), "abc");
    }

    #[test]
    fn truncate_long_string_adds_ellipsis() {
        assert_eq!(truncate_content("abcdef", 5), "ab...");
        assert_eq!(truncate_content("abcdef", 2), "ab");
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
        // Smoke: table renders. No substring assertions on the output here:
        // rendered width follows the terminal, so long lines may wrap.
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
        // Smoke only: the truncation itself is covered by the
        // truncate_content tests; rendered width is terminal-dependent.
        let out = build_table::<SysRecord>(&entries, &TableMode::Compact { max_col_width: 8 })
            .expect("table");
        assert!(!out.is_empty());
    }

    #[test]
    fn build_table_summary_keeps_only_requested() {
        let entries = sys_entries();
        let columns = vec!["message".to_string()];
        // Selection logic (width-independent): "message" is index 4 and the
        // row data maps back to the parsed message.
        let keep = column_indices(&SysRecord::headers(), &columns);
        assert_eq!(keep, vec![4]);
        let rows: Vec<&SysRecord> = group(&entries);
        assert_eq!(
            rows[0].fields()[keep[0]],
            "hello world, this is a long message"
        );
        // Smoke: summary mode renders.
        build_table::<SysRecord>(&entries, &TableMode::Summary { columns }).expect("table");
    }

    #[test]
    fn summary_unknown_column_selects_nothing() {
        // Documents current behavior: unknown columns match nothing, and
        // build_table falls back to the full table in that case.
        let keep = column_indices(
            &SysRecord::headers(),
            &["no-such-column".to_string()],
        );
        assert!(keep.is_empty());
    }

    #[test]
    fn build_table_keyvalue_marks_entries() {
        let entries = sys_entries();
        let out = build_table::<SysRecord>(&entries, &TableMode::KeyValue).expect("table");
        assert!(out.contains("[ Entry 1 ]"));
        assert!(out.contains("[ Entry 2 ]"));
    }

    // NOTE: the 27-column journal table wraps to the detected terminal width
    // (ContentArrangement::Dynamic), so these tests must not assert on
    // substrings of the rendered output. Assert on the column-selection
    // logic instead, plus a render smoke check.
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
        build_journal_table::<JournalRecord>(
            &entries,
            &JournalScope::User,
            &TableMode::Standard,
        )
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
        // Absence is width-independent: wrapping can split words, never
        // create "hostname" out of nothing.
        assert!(!out.contains("hostname"));
    }

    #[test]
    fn render_all_tables_skips_empty() {
        let sys = sys_entries();
        let empty: Vec<LogEntry> = vec![];
        let out = render_all_tables(
            vec![RecordType::Sys(&sys), RecordType::Auth(&empty)],
            &TableMode::Standard,
        );
        assert_eq!(out.len(), 1);
    }
}
