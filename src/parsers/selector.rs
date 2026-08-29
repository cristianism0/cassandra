use std::path::Path;
use std::{fs::File, io::ErrorKind};
use systemd::journal::{Journal, OpenOptions};

use crate::models::{Finfo, LogEntry, LogSource, journal::JournalError, journal::JournalScope};
use crate::parsers::ParseError;
use crate::parsers::{auth::AuthLog, journal::JournalLog, sys::SysLog, wtmp::WtmpLog};

pub fn parser_selector(
    file_info: Finfo,
    lines: Option<u64>,
    reverse: bool,
) -> Result<Vec<LogEntry>, ParseError> {
    // TODO: wire search/gravity here — pass through to LogParser::parser as `search` arg
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

pub fn journal_parsed(
    journal_scope: JournalScope,
    lines: Option<u64>,
    reverse: bool,
) -> Result<Vec<LogEntry>, JournalError> {
    let j = JournalLog;
    let mut jc = j.connect(journal_scope)?;
    let jentry = j.parser(&mut jc, lines, reverse)?;
    Ok(jentry)
}

pub trait LogParser {
    fn parser(
        &self,
        path: &Path,
        lines: Option<u64>,
        reverse: bool,
    ) -> Result<Vec<LogEntry>, ParseError>;

    fn check_access(&self, path: &Path) -> Result<(), ParseError> {
        match File::open(path) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Err(ParseError::IoError(format!(
                "Path doesn't exists or was moved: {path:?}"
            ))),
            Err(e) => Err(ParseError::IoError(format!(
                "Cannot open path {path:?} due to error: {e}"
            ))),
        }
    }
}

pub trait JournalParser {
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
    fn parser(
        &self,
        journal: &mut Journal,
        lines: Option<u64>,
        reverse: bool,
    ) -> Result<Vec<LogEntry>, JournalError>;
}
