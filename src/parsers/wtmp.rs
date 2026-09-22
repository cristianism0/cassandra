use crate::parsers::{ParseError, selector::LogParser};
use std::io::{Read, Seek, SeekFrom};
use std::{fs::File, path::Path};

use crate::models::{LogEntry, wtmp::WtmpRecord};
pub struct WtmpLog;

impl LogParser for WtmpLog {
    fn try_iter(
        &self,
        path: &Path,
    ) -> Result<Box<dyn Iterator<Item = Result<LogEntry, ParseError>>>, ParseError> {
        let mut f = File::open(path).map_err(|e| {
            ParseError::IoError(format!(
                "Cannot open file at {} due to: {e}",
                path.display()
            ))
        })?;

        let meta = f.metadata().map_err(|e| {
            ParseError::IoError(format!(
                "Cannot traverse path: {} due to: {e}.",
                path.display()
            ))
        })?;

        let file_len = meta.len();
        let record_size: u64 = 384;

        if file_len % record_size != 0 {
        }

        let total_records = file_len / record_size;
        if total_records == 0 {
            return Ok(Box::new(std::iter::empty()));
        }

        let mut buf = vec![0u8; file_len as usize];
        f.seek(SeekFrom::Start(0))
            .map_err(|e| ParseError::IoError(format!("wtmp seek error: {e}")))?;
        f.read_exact(&mut buf)
            .map_err(|e| ParseError::IoError(format!("Cannot read wtmp file due to: {e}")))?;

        // Own the buffer and iterate chunk by chunk, yielding owned LogEntry
        let iter = (0..total_records).map(move |idx| {
            let start = (idx * record_size) as usize;
            let end = start + record_size as usize;
            let chunk = &buf[start..end];
            Ok(LogEntry::Wtmp(parse_record(chunk)))
        });

        Ok(Box::new(iter))
    }

    fn parser(
        &self,
        path: &Path,
        lines: Option<u64>,
        reverse: bool,
    ) -> Result<Vec<LogEntry>, ParseError> {
        // Preserve previous semantics: `lines` is last N, `reverse` flips.
        // Now implemented via `try_iter()` for consistency.
        if let Some(0) = lines {
            return Ok(Vec::new());
        }

        let all: Vec<LogEntry> = self
            .try_iter(path)?
            .collect::<Result<Vec<_>, _>>()?;

        let total = all.len() as u64;
        let lines_to_keep = match lines {
            Some(n) => (n.min(total)) as usize,
            None => all.len(),
        };

        if lines_to_keep == 0 {
            return Ok(Vec::new());
        }

        let start = all.len().saturating_sub(lines_to_keep);
        let mut entries: Vec<LogEntry> = all.into_iter().skip(start).collect();

        if reverse {
            entries.reverse();
        }

        Ok(entries)
    }
}

fn parse_record(buffer: &[u8]) -> WtmpRecord {
    WtmpRecord {
        ut_type: i16::from_ne_bytes(buffer[0..2].try_into().unwrap_or_default()),
        ut_pid: i32::from_ne_bytes(buffer[4..8].try_into().unwrap_or_default()),
        ut_dname: bytes_to_string(&buffer[8..40]),
        ut_id: bytes_to_string(&buffer[40..44]),
        ut_user: bytes_to_string(&buffer[44..76]),
        ut_host: bytes_to_string(&buffer[76..332]),
        e_termination: i16::from_ne_bytes(buffer[332..334].try_into().unwrap_or_default()),
        e_exit: i16::from_ne_bytes(buffer[334..336].try_into().unwrap_or_default()),
    }
}

