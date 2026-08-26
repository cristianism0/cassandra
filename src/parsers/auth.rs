use crate::models::{AuthRecord, LogEntry, ParseError};
use crate::parsers::selector::LogParser;
use regex::Regex;
use std::io::{BufRead, BufReader};
use std::{fs::File, path::Path};

pub struct AuthLog;

// Structure for Auth and Sys follow RFC 3164:
// PRI HEADER MSG
// but, for a better readability most linus system ommit the priority
// if you have priority available (after change the rsyslog.conf) the regex will capture and
// display.

impl LogParser for AuthLog {
    fn parser(&self, path: &Path) -> Result<Vec<LogEntry>, ParseError> {
        let sec_pattern = Regex::new(r"^(?:<(?P<pri>\d+)>)?(?P<timestamp>(?P<month>[A-Za-z]{3})\s+(?P<day>\d{1,2})\s+(?P<time>\d{2}:\d{2}:\d{2}))\s+(?P<host>\S+)\s+(?P<process>[^\[:]+)(?:\[(?P<pid>\d+)\])?:\s*(?:(?P<caller>[^:]+):\s*)?(?P<msg>.*)$").unwrap();

        let f = File::open(path).map_err(|e| {
            ParseError::IoError(format!("Cannot open file at {path:#?} due to error: {e}"))
        })?;

        let mut bufr = BufReader::new(f);
        let mut bufl = String::new();

        let mut entries: Vec<LogEntry> = Vec::new();
        while bufr.read_line(&mut bufl).map_err(|e| {
            ParseError::MalformedLine(format!(
                "Error ocurred during file reading at {path:#?}: {e}"))
        })? > 0
        {
            let trimmed_bufl = bufl.trim_end();
            entries.push(LogEntry::Auth(
                parse_re(&sec_pattern, trimmed_bufl).expect("Cannot get the information line."),
            ));
            bufl.clear();
        }
        Ok(entries)
    }
}

fn parse_re(pattern: &Regex, raw: &str) -> Option<AuthRecord> {
    let cap = pattern.captures(raw)?;

    Some(AuthRecord {
        priority: cap.name("pri").map(|m| m.as_str().to_string()),
        timestamp: format!(
            "{} {} {}",
            cap.name("month")?.as_str(),
            cap.name("day")?.as_str(),
            cap.name("time")?.as_str()
        ),
        host: cap.name("host")?.as_str().to_string(),
        process: cap.name("process")?.as_str().to_string(),
        caller: cap.name("caller").map(|m| m.as_str().to_string()),
        message: cap.name("msg")?.as_str().to_string(),
    })
}
