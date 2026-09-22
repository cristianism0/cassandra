use crate::{
    models::{JournalError, JournalRecord, LogEntry},
    parsers::selector::JournalParser,
};
use systemd::journal::Journal;

pub struct JournalLog;

impl JournalParser for JournalLog {
    fn try_iter<'a>(
        &self,
        journal: &'a mut Journal,
        lines: Option<u64>,
        since_usec: Option<u64>,
        until_usec: Option<u64>,
    ) -> Result<Box<dyn Iterator<Item = Result<LogEntry, JournalError>> + 'a>, JournalError> {
        if let Some(0) = lines {
            return Ok(Box::new(std::iter::empty()));
        }

        if let Some(since) = since_usec {
            journal.seek_realtime_usec(since).map_err(|e| {
                JournalError::IoError(format!(
                    "Cannot seek journal to since time due to error: {e:#?}"
                ))
            })?;
            let until = until_usec;
            // For `since` with `lines`, efficient last-N requires buffering all in range;
            // caller (raw/table) will handle `lines` via bounded VecDeque after filtering.
            let iter = std::iter::from_fn(move || {
                match journal.next_entry() {
                    Ok(Some(_)) => {
                        let ts = match journal.timestamp_usec() {
                            Ok(t) => t,
                            Err(e) => {
                                return Some(Err(JournalError::IoError(format!(
                                    "Cannot get journal timestamp: {e:#?}"
                                ))))
                            }
                        };
                        if let Some(u) = until
                            && ts > u
                        {
                            return None;
                        }
                        Some(Ok(LogEntry::Journal(Box::new(extract_record(journal)))))
                    }
                    Ok(None) => None,
                    Err(e) => Some(Err(JournalError::IoError(format!(
                        "An error ocurred during the journal lines parsing: {e:#?}"
                    )))),
                }
            });
            Ok(Box::new(iter))
        } else {
            // No `since`: use tail + previous_skip for efficient last-N
            journal.seek_tail().map_err(|e| {
                JournalError::IoError(format!(
                    "Cannot read the journal during the parser due to error: {e:#?}"
                ))
            })?;

            // If `until` is set, we still want last-N before `until`, not just last-N overall.
            // We use `previous_skip(limit)` from tail, then filter by `until` during iteration.
            let limit = lines.unwrap_or(50);
            // Only skip if limit > 0; journal.previous_skip(0) is a no-op but keep explicit
            if limit > 0 {
                journal.previous_skip(limit).map_err(|e| {
                    JournalError::IoError(format!(
                        "Cannot read the journal's lines during the parser due to error: {e:#?}"
                    ))
                })?;
            }
            let until = until_usec;
            let iter = std::iter::from_fn(move || {
                match journal.next_entry() {
                    Ok(Some(_)) => {
                        if let Some(u) = until {
                            let ts = match journal.timestamp_usec() {
                                Ok(t) => t,
                                Err(e) => {
                                    return Some(Err(JournalError::IoError(format!(
                                        "Cannot get journal timestamp: {e:#?}"
                                    ))))
                                }
                            };
                            if ts > u {
                                return None;
                            }
                        }
                        Some(Ok(LogEntry::Journal(Box::new(extract_record(journal)))))
                    }
                    Ok(None) => None,
                    Err(e) => Some(Err(JournalError::IoError(format!(
                        "An error ocurred during the journal lines parsing: {e:#?}"
                    )))),
                }
            });
            Ok(Box::new(iter))
        }
    }

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

        // `parser()` is now a thin wrapper over `try_iter()` for non-`since` case,
        // but keeps the `since + lines` last-N buffering (needs to read all in range).
        if since_usec.is_some() {
            // For `since`, `try_iter` iterates from `since`; we need to collect all then
            // keep last N to preserve `lines` semantics (last N in range).
            let iter = self.try_iter(journal, None, since_usec, until_usec)?;
            let mut entries: Vec<LogEntry> = iter.collect::<Result<Vec<_>, _>>()?;
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
            Ok(entries)
        } else {
            // Efficient path: `try_iter` already did `previous_skip(limit)` from tail
            let iter = self.try_iter(journal, lines, None, until_usec)?;
            let mut entries: Vec<LogEntry> = iter.collect::<Result<Vec<_>, _>>()?;
            if reverse {
                entries.reverse();
            }
            Ok(entries)
        }
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
