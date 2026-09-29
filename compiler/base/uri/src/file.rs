//! File URIs (RFC 8089): the URI of a path of this system, and the path a file URI names.
//!
//! A path is written after an empty authority, `file:///home/a%20b`, each byte its path does not
//! allow escaped. On Windows a drive is the first segment, `file:///C:/dir`, and a UNC path's
//! server the host, `file://server/share/dir`. Reading, the host may be empty or `localhost`; on
//! Windows another host names a UNC path.

use std::path::{Path, PathBuf};

use crate::chars::Set;
use crate::compose::{Composer, Pieces};
use crate::error::NotFile;
use crate::parts::HostKind;
use crate::scheme::Known;
use crate::uri::Uri;

/// A path of this system as a file URI writes it: a host, empty for this machine, and the path.
struct Written {
    host: String,
    path: String,
}

impl Uri {
    /// The file URI of `path`, which must be absolute.
    ///
    /// # Errors
    /// That the path is relative, or not written in this system's encoding.
    pub fn from_file_path(path: &Path) -> Result<Self, NotFile> {
        if !path.is_absolute() {
            return Err(NotFile::Relative);
        }
        let written = Written::of(path)?;
        let kind = HostKind::named(&written.host).map_err(|_| NotFile::Encoding)?;
        let mut out = Composer::default();
        out.scheme(Known::FILE.name);
        out.authority(Pieces { userinfo: None, host: &written.host, kind, port: None });
        out.path(&written.path);
        Ok(out.finish())
    }

    /// The path of this system this file URI names.
    ///
    /// # Errors
    /// That the URI is not a file URI, names another machine, or a path this system cannot hold.
    pub fn to_file_path(&self) -> Result<PathBuf, NotFile> {
        if !self.scheme().is(Known::FILE.name) {
            return Err(NotFile::Scheme);
        }
        let host = self.host().filter(|h| !h.is_local()).map(|h| h.as_str());
        if !self.path().is_absolute() {
            return Err(NotFile::Relative);
        }
        Written::read(host, &self.path().encoded().decode())
    }
}

#[cfg(not(windows))]
impl Written {
    #[expect(clippy::unnecessary_wraps, reason = "every path has a file URI here, not on Windows")]
    fn of(path: &Path) -> Result<Self, NotFile> {
        let path = Set::Path.encode(path.as_os_str().as_encoded_bytes()).into_owned();
        Ok(Self { host: String::new(), path })
    }

    fn read(host: Option<&str>, bytes: &[u8]) -> Result<PathBuf, NotFile> {
        if host.is_some() {
            return Err(NotFile::Host);
        }
        Self::os(bytes.to_vec())
    }

    #[cfg(unix)]
    #[expect(clippy::unnecessary_wraps, reason = "every byte string is a path on unix, not elsewhere")]
    fn os(bytes: Vec<u8>) -> Result<PathBuf, NotFile> {
        use std::os::unix::ffi::OsStringExt;
        Ok(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
    }

    #[cfg(not(unix))]
    fn os(bytes: Vec<u8>) -> Result<PathBuf, NotFile> {
        String::from_utf8(bytes).map(PathBuf::from).map_err(|_| NotFile::Encoding)
    }
}

#[cfg(windows)]
impl Written {
    const SEPARATOR: char = '\\';
    const UNC: &str = r"\\";

    fn of(path: &Path) -> Result<Self, NotFile> {
        use std::path::{Component, Prefix};
        let mut host = String::new();
        let mut out = String::new();
        for component in path.components() {
            match component {
                Component::Prefix(prefix) => match prefix.kind() {
                    Prefix::Disk(d) | Prefix::VerbatimDisk(d) => {
                        out.push('/');
                        out.push(char::from(d));
                        out.push(':');
                    }
                    Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
                        host = server.to_str().ok_or(NotFile::Encoding)?.to_ascii_lowercase();
                        out.push('/');
                        Set::Segment
                            .encode_into(share.to_str().ok_or(NotFile::Encoding)?.as_bytes(), &mut out);
                    }
                    Prefix::Verbatim(_) | Prefix::DeviceNS(_) => return Err(NotFile::Encoding),
                },
                Component::RootDir => {}
                Component::CurDir | Component::ParentDir | Component::Normal(_) => {
                    let segment = component.as_os_str().to_str().ok_or(NotFile::Encoding)?;
                    out.push('/');
                    Set::Segment.encode_into(segment.as_bytes(), &mut out);
                }
            }
        }
        Ok(Self { host, path: out })
    }

    fn read(host: Option<&str>, bytes: &[u8]) -> Result<PathBuf, NotFile> {
        let path = str::from_utf8(bytes).map_err(|_| NotFile::Encoding)?;
        let path = path.replace('/', &Self::SEPARATOR.to_string());
        Ok(PathBuf::from(match host {
            Some(host) => format!("{}{host}{path}", Self::UNC),
            // `\C:\dir` or `\C|\dir`: the drive, without the root before it.
            None => match path.as_bytes() {
                [_, d, b':' | b'|', ..] if d.is_ascii_alphabetic() => {
                    format!("{}:{}", char::from(*d), &path[3..])
                }
                _ => return Err(NotFile::Relative),
            },
        }))
    }
}
