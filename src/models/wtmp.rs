use crate::models::{FromLogEntry, LogEntry};

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

impl FromLogEntry for WtmpRecord {
    fn from_entry(entry: &LogEntry) -> Option<&Self> {
        match entry {
            LogEntry::Wtmp(r) => Some(r),
            _ => None,
        }
    }
}
