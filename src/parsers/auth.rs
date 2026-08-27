use crate::models::{AuthRecord, LogEntry, ParseError};
use crate::parsers::selector::LogParser;
use regex::Regex;
use std::io::{BufRead, BufReader};
use std::{fs::File, path::Path, collections::VecDeque};

pub struct AuthLog;

// Structure for Auth and Sys follow RFC 3164:
// PRI HEADER MSG
// but, for a better readability most linus system ommit the priority
// if you have priority available (after change the rsyslog.conf) the regex will capture and
// display.

impl LogParser for AuthLog {
    fn parser(&self, path: &Path, lines: Option<u64>, reverse: bool) -> Result<Vec<LogEntry>, ParseError> {
	if let Some(0) = lines {
	    return Ok(Vec::new());
	}

	let sec_pattern = Regex::new(r"^(?:<(?P<pri>\d+)>)?(?P<timestamp>(?P<month>[A-Za-z]{3})\s+(?P<day>\d{1,2})\s+(?P<time>\d{2}:\d{2}:\d{2}))\s+(?P<host>\S+)\s+(?P<process>[^\[:]+)(?:\[(?P<pid>\d+)\])?:\s*(?:(?P<caller>[^:]+):\s*)?(?P<msg>.*)$").unwrap();

	let f = File::open(path).map_err(|e| {
	    ParseError::IoError(format!("Cannot open file at {path:?} due to: {e}"))
	})?;

	let mut bufr = BufReader::new(f);
	let mut bufl = String::new();

	let limit = lines.map(|n| n as usize);
	let mut deque = match limit {
	    Some(n) => VecDeque::with_capacity(n),
	    None => VecDeque::new(),
	};

	while bufr.read_line(&mut bufl).map_err(|e| {
	    ParseError::MalformedLine(format!(
		"File with malformed line was found while reading log file at {path:?}: {e}"))
	})? > 0
	{
	    let entry = LogEntry::Auth(
		parse_re(&sec_pattern, bufl.trim_end()).expect("Cannot get the information line.")
	    );

	    if let Some(n) = limit {
		if deque.len() == n {
		    deque.pop_front();
		}
	    }

	    deque.push_back(entry);
	    bufl.clear();
	}

	let mut entries = Vec::with_capacity(deque.len());

	if reverse {
	    entries.extend(deque.into_iter().rev());
	} else {
	    entries.extend(deque.into_iter());
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
