use crate::models::{FromLogEntry, LogEntry};

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

impl FromLogEntry for AuthRecord {
    fn from_entry(entry: &LogEntry) -> Option<&Self> {
        match entry {
            LogEntry::Auth(r) => Some(r),
            _ => None,
        }
    }
}
