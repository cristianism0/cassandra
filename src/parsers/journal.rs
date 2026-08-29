use crate::{
    models::{JournalError, JournalRecord, LogEntry},
    parsers::selector::JournalParser,
};
use systemd::journal::Journal;

pub struct JournalLog;

impl JournalParser for JournalLog {
    fn parser(&self, journal: &mut Journal, lines: Option<u64>, reverse: bool) -> Result<Vec<LogEntry>, JournalError> {
        if let Some(0) = lines {
            return Ok(Vec::new());
        }

        let mut entries = match lines {
            Some(n) => Vec::with_capacity(n as usize),
            None => Vec::with_capacity(1024),
        };

        if reverse {
            journal.seek_tail()
                .map_err(|e| {JournalError::IoError(format!("Cannot read the journal during the parser due to: {e:#?}"))})?;

            // No point of return for `lines=None` — default to 1000 to avoid blocking on large journal
            let mut remaining = lines.or(Some(1000));
            loop {
                match journal
                    .previous_entry()
                    .map_err(|e| JournalError::IoError(format!("An error ocurred during the journal lines parsing: {e:#?}")))?
                {
                    Some(_) => {
                        match remaining {
                            Some(0) => break,
                            Some(n) => remaining = Some(n - 1),
                            None => {}
                        }

                        let record = extract_record(journal).map_err(|e| {
                            JournalError::FieldMissing(format!("Could not extract the record from journal line due to: {e:#?}"))
                        })?;
                        entries.push(LogEntry::Journal(Box::new(record)));
                    }
                    None => break,
                }
            }
        } else {
            if let Some(n) = lines {
                journal.seek_tail()
                    .map_err(|e| {JournalError::IoError(format!("Cannot read the journal during the parser due to: {e:#?}"))})?;
                // previous_skip(n) positions so that next_entry will yield last n entries.
                // Bounded loop guarantees return even if n > total (returns total).
                journal
                    .previous_skip(n)
                    .map_err(|e| JournalError::IoError(format!("Cannot read the journal's lines during the parser due to: {e:#?}")))?;
                let mut remaining = n;
                while remaining > 0
                    && journal
                        .next_entry()
                        .map_err(|e| JournalError::IoError(format!("An error ocurred during the journal lines parsing: {e:#?}")))?
                        .is_some()
                {
                    let record = extract_record(journal).map_err(|e| {
                        JournalError::FieldMissing(format!("Could not extract the record from journal line due to: {e:#?}"))
                    })?;
                    entries.push(LogEntry::Journal(Box::new(record)));
                    remaining -= 1;
                }
            } else {
                // No point of return — `lunete journal` without `lines` would read entire journal and block.
                // Put sane default (last 1000) to keep CLI responsive; TUI will stream lazy windowed via tokio mpsc.
                journal.seek_tail()
                    .map_err(|e| {JournalError::IoError(format!("Cannot read the journal during the parser due to: {e:#?}"))})?;
                journal
                    .previous_skip(1000)
                    .map_err(|e| JournalError::IoError(format!("Cannot read the journal's lines during the parser due to: {e:#?}")))?;
                while journal
                    .next_entry()
                    .map_err(|e| JournalError::IoError(format!("An error ocurred during the journal lines parsing: {e:#?}")))?
                    .is_some()
                {
                    let record = extract_record(journal).map_err(|e| {
                        JournalError::FieldMissing(format!("Could not extract the record from journal line due to: {e:#?}"))
                    })?;
                    entries.push(LogEntry::Journal(Box::new(record)));
                }
            }
        }

        Ok(entries)
    }
}

fn extract_record(journal: &mut Journal) -> Result<JournalRecord, JournalError> {
    //mount the struct
    Ok(JournalRecord {
        message: extract_field(journal, "MESSAGE").unwrap_or_default(),
        priority: extract_field(journal, "PRIORITY"),
        code_file: extract_field(journal, "CODE_FILE"),
        code_func: extract_field(journal, "CODE_FUNC"),
        code_line: extract_field(journal, "CODE_LINE"),
        syslog_facility: extract_field(journal, "SYSLOG_FACILITY"),
        syslog_identifier: extract_field(journal, "SYSLOG_IDENTIFIER"),
        tid: extract_field(journal, "TID"),
        _audit_loginuid: extract_field(journal, "_AUDIT_LOGINUID"),
        _audit_session: extract_field(journal, "_AUDIT_SESSION"),
        _boot_id: extract_field(journal, "_BOOT_ID"),
        _gid: extract_field(journal, "_GID"),
        _hostname: extract_field(journal, "_HOSTNAME"),
        _machine_id: extract_field(journal, "_MACHINE_ID"),
        _pid: extract_field(journal, "_PID"),
        _runtime_scope: extract_field(journal, "_RUNTIME_SCOPE"),
        _selinux_context: extract_field(journal, "_SELINUX_CONTEXT"),
        _source_monotonic_timestamp: extract_field(journal, "_SOURCE_MONOTONIC_TIMESTAMP"),
        _source_boottime_timestamp: extract_field(journal, "_SOURCE_BOOTTIME_TIMESTAMP"),
        _source_realtime_timestamp: extract_field(journal, "_SOURCE_REALTIME_TIMESTAMP"),
        _systemd_cgroup: extract_field(journal, "_SYSTEMD_CGROUP"),
        _systemd_owner_uid: extract_field(journal, "_SYSTEMD_OWNER_UID"),
        _systemd_slice: extract_field(journal, "_SYSTEMD_SLICE"),
        _systemd_unit: extract_field(journal, "_SYSTEMD_UNIT"),
        _systemd_user_slice: extract_field(journal, "_SYSTEMD_USER_SLICE"),
        _transport: extract_field(journal, "_TRANSPORT"),
        _uid: extract_field(journal, "_UID"),
    })
}

fn extract_field(journal: &mut Journal, name: &'static str) -> Option<String> {
    journal
        .get_data(name)
        .ok()
        .flatten()
        .map(|data| bytes_to_string(data.value().unwrap()))
}

fn bytes_to_string(raw: &[u8]) -> String {
    String::from_utf8_lossy(raw).into_owned()
}
