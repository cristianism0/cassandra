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
        _lines: Option<u64>,
        _reverse: bool,
    ) -> Result<Vec<LogEntry>, JournalError> {
        journal.seek_tail().map_err(|e| {
            JournalError::IoError(format!(
                "Cannot read the journal during the parser due to error: {e:#?}"
            ))
        })?;
        journal
            .previous_skip(50) // for now
            .map_err(|e| {
                JournalError::IoError(format!(
                    "Cannot read the journal's lines during the parser due to error: {e:#?}"
                ))
            })?;

        let mut entries = Vec::new();
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
        Ok(entries)
    }
}

fn extract_record(journal: &mut Journal) -> JournalRecord {
    //mount the struct
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
