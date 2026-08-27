use crate::models::{LogEntry, ParseError, SysRecord};
use crate::parsers::selector::LogParser;

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::{path::Path, collections::VecDeque};

use regex::Regex;
pub struct SysLog;

impl LogParser for SysLog {
    fn parser(&self, path: &Path, lines: Option<u64>, reverse: bool) -> Result<Vec<LogEntry>, ParseError> {
	if let Some(0) = lines {
	    return Ok(Vec::new());
	}

	let sys_pattern = Regex::new(r"^(?:<(?P<pri>\d+)>)?(?P<timestamp>(?P<month>[A-Za-z]{3})\s+(?P<day>\d{1,2})\s+(?P<time>\d{2}:\d{2}:\d{2}))\s+(?P<host>\S+)\s+(?P<process>[^\[:]+)(?:\[(?P<pid>\d+)\])?:\s*(?:(?P<caller>[^:]+):\s*)?(?P<msg>.*)$").unwrap();

	let f = File::open(path).map_err(|e| {
	    ParseError::IoError(format!("Cannot open file at {path:#?} due to: {e}"))
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
	    let entry = LogEntry::Sys(
		parse_re(&sys_pattern, bufl.trim_end()).expect("Cannot get the information line due to bad regex match.")
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
