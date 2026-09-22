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
    Raw,
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
            "audit_loginuid",
            "audit_session",
            "boot_id",
            "gid",
            "hostname",
            "machine_id",
            "pid",
            "runtime_scope",
            "selinux_context",
            "source_monotonic_timestamp",
            "source_boottime_timestamp",
            "source_realtime_timestamp",
            "systemd_cgroup",
            "systemd_owner_uid",
            "systemd_slice",
            "systemd_unit",
            "systemd_user_slice",
            "transport",
            "uid",
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
            display_opt(&self.audit_loginuid),
            display_opt(&self.audit_session),
            display_opt(&self.boot_id),
            display_opt(&self.gid),
            display_opt(&self.hostname),
            display_opt(&self.machine_id),
            display_opt(&self.pid),
            display_opt(&self.runtime_scope),
            display_opt(&self.selinux_context),
            display_opt(&self.source_monotonic_timestamp),
            display_opt(&self.source_boottime_timestamp),
            display_opt(&self.source_realtime_timestamp),
            display_opt(&self.systemd_cgroup),
            display_opt(&self.systemd_owner_uid),
            display_opt(&self.systemd_slice),
            display_opt(&self.systemd_unit),
            display_opt(&self.systemd_user_slice),
            display_opt(&self.transport),
            display_opt(&self.uid),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_opt_some_and_none() {
        assert_eq!(display_opt(&Some("x".to_string())), "x");
        assert_eq!(display_opt(&None), "-");
    }

    #[test]
    fn headers_match_fields_len_for_all_records() {
        let sys = SysRecord {
            priority: None,
            timestamp: "t".to_string(),
            host: "h".to_string(),
            process: "p".to_string(),
            message: "m".to_string(),
        };
        assert_eq!(SysRecord::headers().len(), sys.fields().len());

        let auth = AuthRecord {
            priority: None,
            timestamp: "t".to_string(),
            host: "h".to_string(),
            process: "p".to_string(),
            caller: None,
            message: "m".to_string(),
        };
        assert_eq!(AuthRecord::headers().len(), auth.fields().len());

        let wtmp = WtmpRecord {
            ut_type: 7,
            ut_pid: 1,
            ut_dname: "d".to_string(),
            ut_id: "i".to_string(),
            ut_user: "u".to_string(),
            ut_host: "h".to_string(),
            e_termination: 0,
            e_exit: 0,
        };
        assert_eq!(WtmpRecord::headers().len(), wtmp.fields().len());
    }

    #[test]
    fn none_options_render_as_dash() {
        let auth = AuthRecord {
            priority: None,
            timestamp: "t".to_string(),
            host: "h".to_string(),
            process: "p".to_string(),
            caller: None,
            message: "m".to_string(),
        };
        let fields = auth.fields();
        assert_eq!(fields[0], "-");
        assert_eq!(fields[4], "-");
    }
}
