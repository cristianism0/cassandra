use crate::parsers::selector::LogParser;
use std::io::{Read, Seek, SeekFrom};
use std::{fs::File, path::Path};

use crate::models::{LogEntry, ParseError, WtmpRecord};
pub struct WtmpLog;

impl LogParser for WtmpLog {
    fn parser(&self, path: &Path, lines: Option<u64>, reverse: bool) -> Result<Vec<LogEntry>, ParseError> {
	let mut f = File::open(path).map_err(|e| {
	    ParseError::IoError(format!("Cannot open file at {path:#?} due to: {e}"))
	})?;

	let meta = f.metadata().map_err(|e| {
	    ParseError::IoError(format!("Cannot traverse path: {path:#?}. Due to error {e}."))
	})?;

	let file_len = meta.len();
	//wtmp has 384 bytes per line
	let record_size = 384;

	let total_lines = file_len / record_size;

	let lines_to_read = match lines {
	    Some(n) => n.min(total_lines),
	    None => total_lines,
	};

	if lines_to_read == 0 {
	    return Ok(Vec::new());
	}

	let bytes_to_read = lines_to_read * record_size;
	let start_offset = file_len - bytes_to_read;

	f.seek(SeekFrom::Start(start_offset))
	    .map_err(|e| ParseError::IoError(format!("Journal seek error: {e}")))?;

	let mut buf = vec![0u8; bytes_to_read as usize];
	f.read_exact(&mut buf).map_err(|e| {
	    ParseError::IoError(format!("Cannot read journald line due to: {e}"))
	})?;

	let mut entries = Vec::with_capacity(lines_to_read as usize);

	if reverse {
	    for r in buf.chunks_exact(record_size as usize).rev() {
		entries.push(LogEntry::Wtmp(
		    parse_record(r).expect("Failed to collect the Wtmp line info.")
		));
	    }
	} else {
	    for r in buf.chunks_exact(record_size as usize) {
		entries.push(LogEntry::Wtmp(
		    parse_record(r).expect("Failed to collect the Wtmp line info.")
		));
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
