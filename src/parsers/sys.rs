use crate::models::{LogEntry, ParseError, SysRecord};
use crate::parsers::selector::LogParser;

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use regex::Regex;
pub struct SysLog;

impl LogParser for SysLog {
    fn parser(&self, path: &Path) -> Result<Vec<LogEntry>, ParseError> {
        let sec_pattern = Regex::new(r"^(?:<(?P<pri>\d+)>)?(?P<timestamp>(?P<month>[A-Za-z]{3})\s+(?P<day>\d{1,2})\s+(?P<time>\d{2}:\d{2}:\d{2}))\s+(?P<host>\S+)\s+(?P<process>[^\[:]+)(?:\[(?P<pid>\d+)\])?:\s*(?:(?P<caller>[^:]+):\s*)?(?P<msg>.*)$").unwrap();

        let f = File::open(path).map_err(|e| {
            ParseError::IoError(format!("Cannot open file at {path:#?} due to: {e}"))
        })?;

        let mut bufr = BufReader::new(f);
        let mut bufl = String::new();

        let mut entries: Vec<LogEntry> = Vec::new();
        while bufr.read_line(&mut bufl).map_err(|e| {
            ParseError::MalformedLine(format!(
                "File with malformed line was found while reading log file at {path:?}: {e}"))
        })? > 0
        {
            let trimmed_bufl = bufl.trim_end();
            entries.push(LogEntry::Sys(parse_re(&sec_pattern, trimmed_bufl).expect("Cannot get the information line.")));
            bufl.clear();
        }
        Ok(entries)
    }
}

fn parse_re(pattern: &Regex, raw: &str) -> Option<SysRecord> {
    let cap = pattern.captures(raw)?;

    Some(SysRecord {
        priority: cap.name("pri").map(|m| m.as_str().to_string()),
        timestamp: format!(
            "{} {} {}",
            cap.name("month")?.as_str(),
            cap.name("day")?.as_str(),
            cap.name("time")?.as_str()
        ),
        host: cap.name("host")?.as_str().to_string(),
        process: cap.name("process")?.as_str().to_string(),
        message: cap.name("msg")?.as_str().to_string(),
    })
}
