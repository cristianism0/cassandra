use crate::models::{LogEntry, sys::SysRecord};
use crate::parsers::{ParseError, selector::LogParser};

use std::{
    collections::VecDeque,
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use crate::parsers::SYS_RE;
use regex::Regex;

pub struct SysLog;

impl LogParser for SysLog {
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

        let mut bufread = BufReader::new(f);
        let mut bufline = String::new();

        let limit = lines.map(|n| {
            usize::try_from(n).map_err(|e| {
                ParseError::IoError(format!(
                    "Could not convert the number of lines at {} due to: {e}",
                    path.display()
                ))
            })
        });

        let mut deque = match &limit {
            Some(val) => {
                let m = match val.as_ref() {
                    Ok(n) => *n,
                    _ => 0,
                };

                VecDeque::with_capacity(m)
            }
            None => VecDeque::new(),
        };

        while bufread.read_line(&mut bufline).map_err(|e| {
            ParseError::MalformedLine(format!(
                "File with malformed line was found while reading log file at {}: {e}",
                path.display()
            ))
        })? > 0
        {
            let entry = LogEntry::Sys(
                parse_re(&SYS_RE, bufline.trim_end())
                    .expect("Cannot get the information due to bad regex match."),
            );

            if let Some(n) = &limit
                && deque.len() == *n.as_ref().expect("asd")
            {
                deque.pop_front();
            }

            deque.push_back(entry);
            bufline.clear();
        }

        let mut entries = Vec::with_capacity(deque.len());

        if reverse {
            entries.extend(deque.into_iter().rev());
        } else {
            entries.extend(deque);
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