fn bytes_to_string(raw: &[u8]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static CTR: AtomicU64 = AtomicU64::new(0);

    fn write_tmp_bytes(name: &str, contents: &[u8]) -> std::path::PathBuf {
        let id = CTR.fetch_add(1, Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!(
            "cassandra-wtmp-{}-{id}-{name}",
            std::process::id()
        ));
        std::fs::write(&p, contents).expect("write tmp fixture");
        p
    }

    #[allow(clippy::too_many_arguments)]
    fn make_record(
        ut_type: i16,
        ut_pid: i32,
        dname: &str,
        id: &str,
        user: &str,
        host: &str,
        term: i16,
        exit: i16,
    ) -> [u8; 384] {
        let mut buf = [0u8; 384];
        buf[0..2].copy_from_slice(&ut_type.to_ne_bytes());
        buf[4..8].copy_from_slice(&ut_pid.to_ne_bytes());
        copy_str(&mut buf[8..40], dname);
        copy_str(&mut buf[40..44], id);
        copy_str(&mut buf[44..76], user);
        copy_str(&mut buf[76..332], host);
        buf[332..334].copy_from_slice(&term.to_ne_bytes());
        buf[334..336].copy_from_slice(&exit.to_ne_bytes());
        buf
    }

    fn copy_str(dst: &mut [u8], s: &str) {
        let bytes = s.as_bytes();
        let n = bytes.len().min(dst.len());
        dst[..n].copy_from_slice(&bytes[..n]);
    }

    fn concat(records: &[[u8; 384]]) -> Vec<u8> {
        records.concat()
    }

    fn users(entries: &[LogEntry]) -> Vec<String> {
        entries
            .iter()
            .map(|e| match e {
                LogEntry::Wtmp(r) => r.ut_user.clone(),
                other => panic!("expected wtmp, got {other:?}"),
            })
            .collect()
    }

    #[test]
    fn parse_record_decodes_fields() {
        let raw = make_record(7, 1234, "pts/0", "01", "alice", "myhost", 0, 1);
        let r = parse_record(&raw);
        assert_eq!(r.ut_type, 7);
        assert_eq!(r.ut_pid, 1234);
        assert_eq!(r.ut_dname, "pts/0");
        assert_eq!(r.ut_id, "01");
        assert_eq!(r.ut_user, "alice");
        assert_eq!(r.ut_host, "myhost");
        assert_eq!(r.e_termination, 0);
        assert_eq!(r.e_exit, 1);
    }

    #[test]
    fn bytes_to_string_stops_at_nul() {
        assert_eq!(bytes_to_string(b"abc\0def"), "abc");
        assert_eq!(bytes_to_string(b"abc"), "abc");
    }

    #[test]
    fn parser_empty_file_returns_empty() {
        let p = write_tmp_bytes("empty.wtmp", &[]);
        let out = WtmpLog.parser(&p, None, false).expect("parse ok");
        let _ = std::fs::remove_file(&p);
        assert!(out.is_empty());
    }

    #[test]
    fn parser_reads_all_records_in_order() {
        let bytes = concat(&[
            make_record(7, 1, "pts/0", "01", "alice", "h1", 0, 0),
            make_record(7, 2, "pts/1", "02", "bob", "h2", 0, 0),
            make_record(7, 3, "pts/2", "03", "carol", "h3", 0, 0),
        ]);
        let p = write_tmp_bytes("all.wtmp", &bytes);
        let out = WtmpLog.parser(&p, None, false).expect("parse ok");
        let _ = std::fs::remove_file(&p);
        assert_eq!(users(&out), vec!["alice", "bob", "carol"]);
    }

    #[test]
    fn parser_lines_keeps_last_n() {
        let bytes = concat(&[
            make_record(7, 1, "d", "1", "alice", "h", 0, 0),
            make_record(7, 2, "d", "2", "bob", "h", 0, 0),
            make_record(7, 3, "d", "3", "carol", "h", 0, 0),
        ]);
        let p = write_tmp_bytes("last.wtmp", &bytes);
        let out = WtmpLog.parser(&p, Some(2), false).expect("parse ok");
        let _ = std::fs::remove_file(&p);
        assert_eq!(users(&out), vec!["bob", "carol"]);
    }

    #[test]
    fn parser_lines_zero_returns_empty() {
        let bytes = concat(&[make_record(7, 1, "d", "1", "alice", "h", 0, 0)]);
        let p = write_tmp_bytes("zero.wtmp", &bytes);
        let out = WtmpLog.parser(&p, Some(0), false).expect("parse ok");
        let _ = std::fs::remove_file(&p);
        assert!(out.is_empty());
    }

    #[test]
    fn parser_lines_beyond_total_returns_all() {
        let bytes = concat(&[make_record(7, 1, "d", "1", "alice", "h", 0, 0)]);
        let p = write_tmp_bytes("beyond.wtmp", &bytes);
        let out = WtmpLog.parser(&p, Some(99), false).expect("parse ok");
        let _ = std::fs::remove_file(&p);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn parser_reverse_flips_order() {
        let bytes = concat(&[
            make_record(7, 1, "d", "1", "alice", "h", 0, 0),
            make_record(7, 2, "d", "2", "bob", "h", 0, 0),
        ]);
        let p = write_tmp_bytes("rev.wtmp", &bytes);
        let out = WtmpLog.parser(&p, None, true).expect("parse ok");
        let _ = std::fs::remove_file(&p);
        assert_eq!(users(&out), vec!["bob", "alice"]);
    }

    #[test]
    fn parser_missing_file_is_io_error() {
        let missing = std::env::temp_dir().join("cassandra-wtmp-definitely-missing");
        let _ = std::fs::remove_file(&missing);
        match WtmpLog.parser(&missing, None, false) {
            Err(ParseError::IoError(_)) => {}
            other => panic!("expected IoError, got {other:?}"),
        }
    }

    #[test]
    fn try_iter_streams_all() {
        let bytes = concat(&[
            make_record(7, 1, "d", "1", "alice", "h", 0, 0),
            make_record(7, 2, "d", "2", "bob", "h", 0, 0),
        ]);
        let p = write_tmp_bytes("iter.wtmp", &bytes);
        let iter = WtmpLog.try_iter(&p).expect("iter ok");
        let out: Vec<_> = iter.map(|r| r.expect("ok")).collect();
        let _ = std::fs::remove_file(&p);
        assert_eq!(users(&out), vec!["alice", "bob"]);
    }
}
