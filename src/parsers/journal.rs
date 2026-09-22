use crate::{
    models::{JournalError, JournalRecord, LogEntry},
    parsers::selector::JournalParser,
};
use systemd::journal::Journal;

pub struct JournalLog;

impl JournalParser for JournalLog {
    fn parser(
        &self,
        journal: &mut Journal,
        lines: Option<u64>,
        reverse: bool,
        since_usec: Option<u64>,
        until_usec: Option<u64>,
    ) -> Result<Vec<LogEntry>, JournalError> {
        if let Some(0) = lines {
            return Ok(Vec::new());
        }

        let mut entries = Vec::new();

        if let Some(since) = since_usec {
            // Native seek to since time
            journal.seek_realtime_usec(since).map_err(|e| {
                JournalError::IoError(format!(
                    "Cannot seek journal to since time due to error: {e:#?}"
                ))
            })?;
            // After seek, next_entry will return first entry >= since
            while journal
                .next_entry()
                .map_err(|e| {
                    JournalError::IoError(format!(
                        "An error ocurred during the journal lines parsing: {e:#?}"
                    ))
                })?
                .is_some()
            {
                // Check until
                let ts = journal.timestamp_usec().map_err(|e| {
                    JournalError::IoError(format!("Cannot get journal timestamp: {e:#?}"))
                })?;
                if let Some(until) = until_usec
                    && ts > until
                {
                    break;
                }
                let record = extract_record(journal);
                entries.push(LogEntry::Journal(Box::new(record)));
                // Early break if we have collected enough and lines is set and we want only first N?
                // For now collect all within range, then apply lines limit after
            }
            // Apply lines limit as last N in chronological order within time range
            if let Some(n) = lines {
                let n_usize = n as usize;
                if entries.len() > n_usize {
                    let skip = entries.len() - n_usize;
                    entries = entries.into_iter().skip(skip).collect();
                }
            }
            if reverse {
                entries.reverse();
            }
        } else {
            // No since: use tail logic, but still need to handle until
            journal.seek_tail().map_err(|e| {
                JournalError::IoError(format!(
                    "Cannot read the journal during the parser due to error: {e:#?}"
                ))
            })?;

            if let Some(until) = until_usec {
                // When until is set without since, we need to consider entries before until
                // Approach: seek to until time, then collect previous entries? But simpler: collect last N then filter by until
                // For now, collect last N (or all if lines None with default 50) then filter
                let limit = lines.unwrap_or(50);
                journal.previous_skip(limit).map_err(|e| {
                    JournalError::IoError(format!(
                        "Cannot read the journal's lines during the parser due to error: {e:#?}"
                    ))
                })?;
                while journal
                    .next_entry()
                    .map_err(|e| {
                        JournalError::IoError(format!(
                            "An error ocurred during the journal lines parsing: {e:#?}"
                        ))
                    })?
                    .is_some()
                {
                    let ts = journal.timestamp_usec().map_err(|e| {
                        JournalError::IoError(format!("Cannot get journal timestamp: {e:#?}"))
                    })?;
                    if ts > until {
                        break;
                    }
                    let record = extract_record(journal);
                    entries.push(LogEntry::Journal(Box::new(record)));
                }
                if reverse {
                    entries.reverse();
                }
            } else {
                let limit = lines.unwrap_or(50);
                journal.previous_skip(limit).map_err(|e| {
                    JournalError::IoError(format!(
                        "Cannot read the journal's lines during the parser due to error: {e:#?}"
                    ))
                })?;

                while journal
                    .next_entry()
                    .map_err(|e| {
                        JournalError::IoError(format!(
                            "An error ocurred during the journal lines parsing: {e:#?}"
                        ))
                    })?
                    .is_some()
                {
                    let record = extract_record(journal);
                    entries.push(LogEntry::Journal(Box::new(record)));
                }

                if reverse {
                    entries.reverse();
                }
            }
        }

        Ok(entries)
    }
}

fn extract_record(journal: &mut Journal) -> JournalRecord {
    JournalRecord {
        message: extract_field(journal, "MESSAGE").unwrap_or_default(),
        priority: extract_field(journal, "PRIORITY"),
        code_file: extract_field(journal, "CODE_FILE"),
        code_func: extract_field(journal, "CODE_FUNC"),
        code_line: extract_field(journal, "CODE_LINE"),
        syslog_facility: extract_field(journal, "SYSLOG_FACILITY"),
        syslog_identifier: extract_field(journal, "SYSLOG_IDENTIFIER"),
        tid: extract_field(journal, "TID"),
        audit_loginuid: extract_field(journal, "_AUDIT_LOGINUID"),
        audit_session: extract_field(journal, "_AUDIT_SESSION"),
        boot_id: extract_field(journal, "_BOOT_ID"),
        gid: extract_field(journal, "_GID"),
        hostname: extract_field(journal, "_HOSTNAME"),
        machine_id: extract_field(journal, "_MACHINE_ID"),
        pid: extract_field(journal, "_PID"),
        runtime_scope: extract_field(journal, "_RUNTIME_SCOPE"),
        selinux_context: extract_field(journal, "_SELINUX_CONTEXT"),
        source_monotonic_timestamp: extract_field(journal, "_SOURCE_MONOTONIC_TIMESTAMP"),
        source_boottime_timestamp: extract_field(journal, "_SOURCE_BOOTTIME_TIMESTAMP"),
        source_realtime_timestamp: extract_field(journal, "_SOURCE_REALTIME_TIMESTAMP"),
        systemd_cgroup: extract_field(journal, "_SYSTEMD_CGROUP"),
        systemd_owner_uid: extract_field(journal, "_SYSTEMD_OWNER_UID"),
        systemd_slice: extract_field(journal, "_SYSTEMD_SLICE"),
        systemd_unit: extract_field(journal, "_SYSTEMD_UNIT"),
        systemd_user_slice: extract_field(journal, "_SYSTEMD_USER_SLICE"),
        transport: extract_field(journal, "_TRANSPORT"),
        uid: extract_field(journal, "_UID"),
    }
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
