use crate::models::{LogEntry, auth::AuthRecord};
use crate::parsers::selector::LogParser;

use crate::parsers::ParseError;

use regex::Regex;
use std::io::{BufRead, BufReader};
use std::{collections::VecDeque, fs::File, path::Path};

use crate::parsers::AUTH_RE;

// Structure for Auth and Sys follow RFC 3164:
// PRI HEADER MSG
// but, for a better readability most linus system ommit the priority
// if you have priority available (after change the rsyslog.conf) the regex will capture and
// display.
//
pub struct AuthLog;
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

        let f = File::open(path).map_err(|e| {
            ParseError::IoError(format!(
                "Cannot open file at {} due to: {e}",
                path.display()
            ))
        })?;

        let mut bufr = BufReader::new(f);
        let mut bufl = String::new();

        let limit = lines.map(usize::try_from);
        let mut deque = match limit {
            Some(n) => VecDeque::with_capacity(n.map_err(|e| {
                ParseError::IoError(format!(
                    "Could not create the buffer to read the auth file at {} due to: {e}",
                    path.display()
                ))
            })?),
            None => VecDeque::new(),
        };

        while bufr.read_line(&mut bufl).map_err(|e| {
            ParseError::MalformedLine(format!(
                "File with malformed line was found while reading log file at {}: {e}",
                path.display()
            ))
        })? > 0
        {
            let entry = LogEntry::Auth(
                parse_re(&AUTH_RE, bufl.trim_end()).expect("Cannot get the information line."),
            );

            if let Some(n) = limit
                && deque.len()
                    == n.map_err(|e| {
                        ParseError::UnexpectedFormat(format!(
                            "Could not associated the exact line in the buffer at {} due to: {e}",
                            path.display()
                        ))
                    })?
            {
                deque.pop_front();
            }

            deque.push_back(entry);
            bufl.clear();
        }

        let mut entries = Vec::with_capacity(deque.len());

        if reverse {
            // TODO: remove the .rev(), extremelly bad for large files for parsing and reversing. pure CPU bound.
            entries.extend(deque.into_iter().rev());
        } else {
            entries.extend(deque);
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
