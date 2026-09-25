use std::path::Path;
use std::{fs::File, io::ErrorKind};
use systemd::journal::{Journal, OpenOptions};

use crate::models::{Finfo, LogEntry, LogSource, journal::JournalError, journal::JournalScope};
use crate::parsers::ParseError;
use crate::parsers::{auth::AuthLog, journal::JournalLog, sys::SysLog, wtmp::WtmpLog};

/// # Errors
/// Returns [`ParseError`] when the file cannot be opened or a line does not
/// follow the expected format.
pub fn parser_selector(
    file_info: &Finfo,
    lines: Option<u64>,
    reverse: bool,
) -> Result<Vec<LogEntry>, ParseError> {
    match file_info.source {
        LogSource::Sys => {
            let p = SysLog;
            p.check_access(&file_info.path)?;
            p.parser(&file_info.path, lines, reverse)
        }
        LogSource::Auth => {
            let p = AuthLog;
            p.check_access(&file_info.path)?;
            p.parser(&file_info.path, lines, reverse)
        }
        LogSource::Wtmp => {
            let p = WtmpLog;
            p.check_access(&file_info.path)?;
            p.parser(&file_info.path, lines, reverse)
        }
    }
}
/// # Errors
/// Returns [`JournalError`] when the journal socket cannot be opened or the
/// journal cannot be read.
pub fn journal_parsed(
    journal_scope: JournalScope,
    lines: Option<u64>,
    reverse: bool,
    since_usec: Option<u64>,
    until_usec: Option<u64>,
) -> Result<Vec<LogEntry>, JournalError> {
    let j = JournalLog;
    let mut jc = j.connect(journal_scope)?;
    let jentry = j.parser(&mut jc, lines, reverse, since_usec, until_usec)?;
    Ok(jentry)
}

pub trait LogParser {
    /// # Errors
    /// Returns [`ParseError`] when the file cannot be opened or read.
    fn try_iter(
        &self,
        path: &Path,
    ) -> Result<Box<dyn Iterator<Item = Result<LogEntry, ParseError>>>, ParseError>;

    /// # Errors
    /// Returns [`ParseError`] when the file cannot be opened or a line is malformed.
    fn parser(
        &self,
        path: &Path,
        lines: Option<u64>,
        reverse: bool,
    ) -> Result<Vec<LogEntry>, ParseError>;

    /// # Errors
    /// Returns [`ParseError::IoError`] when the path does not exist or cannot be opened.
    fn check_access(&self, path: &Path) -> Result<(), ParseError> {
        match File::open(path) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Err(ParseError::IoError(format!(
                "Path doesn't exists or was moved: {}",
                path.display()
            ))),
            Err(e) => Err(ParseError::IoError(format!(
                "Cannot open path {} due to error: {e}",
                path.display()
            ))),
        }
    }
}

pub trait JournalParser {
    /// # Errors
    /// Returns [`JournalError`] when the journal cannot be read.
    fn try_iter<'a>(
        &self,
        journal: &'a mut Journal,
        lines: Option<u64>,
        since_usec: Option<u64>,
        until_usec: Option<u64>,
    ) -> Result<Box<dyn Iterator<Item = Result<LogEntry, JournalError>> + 'a>, JournalError>;

    /// # Errors
    /// Returns [`JournalError::Unavailable`] when the journal socket cannot be opened.
    fn connect(&self, scope: JournalScope) -> Result<Journal, JournalError> {
        let mut opts = OpenOptions::default();
        opts.local_only(true).runtime_only(false);
        match scope {
            JournalScope::System => {
                opts.system(true);
            }
            JournalScope::User => {
                opts.current_user(true);
            }
        }
        opts.open().map_err(|e| {
            JournalError::Unavailable(format!("Cannot connect the journal socket due to: {e}"))
        })
    }

    /// # Errors
    /// Returns [`JournalError`] when the journal cannot be read.
    fn parser(
        &self,
        journal: &mut Journal,
        lines: Option<u64>,
        reverse: bool,
        since_usec: Option<u64>,
        until_usec: Option<u64>,
    ) -> Result<Vec<LogEntry>, JournalError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Finfo, PathStatus};
    use std::sync::atomic::{AtomicU64, Ordering};

    static CTR: AtomicU64 = AtomicU64::new(0);

    fn write_tmp(name: &str, contents: &str) -> std::path::PathBuf {
        let id = CTR.fetch_add(1, Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!(
            "cassandra-selector-{}-{id}-{name}",
            std::process::id()
        ));
        std::fs::write(&p, contents).expect("write tmp fixture");
        p
    }

    fn finfo_for(path: std::path::PathBuf, source: LogSource) -> Finfo {
        Finfo {
            path,
            source,
            pstatus: PathStatus::Found,
            data: None,
        }
    }

    #[test]
    fn selector_dispatches_sys() {
        let p = write_tmp("sys.log", "Oct 11 22:14:15 h proc: hello\n");
        let fi = finfo_for(p.clone(), LogSource::Sys);
        let out = parser_selector(&fi, None, false).expect("selector ok");
        let _ = std::fs::remove_file(&p);
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], LogEntry::Sys(_)));
    }

    #[test]
    fn selector_dispatches_auth() {
        let p = write_tmp("auth.log", "Oct 11 22:14:15 h sshd[1]: hello\n");
        let fi = finfo_for(p.clone(), LogSource::Auth);
        let out = parser_selector(&fi, None, false).expect("selector ok");
        let _ = std::fs::remove_file(&p);
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], LogEntry::Auth(_)));
    }

    #[test]
    fn selector_missing_file_is_io_error() {
        let fi = finfo_for(
            std::env::temp_dir().join("cassandra-selector-definitely-missing.log"),
            LogSource::Sys,
        );
        match parser_selector(&fi, None, false) {
            Err(ParseError::IoError(_)) => {}
            other => panic!("expected IoError, got {other:?}"),
        }
    }

    #[test]
    fn check_access_ok_and_missing() {
        let p = write_tmp("access.log", "Oct 11 22:14:15 h p: x\n");
        let parser = crate::parsers::sys::SysLog;
        assert!(parser.check_access(&p).is_ok());
        let _ = std::fs::remove_file(&p);
        let missing = std::env::temp_dir().join("cassandra-selector-access-missing.log");
        let _ = std::fs::remove_file(&missing);
        assert!(parser.check_access(&missing).is_err());
    }
}
