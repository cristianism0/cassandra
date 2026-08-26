use std::borrow::Cow;

use crate::models::{AuthRecord, FromLogEntry, JournalRecord, LogEntry, SysRecord, WtmpRecord};
use crate::models::{JournalScope, RecordType, TableKey, TableMode};
use tabled::{
    Table, Tabled,
    settings::{Remove, Style, Width, object::Columns},
};

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

fn remove_columns(table: &mut Table, mut indices: Vec<usize>) {
    indices.sort_unstable();
    indices.dedup();
    for col in indices.into_iter().rev() {
        table.with(Remove::column(Columns::new(col..=col)));
    }
}

fn column_indices(headers: &[Cow<'static, str>], names: &[impl AsRef<str>]) -> Vec<usize> {
    names
        .iter()
        .filter_map(|n| {
            headers
                .iter()
                .position(|h| h.eq_ignore_ascii_case(n.as_ref()))
        })
        .collect()
}

fn apply_mode_style<T: Tabled>(table: &mut Table, mode: &TableMode, key: TableKey) {
    match mode {
        TableMode::Standard => {
            table.with(Style::rounded());
        }
        TableMode::Compact { max_col_width } => {
            table
                .with(Style::sharp())
                .with(Width::truncate(*max_col_width).suffix("..."));
        }
        TableMode::Summary { columns } => {
            table.with(Style::modern());
            if let Some(names) = columns.get(&key) {
                let headers = T::headers();
                let keep = column_indices(&headers, names);
                if !keep.is_empty() {
                    let drop: Vec<usize> =
                        (0..headers.len()).filter(|c| !keep.contains(c)).collect();
                    remove_columns(table, drop);
                }
            }
        }
        TableMode::KeyValue => unreachable!("KeyValue is rendered via render_key_value"),
    }
}

fn render_key_value<T: Tabled>(rows: Vec<&T>, hide_columns: Option<&[usize]>) -> String {
    let mut output = String::new();
    for (i, row) in rows.into_iter().enumerate() {
        let mut table = Table::new(vec![row]);
        if let Some(cols) = hide_columns {
            remove_columns(&mut table, cols.to_vec());
        }
        table.with(Style::extended());
        output.push_str(&format!("[ Entry {} ]\n", i + 1));
        output.push_str(&table.to_string());
        output.push_str("\n\n");
    }
    output
}

pub fn build_table<T: Tabled + FromLogEntry>(
    entries: &[LogEntry],
    mode: &TableMode,
    key: TableKey,
) -> Option<String> {
    let rows = group::<T>(entries);
    if rows.is_empty() {
        return None;
    }
    if let TableMode::KeyValue = mode {
        return Some(render_key_value(rows, None));
    }
    let mut table = Table::new(rows);
    apply_mode_style::<T>(&mut table, mode, key);
    Some(table.to_string())
}

pub fn build_journal_table<T: Tabled + FromLogEntry>(
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
    let mut table = Table::new(rows);
    if let Some(cols) = &hide_columns {
        remove_columns(&mut table, cols.clone());
    }
    apply_mode_style::<T>(&mut table, mode, TableKey::Journal);
    Some(table.to_string())
}

pub fn render_all_tables(records: Vec<RecordType<'_>>, mode: &TableMode) -> Vec<String> {
    let mut rendered = Vec::with_capacity(records.len());
    for record in records {
        let table_opt = match record {
            RecordType::Sys(entries) => build_table::<SysRecord>(entries, mode, TableKey::Sys),
            RecordType::Auth(entries) => build_table::<AuthRecord>(entries, mode, TableKey::Auth),
            RecordType::Wtmp(entries) => build_table::<WtmpRecord>(entries, mode, TableKey::Wtmp),
            RecordType::Journal(entries, scope) => {
                build_journal_table::<JournalRecord>(entries, &scope, mode)
            }
        };
        if let Some(table) = table_opt {
            rendered.push(table);
        }
    }
    rendered
}
