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
    pub audit_loginuid: Option<String>,
    pub audit_session: Option<String>,
    pub boot_id: Option<String>,
    pub gid: Option<String>,
    pub hostname: Option<String>,
    pub machine_id: Option<String>,
    pub pid: Option<String>,
    pub runtime_scope: Option<String>,
    pub selinux_context: Option<String>,
    pub source_monotonic_timestamp: Option<String>,
    pub source_boottime_timestamp: Option<String>,
    pub source_realtime_timestamp: Option<String>,
    pub systemd_cgroup: Option<String>,
    pub systemd_owner_uid: Option<String>,
    pub systemd_slice: Option<String>,
    pub systemd_unit: Option<String>,
    pub systemd_user_slice: Option<String>,
    pub transport: Option<String>,
    pub uid: Option<String>,
}

#[derive(Debug)]
pub struct JournalPreset {
    pub columns: &'static [&'static str],
    pub max_priority: u8,
}

pub const JOURNAL_CRITICAL: &[&str] = &[
    "message",
    "code_file",
    "hostname",
    "systemd_unit",
    "selinux_context",
    "pid",
    "source_realtime_timestamp",
];

pub const JOURNAL_MEDIUM: &[&str] = &[
    "message",
    "code_file",
    "code_func",
    "code_line",
    "audit_loginuid",
    "audit_session",
    "selinux_context",
    "pid",
    "id",
    "boot_id",
];

pub const JOURNAL_LOW: &[&str] = &[
    "message",
    "transport",
    "syslog_facility",
    "runtime_scope",
    "systemd_cgroup",
    "systemd_user_slice",
    "systemd_owner_uid",
    "machine_id",
    "source_monotonic_timestamp",
    "source_boottime_timestamp",
];

pub const JOURNAL_KERNEL_COL: &[&str] = &[
    "MESSAGE",
    "PRIORITY",
    "SYSLOG_FACILITY",
    "SYSLOG_IDENTIFIER",
    "BOOT_ID",
    "HOSTNAME",
    "MACHINE_ID",
    "RUNTIME_SCOPE",
    "SOURCE_BOOTTIME_TIMESTAMP",
    "SOURCE_MONOTONIC_TIMESTAMP",
    "TRANSPORT",
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
