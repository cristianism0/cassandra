pub mod table_cli;
pub mod theme;

use crate::models::{
    LogEntry,
    auth::AuthRecord,
    journal::{JournalRecord, JournalScope},
    sys::SysRecord,
    wtmp::WtmpRecord,
};
use clap::ValueEnum;

pub use table_cli::{build_journal_table, build_table};
pub use theme::rose_pine_moon;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ValueEnum)]
pub enum TableKey {
    Sys,
    Auth,
    Wtmp,
    Journal,
}

#[derive(Debug)]
pub enum TableMode {
    Standard,
    Compact { max_col_width: usize },
    Summary { columns: Vec<String> },
    KeyValue,
}

pub enum RecordType<'a> {
    Sys(&'a [LogEntry]),
    Auth(&'a [LogEntry]),
    Wtmp(&'a [LogEntry]),
    Journal(&'a [LogEntry], &'a JournalScope),
}

pub trait TableDisplay {
    fn headers() -> Vec<&'static str>;
    fn fields(&self) -> Vec<String>;
}

pub fn display_opt(opt: &Option<String>) -> String {
    opt.as_deref().unwrap_or("-").to_string()
}

impl TableDisplay for AuthRecord {
    fn headers() -> Vec<&'static str> {
        vec![
            "priority",
            "timestamp",
            "host",
            "process",
            "caller",
            "message",
        ]
    }
    fn fields(&self) -> Vec<String> {
        vec![
            display_opt(&self.priority),
            self.timestamp.clone(),
            self.host.clone(),
            self.process.clone(),
            display_opt(&self.caller),
            self.message.clone(),
        ]
    }
}

impl TableDisplay for JournalRecord {
    fn headers() -> Vec<&'static str> {
        vec![
            "message",
            "priority",
            "code_file",
            "code_func",
            "code_line",
            "syslog_facility",
            "syslog_identifier",
            "tid",
            "_audit_loginuid",
            "_audit_session",
            "_boot_id",
            "_gid",
            "_hostname",
            "_machine_id",
            "_pid",
            "_runtime_scope",
            "_selinux_context",
            "_source_monotonic_timestamp",
            "_source_boottime_timestamp",
            "_source_realtime_timestamp",
            "_systemd_cgroup",
            "_systemd_owner_uid",
            "_systemd_slice",
            "_systemd_unit",
            "_systemd_user_slice",
            "_transport",
            "_uid",
        ]
    }
    fn fields(&self) -> Vec<String> {
        vec![
            self.message.clone(),
            display_opt(&self.priority),
            display_opt(&self.code_file),
            display_opt(&self.code_func),
            display_opt(&self.code_line),
            display_opt(&self.syslog_facility),
            display_opt(&self.syslog_identifier),
            display_opt(&self.tid),
            display_opt(&self._audit_loginuid),
            display_opt(&self._audit_session),
            display_opt(&self._boot_id),
            display_opt(&self._gid),
            display_opt(&self._hostname),
            display_opt(&self._machine_id),
            display_opt(&self._pid),
            display_opt(&self._runtime_scope),
            display_opt(&self._selinux_context),
            display_opt(&self._source_monotonic_timestamp),
            display_opt(&self._source_boottime_timestamp),
            display_opt(&self._source_realtime_timestamp),
            display_opt(&self._systemd_cgroup),
            display_opt(&self._systemd_owner_uid),
            display_opt(&self._systemd_slice),
            display_opt(&self._systemd_unit),
            display_opt(&self._systemd_user_slice),
            display_opt(&self._transport),
            display_opt(&self._uid),
        ]
    }
}

impl TableDisplay for SysRecord {
    fn headers() -> Vec<&'static str> {
        vec!["priority", "timestamp", "host", "process", "message"]
    }
    fn fields(&self) -> Vec<String> {
        vec![
            display_opt(&self.priority),
            self.timestamp.clone(),
            self.host.clone(),
            self.process.clone(),
            self.message.clone(),
        ]
    }
}

impl TableDisplay for WtmpRecord {
    fn headers() -> Vec<&'static str> {
        vec![
            "ut_type",
            "ut_pid",
            "ut_dname",
            "ut_id",
            "ut_user",
            "ut_host",
            "e_termination",
            "e_exit",
        ]
    }
    fn fields(&self) -> Vec<String> {
        vec![
            self.ut_type.to_string(),
            self.ut_pid.to_string(),
            self.ut_dname.clone(),
            self.ut_id.clone(),
            self.ut_user.clone(),
            self.ut_host.clone(),
            self.e_termination.to_string(),
            self.e_exit.to_string(),
        ]
    }
}
