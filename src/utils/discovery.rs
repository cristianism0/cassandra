use std::fs::*;
use std::io::ErrorKind;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

use crate::models::{SourceCandidate, Finfo, PathStatus, FiData, LogSource, FsKind, ContentFormat};

impl Finfo {
    pub fn gather_info(sc: &SourceCandidate) -> Finfo {
        let path = Path::new(sc.path);

        match File::open(path) {
            Ok(file) => {
                let meta = file
                    .metadata()
                    .expect("Unexpected error ocurred during the metadata collect on path: {path:#?}");
                Finfo {
                    path: path.to_path_buf(),
                    source: sc.source,
                    pstatus: PathStatus::Found,
                    data: Some(FiData {
                        readable: true,
                        kind: Self::get_type(&meta),
                        mode: meta.mode(),
                        format: Self::content_format(sc.source),
                    }),
                }
            }
            Err(e) if e.kind() == ErrorKind::NotFound => Finfo {
                path: path.to_path_buf(),
                source: sc.source,
                pstatus: PathStatus::NotFound,
                data: None,
            },
            Err(e) => match metadata(path) {
                Ok(meta) => Finfo {
                    path: path.to_path_buf(),
                    source: sc.source,
                    pstatus: PathStatus::Found,
                    data: Some(FiData {
                        readable: false,
                        kind: Self::get_type(&meta),
                        mode: meta.mode(),
                        format: Self::content_format(sc.source),
                    }),
                },
                Err(_) => Finfo {
                    path: path.to_path_buf(),
                    source: sc.source,
                    pstatus: PathStatus::Indeterminate(e),
                    data: None,
                },
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
