use clap::ValueEnum;
use std::io::Error;
use std::path::PathBuf;

//---------- Columns Presets ----------//
#[derive(Debug)]
pub struct JournalPreset {
    pub columns: &'static [&'static str],
    pub max_priority: u8,
}

pub const JOURNAL_CRITICAL: &[&str] = &[
    "message", "code_file", "_hostname", "_systemd_unit", "_selinux_context",
    "_pid", "_source_realtime_timestamp"
];

pub const JOURNAL_MEDIUM: &[&str] = &[
    "message","code_file", "code_func", "code_line", "_audit_loginuid",
    "_audit_session", "_selinux_context", "_pid", "tid", "_boot_id"
];

pub const JOURNAL_LOW: &[&str] = &[
    "message", "_transport", "syslog_facility", "_runtime_scope","_systemd_cgroup",
    "_systemd_user_slice", "_systemd_owner_uid", "_machine_id",
    "_source_monotonic_timestamp", "_source_boottime_timestamp"
];

pub trait TableDisplay {
    fn headers() -> Vec<&'static str>;
    fn fields(&self) -> Vec<String>;
}

// -------- Misc ---------
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogSource {
    Auth,
    Sys,
    Wtmp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ValueEnum)]
pub enum JournalScope {
    System,
    User,
}

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
    Compact {
        max_col_width: usize,
    },
    Summary {
        columns: Vec<String>,
    },
    KeyValue,
}

pub enum RecordType<'a> {
    Sys(&'a [LogEntry]),
    Auth(&'a [LogEntry]),
    Wtmp(&'a [LogEntry]),
    Journal(&'a [LogEntry], &'a JournalScope),
}

// ---------- Paths ----------
#[derive(Debug)]
pub struct SourceCandidate {
    pub source: LogSource,
    pub path: &'static str,
}

pub const SOURCES: &[SourceCandidate] = &[
    SourceCandidate {
        source: LogSource::Wtmp,
        path: "/var/log/wtmp",
    },
    // legacy - fallback for journald and openrc
    // rhel
    SourceCandidate {
        source: LogSource::Auth,
        path: "/var/log/secure",
    },
    SourceCandidate {
        source: LogSource::Sys,
        path: "/var/log/messages",
    },
    //debian
    SourceCandidate {
        source: LogSource::Auth,
        path: "/var/log/auth.log",
    },
    SourceCandidate {
        source: LogSource::Sys,
        path: "/var/log/syslog",
    },
];
// ---------- File Variants ----------
#[derive(Debug)]
pub enum FsKind {
    Regular,
    Dir,
    Symlink,
    Socket,
    Fifo,
    CharDevice,
    BlockDevice,
    Unknown,
}

#[derive(Debug)]
pub enum ContentFormat {
    PlainText,
    Json,
    Binary,
    Unknown,
}

// ---------- File Data Structures ----------
#[derive(Debug)]
pub struct Finfo {
    pub path: PathBuf,
    pub source: LogSource,
    pub pstatus: PathStatus,
    pub data: Option<FiData>,
}

#[derive(Debug)]
pub struct FiData {
    pub kind: FsKind,
    pub mode: u32,
    pub readable: bool,
    pub format: ContentFormat,
}

// ---------- Error Models ----------
#[derive(Debug)]
pub enum PathStatus {
    Found,
    NotFound,
    Indeterminate(Error),
}

#[derive(Debug)]
pub enum FileError {
    IoError(String),
    TraverseError(String),
}

#[derive(Debug)]
pub enum ParseError {
    IoError(String),
    MalformedLine(String),
    UnexpectedFormat(String),
}

#[derive(Debug)]
pub enum JournalError {
    IoError(String),
    FieldMissing(String), // ENODATA
    NotPositioned,        // EADDRNOTAVAIL
    Unavailable(String),
}

// ---------- Records ----------
#[derive(Debug)]
pub struct SysRecord {
    //syslog or messages
    pub priority: Option<String>, //filtered
    pub timestamp: String,
    pub host: String,
    pub process: String,
    pub message: String,
}

#[derive(Debug)]
pub struct AuthRecord {
    //auth or secure
    pub priority: Option<String>, //filtered
    pub timestamp: String,
    pub host: String,
    pub process: String,
    pub caller: Option<String>,
    pub message: String,
}

#[derive(Debug)]
pub struct WtmpRecord {
    pub ut_type: i16,
    pub ut_pid: i32,
    pub ut_dname: String,
    pub ut_id: String,
    pub ut_user: String,
    pub ut_host: String,
    pub e_termination: i16,
    pub e_exit: i16,
}

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

fn display_opt(opt: &Option<String>) -> String {
    opt.as_deref().unwrap_or("-").to_string()
}

#[derive(Debug)]
pub enum LogEntry {
    Sys(SysRecord),
    Auth(AuthRecord),
    Wtmp(WtmpRecord),
    Journal(Box<JournalRecord>),
}

pub trait FromLogEntry {
    fn from_entry(entry: &LogEntry) -> Option<&Self>;
}

impl FromLogEntry for SysRecord {
    fn from_entry(entry: &LogEntry) -> Option<&Self> {
        match entry {
            LogEntry::Sys(r) => Some(r),
            _ => None,
        }
    }
}

impl FromLogEntry for AuthRecord {
    fn from_entry(entry: &LogEntry) -> Option<&Self> {
        match entry {
            LogEntry::Auth(r) => Some(r),
            _ => None,
        }
    }
}

impl FromLogEntry for WtmpRecord {
    fn from_entry(entry: &LogEntry) -> Option<&Self> {
        match entry {
            LogEntry::Wtmp(r) => Some(r),
            _ => None,
        }
    }
}

impl FromLogEntry for JournalRecord {
    fn from_entry(entry: &LogEntry) -> Option<&Self> {
        match entry {
            LogEntry::Journal(r) => Some(r),
            _ => None,
        }
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

impl TableDisplay for AuthRecord {
    fn headers() -> Vec<&'static str> {
        vec!["priority", "timestamp", "host", "process", "caller", "message"]
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

impl TableDisplay for WtmpRecord {
    fn headers() -> Vec<&'static str> {
        vec!["ut_type", "ut_pid", "ut_dname", "ut_id", "ut_user", "ut_host", "e_termination", "e_exit"]
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

impl TableDisplay for JournalRecord {
    fn headers() -> Vec<&'static str> {
        vec![
            "message", "priority", "code_file", "code_func", "code_line",
            "syslog_facility", "syslog_identifier", "tid",
            "_audit_loginuid", "_audit_session", "_boot_id", "_gid",
            "_hostname", "_machine_id", "_pid", "_runtime_scope",
            "_selinux_context", "_source_monotonic_timestamp",
            "_source_boottime_timestamp", "_source_realtime_timestamp",
            "_systemd_cgroup", "_systemd_owner_uid", "_systemd_slice",
            "_systemd_unit", "_systemd_user_slice", "_transport", "_uid",
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
