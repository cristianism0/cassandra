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
    fn try_iter(
        &self,
        path: &Path,
    ) -> Result<Box<dyn Iterator<Item = Result<LogEntry, ParseError>>>, ParseError> {
        let f = File::open(path).map_err(|e| {
            ParseError::IoError(format!(
                "Cannot open file at {} due to: {e}",
                path.display()
            ))
        })?;
        let reader = BufReader::new(f);
        let path_owned = path.to_path_buf();
        let iter = reader.lines().map(move |line_res| {
            let line = line_res.map_err(|e| {
                ParseError::MalformedLine(format!(
                    "File with malformed line was found while reading log file at {}: {e}",
                    path_owned.display()
                ))
            })?;
            let trimmed = line.trim_end();
            // For streaming `raw` we propagate per-line errors instead of panicking.
            // This allows `raw` to skip malformed lines with a warning.
            let rec = parse_re(&SYS_RE, trimmed).ok_or_else(|| {
                ParseError::MalformedLine(format!(
                    "Malformed syslog line at {}: '{trimmed}'",
                    path_owned.display()
                ))
            })?;
            Ok(LogEntry::Sys(rec))
        });
        Ok(Box::new(iter))
    }

    fn parser(
        &self,
        path: &Path,
        lines: Option<u64>,
        reverse: bool,
    ) -> Result<Vec<LogEntry>, ParseError> {
        if let Some(0) = lines {
            return Ok(Vec::new());
        }

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

        for res in self.try_iter(path)? {
            let entry = res.expect("Cannot get the information due to bad regex match.");

            if let Some(n) = &limit
                && deque.len() == *n.as_ref().expect("asd")
            {
                deque.pop_front();
            }

            deque.push_back(entry);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static CTR: AtomicU64 = AtomicU64::new(0);

    fn write_tmp(name: &str, contents: &str) -> std::path::PathBuf {
        let id = CTR.fetch_add(1, Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!("cassandra-sys-{}-{id}-{name}", std::process::id()));
        std::fs::write(&p, contents).expect("write tmp fixture");
        p
    }

    #[test]
    fn parse_standard_line_without_pri() {
        let r = parse_re(
            &SYS_RE,
            "Oct 11 22:14:15 myhost sshd[1234]: Accepted password for user",
        )
        .expect("should parse");
        assert_eq!(r.priority, None);
        assert_eq!(r.timestamp, "Oct 11 22:14:15");
        assert_eq!(r.host, "myhost");
        assert_eq!(r.process, "sshd");
        assert_eq!(r.message, "Accepted password for user");
    }

    #[test]
    fn parse_line_with_pri_and_pid() {
        let r = parse_re(
            &SYS_RE,
            "<34>Oct 11 22:14:15 myhost myproc[42]: hello world",
        )
        .expect("should parse");
        assert_eq!(r.priority.as_deref(), Some("34"));
        assert_eq!(r.process, "myproc");
        assert_eq!(r.message, "hello world");
    }

    #[test]
    fn parse_line_without_pid() {
        let r = parse_re(&SYS_RE, "Oct 11 22:14:15 myhost cron: job ran").expect("should parse");
        assert_eq!(r.process, "cron");
        assert_eq!(r.message, "job ran");
    }

    #[test]
    fn parse_double_space_day() {
        let r = parse_re(&SYS_RE, "Oct  5 08:00:01 myhost proc: msg").expect("should parse");
        assert_eq!(r.timestamp, "Oct 5 08:00:01");
    }

    #[test]
    fn parse_invalid_returns_none() {
        assert!(parse_re(&SYS_RE, "not a syslog line").is_none());
        assert!(parse_re(&SYS_RE, "").is_none());
        assert!(parse_re(&SYS_RE, "Oct 11 no-time myhost proc: x").is_none());
    }

    #[test]
    fn parser_reads_all_lines_in_order() {
        let p = write_tmp(
            "all.log",
            "Oct 11 22:14:15 h1 p1: first\nOct 11 22:14:16 h1 p1: second\n",
        );
        let out = SysLog.parser(&p, None, false).expect("parse ok");
        let _ = std::fs::remove_file(&p);
        assert_eq!(out.len(), 2);
        match (&out[0], &out[1]) {
            (LogEntry::Sys(a), LogEntry::Sys(b)) => {
                assert_eq!(a.message, "first");
                assert_eq!(b.message, "second");
            }
            _ => panic!("expected sys entries"),
        }
    }

    #[test]
    fn parser_lines_keeps_last_n() {
        let p = write_tmp(
            "last.log",
            "Oct 11 22:14:15 h p: one\nOct 11 22:14:16 h p: two\nOct 11 22:14:17 h p: three\n",
        );
        let out = SysLog.parser(&p, Some(2), false).expect("parse ok");
        let _ = std::fs::remove_file(&p);
        assert_eq!(out.len(), 2);
        match (&out[0], &out[1]) {
            (LogEntry::Sys(a), LogEntry::Sys(b)) => {
                assert_eq!(a.message, "two");
                assert_eq!(b.message, "three");
            }
            _ => panic!("expected sys entries"),
        }
    }

    #[test]
    fn parser_lines_zero_returns_empty() {
        let p = write_tmp("zero.log", "Oct 11 22:14:15 h p: one\n");
        let out = SysLog.parser(&p, Some(0), false).expect("parse ok");
        let _ = std::fs::remove_file(&p);
        assert!(out.is_empty());
    }

    #[test]
    fn parser_reverse_flips_order() {
        let p = write_tmp(
            "rev.log",
            "Oct 11 22:14:15 h p: one\nOct 11 22:14:16 h p: two\n",
        );
        let out = SysLog.parser(&p, None, true).expect("parse ok");
        let _ = std::fs::remove_file(&p);
        match (&out[0], &out[1]) {
            (LogEntry::Sys(a), LogEntry::Sys(b)) => {
                assert_eq!(a.message, "two");
                assert_eq!(b.message, "one");
            }
            _ => panic!("expected sys entries"),
        }
    }

    #[test]
    fn parser_missing_file_is_io_error() {
        let missing = std::env::temp_dir().join("cassandra-sys-definitely-missing.log");
        let _ = std::fs::remove_file(&missing);
        match SysLog.parser(&missing, None, false) {
            Err(ParseError::IoError(_)) => {}
            other => panic!("expected IoError, got {other:?}"),
        }
    }

    #[test]
    #[should_panic(expected = "Cannot get the information due to bad regex match")]
    fn parser_panics_on_malformed_line_documents_current_behavior() {
        let p = write_tmp(
            "bad.log",
            "Oct 11 22:14:15 h p: ok\nTHIS IS NOT A SYSLOG LINE\n",
        );
        let _ = SysLog.parser(&p, None, false);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn try_iter_streams_without_collecting_all() {
        let p = write_tmp(
            "iter.log",
            "Oct 11 22:14:15 h p: one\nOct 11 22:14:16 h p: two\nOct 11 22:14:17 h p: three\n",
        );
        let iter = SysLog.try_iter(&p).expect("iter ok");
        let mut count = 0;
        for res in iter {
            let e = res.expect("line ok");
            match e {
                LogEntry::Sys(r) => {
                    assert!(r.message == "one" || r.message == "two" || r.message == "three")
                }
                _ => panic!("expected sys"),
            }
            count += 1;
        }
        let _ = std::fs::remove_file(&p);
        assert_eq!(count, 3);
    }

    #[test]
    fn try_iter_yields_error_on_malformed_instead_of_panic() {
        let p = write_tmp(
            "iter-bad.log",
            "Oct 11 22:14:15 h p: ok\nBAD LINE\nOct 11 22:14:16 h p: ok2\n",
        );
        let iter = SysLog.try_iter(&p).expect("iter ok");
        let results: Vec<_> = iter.collect();
        let _ = std::fs::remove_file(&p);
        assert_eq!(results.len(), 3);
        assert!(results[0].is_ok());
        assert!(results[1].is_err());
        assert!(results[2].is_ok());
    }
}
