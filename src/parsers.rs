pub mod auth;
pub mod journal;
pub mod selector;
pub mod sys;
pub mod wtmp;

use regex::Regex;
use std::sync::LazyLock;

pub static SYS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:<(?P<pri>\d+)>)?(?P<timestamp>(?P<month>[A-Za-z]{3})\s+(?P<day>\d{1,2})\s+(?P<time>\d{2}:\d{2}:\d{2}))\s+(?P<host>\S+)\s+(?P<process>[^\[:]+)(?:\[(?P<pid>\d+)\])?:\s*(?:(?P<caller>[^:]+):\s*)?(?P<msg>.*)$").unwrap()
});

pub static AUTH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:<(?P<pri>\d+)>)?(?P<timestamp>(?P<month>[A-Za-z]{3})\s+(?P<day>\d{1,2})\s+(?P<time>\d{2}:\d{2}:\d{2}))\s+(?P<host>\S+)\s+(?P<process>[^\[:]+)(?:\[(?P<pid>\d+)\])?:\s*(?:(?P<caller>[^:]+):\s*)?(?P<msg>.*)$").unwrap()
});

// Find byte offset of the start of the last `n` lines.
// If file has <= n lines, returns 0. Caller must discard first partial line if offset != 0.
pub(crate) fn find_tail_offset(file: &mut std::fs::File, n: usize) -> std::io::Result<u64> {
    use std::io::{Read, Seek, SeekFrom};
    let len = file.metadata()?.len();
    if len == 0 || n == 0 {
        return Ok(0);
    }
    // If file ends with '\n', that trailing newline does not start a new line — skip it.
    let mut pos = len;
    if len > 0 {
        file.seek(SeekFrom::End(-1))?;
        let mut last = [0u8; 1];
        file.read_exact(&mut last)?;
        if last[0] == b'\n' && n > 0 {
            pos -= 1;
        }
    }
    let mut to_find = n;
    let mut buf = [0u8; 8192];
    while pos > 0 {
        let read_size = std::cmp::min(pos, buf.len() as u64) as usize;
        pos -= read_size as u64;
        file.seek(SeekFrom::Start(pos))?;
        file.read_exact(&mut buf[..read_size])?;
        for i in (0..read_size).rev() {
            if buf[i] == b'\n' {
                to_find -= 1;
                if to_find == 0 {
                    return Ok(pos + i as u64 + 1);
                }
            }
        }
        if pos == 0 {
            break;
        }
    }
    Ok(0)
}
