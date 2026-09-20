use std::fs::{File, Metadata, metadata};
use std::io::ErrorKind;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

use crate::models::{ContentFormat, FiData, Finfo, FsKind, LogSource, PathStatus, SourceCandidate};

impl Finfo {
    /// # Errors
    /// The function will return Err in problems with traversing and unavailable paths
    #[must_use = "This function returns all usefull information about the file and its path; use let f = ..."]
    pub fn gather_info(sc: &SourceCandidate) -> Result<Finfo, PathStatus> {
        let path = Path::new(sc.path);

        match File::open(path) {
            Ok(file) => {
                let meta = file.metadata().map_err(PathStatus::Indeterminate)?;
                Ok(Finfo {
                    path: path.to_path_buf(),
                    source: sc.source,
                    pstatus: PathStatus::Found,
                    data: Some(FiData {
                        readable: true,
                        kind: Self::get_type(&meta),
                        mode: meta.mode(),
                        format: Self::content_format(sc.source),
                    }),
                })
            }
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Finfo {
                path: path.to_path_buf(),
                source: sc.source,
                pstatus: PathStatus::NotFound,
                data: None,
            }),
            Err(_) => match metadata(path) {
                Ok(meta) => Ok(Finfo {
                    path: path.to_path_buf(),
                    source: sc.source,
                    pstatus: PathStatus::Found,
                    data: Some(FiData {
                        readable: false,
                        kind: Self::get_type(&meta),
                        mode: meta.mode(),
                        format: Self::content_format(sc.source),
                    }),
                }),
                Err(_) => Err(PathStatus::NotFound),
            },
        }
    }
    fn content_format(lsc: LogSource) -> ContentFormat {
        match lsc {
            LogSource::Wtmp => ContentFormat::Binary,
            LogSource::Auth | LogSource::Sys => ContentFormat::PlainText,
        }
    }

    fn get_type(meta: &Metadata) -> FsKind {
        let ft = meta.file_type();
        if ft.is_file() {
            FsKind::Regular
        } else if ft.is_dir() {
            FsKind::Dir
        } else if ft.is_symlink() {
            FsKind::Symlink
        } else if ft.is_fifo() {
            FsKind::Fifo
        } else if ft.is_socket() {
            FsKind::Socket
        } else if ft.is_char_device() {
            FsKind::CharDevice
        } else if ft.is_block_device() {
            FsKind::BlockDevice
        } else {
            FsKind::Unknown
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static CTR: AtomicU64 = AtomicU64::new(0);

    fn leaked_path(contents: &str) -> &'static str {
        let id = CTR.fetch_add(1, Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!(
            "cassandra-finfo-{}-{id}.log",
            std::process::id()
        ));
        std::fs::write(&p, contents).expect("write tmp fixture");
        Box::leak(p.to_string_lossy().into_owned().into_boxed_str())
    }

    #[test]
    fn gather_info_missing_path_reports_not_found() {
        let sc = SourceCandidate {
            source: LogSource::Sys,
            path: "/definitely/missing/cassandra-test-path.log",
        };
        let info = Finfo::gather_info(&sc).expect("should return Ok");
        assert!(matches!(info.pstatus, PathStatus::NotFound));
        assert!(info.data.is_none());
    }

    #[test]
    fn gather_info_existing_file_is_readable_regular_plaintext() {
        let path = leaked_path("hello\n");
        let sc = SourceCandidate {
            source: LogSource::Sys,
            path,
        };
        let info = Finfo::gather_info(&sc).expect("should return Ok");
        assert!(matches!(info.pstatus, PathStatus::Found));
        let data = info.data.expect("should have data");
        assert!(data.readable);
        assert!(matches!(data.kind, FsKind::Regular));
        assert!(matches!(data.format, ContentFormat::PlainText));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn gather_info_wtmp_source_reports_binary() {
        let path = leaked_path("x\n");
        let sc = SourceCandidate {
            source: LogSource::Wtmp,
            path,
        };
        let info = Finfo::gather_info(&sc).expect("should return Ok");
        let data = info.data.expect("should have data");
        assert!(matches!(data.format, ContentFormat::Binary));
        let _ = std::fs::remove_file(path);
    }
}
