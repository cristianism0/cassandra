use crate::models::{FromLogEntry, LogEntry};
use clap::ValueEnum;

#[derive(Debug)]
pub struct JournalRecord {
    pub message: String,
    pub priority: Option<String>,
    pub code_file: Option<String>,
    pub code_func: Option<String>,
    pub code_line: Option<String>,
    pub syslog_facility: Option<String>,
    pub syslog_identifier: Option<String>,
    pub tid: Option<String>,
    pub _audit_loginuid: Option<String>,
    pub _audit_session: Option<String>,
    pub _boot_id: Option<String>,
    pub _gid: Option<String>,
    pub _hostname: Option<String>,
    pub _machine_id: Option<String>,
    pub _pid: Option<String>,
    pub _runtime_scope: Option<String>,
    pub _selinux_context: Option<String>,
    pub _source_monotonic_timestamp: Option<String>,
    pub _source_boottime_timestamp: Option<String>,
    pub _source_realtime_timestamp: Option<String>,
    pub _systemd_cgroup: Option<String>,
    pub _systemd_owner_uid: Option<String>,
    pub _systemd_slice: Option<String>,
    pub _systemd_unit: Option<String>,
    pub _systemd_user_slice: Option<String>,
    pub _transport: Option<String>,
    pub _uid: Option<String>,
}

#[derive(Debug)]
pub struct JournalPreset {
    pub columns: &'static [&'static str],
    pub max_priority: u8,
}

pub const JOURNAL_CRITICAL: &[&str] = &[
    "message",
    "code_file",
    "_hostname",
    "_systemd_unit",
    "_selinux_context",
    "_pid",
    "_source_realtime_timestamp",
];

pub const JOURNAL_MEDIUM: &[&str] = &[
    "message",
    "code_file",
    "code_func",
    "code_line",
    "_audit_loginuid",
    "_audit_session",
    "_selinux_context",
    "_pid",
    "tid",
    "_boot_id",
];

pub const JOURNAL_LOW: &[&str] = &[
    "message",
    "_transport",
    "syslog_facility",
    "_runtime_scope",
    "_systemd_cgroup",
    "_systemd_user_slice",
    "_systemd_owner_uid",
    "_machine_id",
    "_source_monotonic_timestamp",
    "_source_boottime_timestamp",
];

pub const JOURNAL_KERNEL_COL: &[&str] = &[
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ValueEnum)]
pub enum JournalScope {
    System,
    User,
}

impl FromLogEntry for JournalRecord {
    fn from_entry(entry: &LogEntry) -> Option<&Self> {
        match entry {
            LogEntry::Journal(r) => Some(r),
            _ => None,
        }
    }
}

// --------- Errors --------
#[derive(Debug)]
pub enum JournalError {
    IoError(String),
    FieldMissing(String), // ENODATA
    NotPositioned,        // EADDRNOTAVAIL
    Unavailable(String),
}
