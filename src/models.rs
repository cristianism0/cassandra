use clap::ValueEnum;
use std::io::Error;
use std::path::PathBuf;

use std::collections::HashMap;
use tabled::Tabled;

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
        columns: HashMap<TableKey, Vec<String>>,
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
#[derive(Debug, Tabled)]
pub struct SysRecord {
    //syslog or messages
    #[tabled(display("display_opt"))]
    pub priority: Option<String>, //filtered
    pub timestamp: String,
    pub host: String,
    pub process: String,
    pub message: String,
}

#[derive(Debug, Tabled)]
pub struct AuthRecord {
    //auth or secure
    #[tabled(display("display_opt"))]
    pub priority: Option<String>, //filtered
    pub timestamp: String,
    pub host: String,
    pub process: String,
    #[tabled(display("display_opt"))]
    pub caller: Option<String>,
    pub message: String,
}

#[derive(Debug, Tabled)]
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

#[derive(Debug, Tabled)]
pub struct JournalRecord {
    pub message: String,
    #[tabled(display("display_opt"))]
    pub priority: Option<String>,
    #[tabled(display("display_opt"))]
    pub code_file: Option<String>,
    #[tabled(display("display_opt"))]
    pub code_func: Option<String>,
    #[tabled(display("display_opt"))]
    pub code_line: Option<String>,
    #[tabled(display("display_opt"))]
    pub syslog_facility: Option<String>,
    #[tabled(display("display_opt"))]
    pub syslog_identifier: Option<String>,
    #[tabled(display("display_opt"))]
    pub tid: Option<String>,
    #[tabled(display("display_opt"))]
    pub _audit_loginuid: Option<String>,
    #[tabled(display("display_opt"))]
    pub _audit_session: Option<String>,
    #[tabled(display("display_opt"))]
    pub _boot_id: Option<String>,
    #[tabled(display("display_opt"))]
    pub _gid: Option<String>,
    #[tabled(display("display_opt"))]
    pub _hostname: Option<String>,
    #[tabled(display("display_opt"))]
    pub _machine_id: Option<String>,
    #[tabled(display("display_opt"))]
    pub _pid: Option<String>,
    #[tabled(display("display_opt"))]
    pub _runtime_scope: Option<String>,
    #[tabled(display("display_opt"))]
    pub _selinux_context: Option<String>,
    #[tabled(display("display_opt"))]
    pub _source_monotonic_timestamp: Option<String>,
    #[tabled(display("display_opt"))]
    pub _source_boottime_timestamp: Option<String>,
    #[tabled(display("display_opt"))]
    pub _source_realtime_timestamp: Option<String>,
    #[tabled(display("display_opt"))]
    pub _systemd_cgroup: Option<String>,
    #[tabled(display("display_opt"))]
    pub _systemd_owner_uid: Option<String>,
    #[tabled(display("display_opt"))]
    pub _systemd_slice: Option<String>,
    #[tabled(display("display_opt"))]
    pub _systemd_unit: Option<String>,
    #[tabled(display("display_opt"))]
    pub _systemd_user_slice: Option<String>,
    #[tabled(display("display_opt"))]
    pub _transport: Option<String>,
    #[tabled(display("display_opt"))]
    pub _uid: Option<String>,
}

fn display_opt(opt: &Option<String>) -> String {
    opt.as_deref().unwrap_or("-").to_string()
}

#[derive(Debug, Tabled)]
pub enum LogEntry {
    Sys(#[tabled(inline)] SysRecord),
    Auth(#[tabled(inline)] AuthRecord),
    Wtmp(#[tabled(inline)] WtmpRecord),
    Journal(#[tabled(inline)] JournalRecord),
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
