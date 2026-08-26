use crate::models::{AuthRecord, FromLogEntry, JournalRecord, LogEntry, SysRecord, TableDisplay, WtmpRecord};
use crate::models::{JournalScope, RecordType, TableMode};

use comfy_table::presets::{UTF8_FULL, UTF8_FULL_CONDENSED};
use comfy_table::ContentArrangement;

const JOURNAL_KERNEL_COL: [&str; 11] = [
    "MESSAGE",
    "PRIORITY",
    "SYSLOG_FACILITY",
    "SYSLOG_IDENTIFIER",
    "_BOOT_ID",
    "_HOSTNAME",
    "_MACHINE_ID",
    "_RUNTIME_SCOPE",
    "_SOURCE_BOOTTIME_TIMESTAMP",
    "_SOURCE_MONOTONIC_TIMESTAMP",
    "_TRANSPORT",
];

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
            let filtered: Vec<usize> = (0..all_headers.len()).filter(|i| keep.contains(i)).collect();
            let h: Vec<String> = filtered.iter().map(|&i| all_headers[i].to_string()).collect();
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
        let cells: Vec<String> = col_map.iter().map(|&i| fields[i].clone()).collect();
        table.add_row(cells);
    }

    if let Some(width) = max_col_width {
        table.set_truncation_indicator("...");
        let constraints: Vec<_> = (0..table.column_count())
            .map(|_| comfy_table::ColumnConstraint::UpperBoundary(
                comfy_table::Width::Fixed(width as u16),
            ))
            .collect();
        table.set_constraints(constraints);
    }

    table.trim_fmt()
}

fn render_key_value<T: TableDisplay>(rows: Vec<&T>, hide_columns: Option<&[usize]>) -> String {
    let mut output = String::new();
    let all_headers = T::headers();
    let keep: Option<Vec<usize>> = hide_columns.map(|hide| {
        (0..all_headers.len()).filter(|i| !hide.contains(i)).collect()
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
    Some(build_table_with(&rows, keep_columns.as_deref(), max_col_width))
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
    let hide_columns = matches!(scope, JournalScope::System)
        .then(|| column_indices(&headers, &JOURNAL_KERNEL_COL));

    if let TableMode::KeyValue = mode {
        return Some(render_key_value(rows, hide_columns.as_deref()));
    }

    let keep_columns = hide_columns.as_ref().map(|h| {
        (0..headers.len()).filter(|i| !h.contains(i)).collect::<Vec<usize>>()
    });
    let max_col_width = match mode {
        TableMode::Compact { max_col_width } => Some(*max_col_width),
        _ => None,
    };
    Some(build_table_with(&rows, keep_columns.as_deref(), max_col_width))
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
