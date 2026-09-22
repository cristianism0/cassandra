use std::io::Error;
use std::path::PathBuf;

pub mod auth;
pub mod journal;
pub mod sys;
pub mod wtmp;

pub use auth::AuthRecord;
pub use journal::{
    JOURNAL_CRITICAL, JOURNAL_KERNEL_COL, JOURNAL_LOW, JOURNAL_MEDIUM, JournalError, JournalPreset,
    JournalRecord, JournalScope,
};
pub use sys::SysRecord;
pub use wtmp::WtmpRecord;

#[derive(Debug)]
pub enum LogEntry {
    Sys(SysRecord),
    Auth(AuthRecord),
    Wtmp(WtmpRecord),
    Journal(Box<JournalRecord>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogSource {
    Auth,
    Sys,
    Wtmp,
}

pub trait FromLogEntry {
    fn from_entry(entry: &LogEntry) -> Option<&Self>;
}

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
    SourceCandidate {
        source: LogSource::Auth,
        path: "/var/log/secure",
    },
    SourceCandidate {
        source: LogSource::Sys,
        path: "/var/log/messages",
    },
    SourceCandidate {
        source: LogSource::Auth,
        path: "/var/log/auth.log",
    },
    SourceCandidate {
        source: LogSource::Sys,
        path: "/var/log/syslog",
    },
];

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
