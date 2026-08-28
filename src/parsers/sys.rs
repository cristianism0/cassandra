use crate::models::{LogEntry, ParseError, SysRecord};
use crate::parsers::SYS_RE;
use crate::parsers::selector::LogParser;
use regex::Regex;

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

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

        let sys_pattern = &*SYS_RE;

        // Lazy tail: if `lines` is Some(n), seek to start of last n lines instead of scanning whole file.
        if let Some(n) = lines.map(|n| n as usize) {
            let mut f = File::open(path).map_err(|e| {
                ParseError::IoError(format!("Cannot open file at {path:#?} due to: {e}"))
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
                    // First line after seek may be partial — discard.
                    first = false;
                    bufl.clear();
                    continue;
                }
                // TODO: when `search` is wired, filter here before regex: if !bufl.contains(search) { bufl.clear(); continue; }
                if let Some(rec) = parse_re(sys_pattern, bufl.trim_end()) {
                    entries.push(LogEntry::Sys(rec));
                }
                bufl.clear();
            }
            if reverse {
                entries.reverse();
            }
            return Ok(entries);
        }

        let f = File::open(path).map_err(|e| {
            ParseError::IoError(format!("Cannot open file at {path:#?} due to: {e}"))
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
            // TODO: filter by `search` here before parsing to avoid regex cost
            if let Some(rec) = parse_re(sys_pattern, bufl.trim_end()) {
                entries.push(LogEntry::Sys(rec));
            }
            bufl.clear();
        }

        if reverse {
            entries.reverse();
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
