use crate::parsers::selector::LogParser;
use std::io::{Read, Seek, SeekFrom};
use std::{fs::File, path::Path};

use crate::models::{LogEntry, ParseError, WtmpRecord};
pub struct WtmpLog;

impl LogParser for WtmpLog {
    fn parser(&self, path: &Path) -> Result<Vec<LogEntry>, ParseError> {
        let mut cursor: u64 = 0;
        let mut f = File::open(path).map_err(|e| {
            ParseError::IoError(format!("Cannot open file at {:#?} due to: {e}", path))
        })?;
        let meta = f.metadata().map_err(|e| {
            ParseError::IoError(format!(
                "Cannot traverse path: {path:#?}. Due to error {e}."))
	})?;
        let file_len = meta.len();

        if file_len < cursor {
            cursor = 0;
        }

        let mut entries = Vec::new();

        if file_len > cursor {
            f.seek(SeekFrom::Start(cursor))
                .map_err(|e| ParseError::IoError(format!("Journal seek error: {e}")))?;
            let mut buf = vec![0u8; (file_len - cursor) as usize];
            f.read_exact(&mut buf).map_err(|e| {
                ParseError::MalformedLine(format!("Cannot read journald line due to error: {e}"))
            })?;

            for r in buf.chunks_exact(384) {
                entries.push(LogEntry::Wtmp(parse_record(r).expect("Failed to collect the Wtmp line info.")));
            }
        }

        Ok(entries)
    }
}
fn parse_record(buffer: &[u8]) -> Option<WtmpRecord> {
    Some(WtmpRecord {
        ut_type: i16::from_ne_bytes(buffer[0..2].try_into().unwrap_or_default()),
        ut_pid: i32::from_ne_bytes(buffer[4..8].try_into().unwrap_or_default()),
        ut_dname: bytes_to_string(&buffer[8..40]),
        ut_id: bytes_to_string(&buffer[40..44]),
        ut_user: bytes_to_string(&buffer[44..76]),
        ut_host: bytes_to_string(&buffer[76..332]),
        e_termination: i16::from_ne_bytes(buffer[332..334].try_into().unwrap_or_default()),
        e_exit: i16::from_ne_bytes(buffer[334..336].try_into().unwrap_or_default()),
    })
}

fn bytes_to_string(raw: &[u8]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).into_owned()
}
