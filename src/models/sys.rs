use crate::models::{FromLogEntry, LogEntry};

#[derive(Debug)]
pub struct SysRecord {
    pub priority: Option<String>, //filtered
    pub timestamp: String,
    pub host: String,
    pub process: String,
    pub message: String,
}

impl FromLogEntry for SysRecord {
    fn from_entry(entry: &LogEntry) -> Option<&Self> {
        match entry {
            LogEntry::Sys(r) => Some(r),
            _ => None,
        }
    }
}

