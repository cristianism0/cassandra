use crate::models::{AuthRecord, LogEntry, ParseError};
use crate::parsers::AUTH_RE;
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
    fn parser(
        &self,
        path: &Path,
        lines: Option<u64>,
        reverse: bool,
    ) -> Result<Vec<LogEntry>, ParseError> {
        if let Some(0) = lines {
            return Ok(Vec::new());
        }

        let sec_pattern = &*AUTH_RE;

        if let Some(n) = lines.map(|n| n as usize) {
            let mut f = File::open(path).map_err(|e| {
                ParseError::IoError(format!("Cannot open file at {path:?} due to: {e}"))
            })?;
            let offset = crate::parsers::find_tail_offset(&mut f, n).map_err(|e| {
                ParseError::IoError(format!("Cannot seek in file at {path:#?} due to: {e}"))
            })?;
            use std::io::Seek;
            f.seek(std::io::SeekFrom::Start(offset)).map_err(|e| {
                ParseError::IoError(format!("Cannot seek in file at {path:#?} due to: {e}"))
            })?;
            let mut bufr = BufReader::with_capacity(128 * 1024, f);
            let mut bufl = String::new();
            let mut entries = Vec::with_capacity(n);
            let mut first = offset != 0;
            while bufr.read_line(&mut bufl).map_err(|e| {
                ParseError::MalformedLine(format!(
                    "File with malformed line was found while reading log file at {path:?}: {e}"
                ))
            })? > 0
            {
                if first {
                    first = false;
                    bufl.clear();
                    continue;
                }
                if let Some(rec) = parse_re(sec_pattern, bufl.trim_end()) {
                    entries.push(LogEntry::Auth(rec));
                }
                bufl.clear();
            }
            if reverse {
                entries.reverse();
            }
            return Ok(entries);
        }

        let f = File::open(path).map_err(|e| {
            ParseError::IoError(format!("Cannot open file at {path:?} due to: {e}"))
        })?;

        let mut bufr = BufReader::with_capacity(128 * 1024, f);
        let mut bufl = String::new();
        let mut entries = Vec::new();

        while bufr.read_line(&mut bufl).map_err(|e| {
            ParseError::MalformedLine(format!(
                "File with malformed line was found while reading log file at {path:?}: {e}"
            ))
        })? > 0
        {
            if let Some(rec) = parse_re(sec_pattern, bufl.trim_end()) {
                entries.push(LogEntry::Auth(rec));
            }
            bufl.clear();
        }

        if reverse {
            entries.reverse();
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
