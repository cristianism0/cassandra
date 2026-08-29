pub mod auth;
pub mod journal;
pub mod selector;
pub mod sys;
pub mod wtmp;

pub use selector::JournalParser;

use regex::Regex;
use std::sync::LazyLock;

pub static SYS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:<(?P<pri>\d+)>)?(?P<timestamp>(?P<month>[A-Za-z]{3})\s+(?P<day>\d{1,2})\s+(?P<time>\d{2}:\d{2}:\d{2}))\s+(?P<host>\S+)\s+(?P<process>[^\[:]+)(?:\[(?P<pid>\d+)\])?:\s*(?:(?P<caller>[^:]+):\s*)?(?P<msg>.*)$").unwrap()
});

pub static AUTH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:<(?P<pri>\d+)>)?(?P<timestamp>(?P<month>[A-Za-z]{3})\s+(?P<day>\d{1,2})\s+(?P<time>\d{2}:\d{2}:\d{2}))\s+(?P<host>\S+)\s+(?P<process>[^\[:]+)(?:\[(?P<pid>\d+)\])?:\s*(?:(?P<caller>[^:]+):\s*)?(?P<msg>.*)$").unwrap()
});

#[derive(Debug)]
pub enum ParseError {
    IoError(String),
    MalformedLine(String),
    UnexpectedFormat(String),
}
