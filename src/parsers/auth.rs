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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static CTR: AtomicU64 = AtomicU64::new(0);

    fn write_tmp(name: &str, contents: &str) -> std::path::PathBuf {
        let id = CTR.fetch_add(1, Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!(
            "cassandra-auth-{}-{id}-{name}",
            std::process::id()
        ));
        std::fs::write(&p, contents).expect("write tmp fixture");
        p
    }

    #[test]
    fn parse_line_without_caller() {
        let r = parse_re(&AUTH_RE, "Oct 11 22:14:15 myhost sshd[1234]: Accepted password")
            .expect("should parse");
        assert_eq!(r.host, "myhost");
        assert_eq!(r.process, "sshd");
        assert_eq!(r.message, "Accepted password");
    }

    #[test]
    fn parse_line_with_simple_caller() {
        let r = parse_re(
            &AUTH_RE,
            "Oct 11 22:14:15 myhost sshd[1234]: callername: the message",
        )
        .expect("should parse");
        assert_eq!(r.caller.as_deref(), Some("callername"));
        assert_eq!(r.message, "the message");
    }

    #[test]
    fn parse_line_with_pri() {
        let r = parse_re(
            &AUTH_RE,
            "<38>Oct 11 22:14:15 myhost login[99]: FAILED LOGIN",
        )
        .expect("should parse");
        assert_eq!(r.priority.as_deref(), Some("38"));
        assert_eq!(r.process, "login");
    }

    #[test]
    fn parse_invalid_returns_none() {
        assert!(parse_re(&AUTH_RE, "garbage").is_none());
        assert!(parse_re(&AUTH_RE, "").is_none());
    }

    #[test]
    fn parser_reads_all_lines_in_order() {
        let p = write_tmp(
            "all.log",
            "Oct 11 22:14:15 h sshd[1]: first\nOct 11 22:14:16 h sshd[1]: second\n",
        );
        let out = AuthLog.parser(&p, None, false).expect("parse ok");
        let _ = std::fs::remove_file(&p);
        assert_eq!(out.len(), 2);
        match (&out[0], &out[1]) {
            (LogEntry::Auth(a), LogEntry::Auth(b)) => {
                assert_eq!(a.message, "first");
                assert_eq!(b.message, "second");
            }
            _ => panic!("expected auth entries"),
        }
    }

    #[test]
    fn parser_lines_keeps_last_n() {
        let p = write_tmp(
            "last.log",
            "Oct 11 22:14:15 h p: one\nOct 11 22:14:16 h p: two\nOct 11 22:14:17 h p: three\n",
        );
        let out = AuthLog.parser(&p, Some(2), false).expect("parse ok");
        let _ = std::fs::remove_file(&p);
        assert_eq!(out.len(), 2);
        match (&out[0], &out[1]) {
            (LogEntry::Auth(a), LogEntry::Auth(b)) => {
                assert_eq!(a.message, "two");
                assert_eq!(b.message, "three");
            }
            _ => panic!("expected auth entries"),
        }
    }

    #[test]
    fn parser_lines_zero_returns_empty() {
        let p = write_tmp("zero.log", "Oct 11 22:14:15 h p: one\n");
        let out = AuthLog.parser(&p, Some(0), false).expect("parse ok");
        let _ = std::fs::remove_file(&p);
        assert!(out.is_empty());
    }

    #[test]
    fn parser_reverse_flips_order() {
        let p = write_tmp(
            "rev.log",
            "Oct 11 22:14:15 h p: one\nOct 11 22:14:16 h p: two\n",
        );
        let out = AuthLog.parser(&p, None, true).expect("parse ok");
        let _ = std::fs::remove_file(&p);
        match (&out[0], &out[1]) {
            (LogEntry::Auth(a), LogEntry::Auth(b)) => {
                assert_eq!(a.message, "two");
                assert_eq!(b.message, "one");
            }
            _ => panic!("expected auth entries"),
        }
    }

    #[test]
    fn parser_missing_file_is_io_error() {
        let missing = std::env::temp_dir().join("cassandra-auth-definitely-missing.log");
        let _ = std::fs::remove_file(&missing);
        match AuthLog.parser(&missing, None, false) {
            Err(ParseError::IoError(_)) => {}
            other => panic!("expected IoError, got {other:?}"),
        }
    }

    #[test]
    #[should_panic(expected = "Cannot get the information line")]
    fn parser_panics_on_malformed_line_documents_current_behavior() {
        let p = write_tmp("bad.log", "Oct 11 22:14:15 h p: ok\nNOT A VALID LINE\n");
        let _ = AuthLog.parser(&p, None, false);
        let _ = std::fs::remove_file(&p);
    }
}
