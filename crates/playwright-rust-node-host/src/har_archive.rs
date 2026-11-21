//! Capability-owning HAR filesystem and ZIP adapter.
//!
//! The replay and recording policy crates remain backend-neutral. This module
//! owns file acquisition, path containment, archive decoding and final
//! persistence in the official Playwright layouts. Replay limits are opt-in;
//! the Playwright-compatible default does not impose private size quotas.

use std::{
    fs::{File, OpenOptions},
    io::{Read, Write as _},
    path::{Component, Path, PathBuf},
};

use playwright_rust_har_replay::{ReplayArchive, ReplayContentSource, ReplayLimits};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

#[cfg(feature = "har-recording")]
use playwright_rust_har_recording::HarExport;

use crate::HostError;

pub(crate) async fn load(path: &str) -> Result<ReplayArchive, HostError> {
    validate_path(path)?;
    let path = PathBuf::from(path);
    let is_zip = path.to_string_lossy().ends_with(".zip");
    tokio::task::spawn_blocking(move || {
        if is_zip {
            load_zip(&path)
        } else {
            load_plain(&path)
        }
    })
    .await
    .map_err(|error| HostError::Backend(format!("HAR reader task failed: {error}")))?
}

#[cfg(feature = "har-recording")]
pub(crate) async fn persist(path: PathBuf, export: HarExport) -> Result<(), HostError> {
    let display = path.to_string_lossy();
    validate_path(&display)?;
    let is_zip = display.ends_with(".zip");
    drop(display);
    tokio::task::spawn_blocking(move || {
        if is_zip {
            persist_zip(&path, export)
        } else {
            persist_plain(&path, export)
        }
    })
    .await
    .map_err(|error| HostError::Backend(format!("HAR writer task failed: {error}")))?
}

fn validate_path(path: &str) -> Result<(), HostError> {
    if path.is_empty() {
        return Err(HostError::InvalidArgument(
            "HAR path must not be empty".to_owned(),
        ));
    }
    Ok(())
}

fn load_plain(path: &Path) -> Result<ReplayArchive, HostError> {
    let limits = ReplayLimits::default();
    let bytes = read_bounded(path, limits.archive_bytes)
        .map_err(|error| invalid_har(path, "cannot read HAR file", error))?;
    let base = parent_directory(path);
    let canonical_base = base
        .canonicalize()
        .map_err(|error| invalid_har(path, "cannot resolve HAR directory", error))?;
    ReplayArchive::from_json_with_content_source(
        &bytes,
        limits,
        PlainContentSource { canonical_base },
    )
    .map_err(HostError::backend)
}

fn load_zip(path: &Path) -> Result<ReplayArchive, HostError> {
    let file = File::open(path).map_err(|error| invalid_har(path, "cannot open HAR ZIP", error))?;
    let mut zip = ZipArchive::new(file).map_err(HostError::backend)?;
    let har_name = zip
        .file_names()
        .filter_map(Result::ok)
        .find(|name| name.ends_with(".har"))
        .map(|name| name.into_owned())
        .ok_or_else(|| {
            HostError::InvalidArgument("Specified archive does not have a .har file".into())
        })?;
    let limits = ReplayLimits::default();
    let bytes = read_zip_entry(&mut zip, &har_name, limits.archive_bytes)
        .map_err(HostError::InvalidArgument)?;
    ReplayArchive::from_json_with_content_source(
        &bytes,
        limits,
        ZipContentSource {
            path: path.to_owned(),
        },
    )
    .map_err(HostError::backend)
}

#[derive(Debug)]
struct PlainContentSource {
    canonical_base: PathBuf,
}

impl ReplayContentSource for PlainContentSource {
    fn read(&self, file: &str, limit: usize) -> Result<Vec<u8>, String> {
        let relative = safe_relative_path(file)?;
        let candidate = self.canonical_base.join(relative);
        let canonical = candidate
            .canonicalize()
            .map_err(|error| format!("cannot resolve attached content: {error}"))?;
        if !canonical.starts_with(&self.canonical_base) {
            return Err("attached content escapes the HAR directory".to_owned());
        }
        read_bounded(&canonical, limit)
            .map_err(|error| format!("cannot read attached content: {error}"))
    }
}

#[derive(Debug)]
struct ZipContentSource {
    path: PathBuf,
}

impl ReplayContentSource for ZipContentSource {
    fn read(&self, name: &str, limit: usize) -> Result<Vec<u8>, String> {
        let file =
            File::open(&self.path).map_err(|error| format!("cannot reopen HAR ZIP: {error}"))?;
        let mut zip =
            ZipArchive::new(file).map_err(|error| format!("cannot read HAR ZIP: {error}"))?;
        read_zip_entry(&mut zip, name, limit)
    }
}

fn read_zip_entry(zip: &mut ZipArchive<File>, name: &str, limit: usize) -> Result<Vec<u8>, String> {
    let mut entry = zip
        .by_name(name)
        .map_err(|error| format!("cannot open ZIP entry {name:?}: {error}"))?;
    let declared = usize::try_from(entry.size())
        .map_err(|_| format!("ZIP entry {name:?} size does not fit this platform"))?;
    if declared > limit {
        return Err(format!("ZIP entry {name:?} exceeds the {limit}-byte limit"));
    }
    read_limited(&mut entry, limit)
        .map_err(|error| format!("cannot read ZIP entry {name:?}: {error}"))
}

fn safe_relative_path(value: &str) -> Result<&Path, String> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err("attached content path is not a contained relative path".to_owned());
    }
    Ok(path)
}

fn parent_directory(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn read_bounded(path: &Path, limit: usize) -> std::io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    read_limited(&mut file, limit)
}

fn read_limited(reader: &mut impl Read, limit: usize) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(limit.min(64 * 1024))
        .map_err(|error| {
            std::io::Error::other(format!("failed to reserve HAR read buffer: {error}"))
        })?;
    reader
        .take(u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(std::io::Error::other(format!(
            "content exceeds the {limit}-byte limit"
        )));
    }
    Ok(bytes)
}

#[cfg(feature = "har-recording")]
fn persist_plain(path: &Path, export: HarExport) -> Result<(), HostError> {
    let parent = parent_directory(path);
    std::fs::create_dir_all(parent)
        .map_err(|error| persistence_error(path, "create HAR directory", error))?;
    for attachment in export.attachments {
        safe_relative_path(&attachment.name).map_err(HostError::InvalidArgument)?;
        let target = parent.join(&attachment.name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(mut file) => file
                .write_all(&attachment.bytes)
                .map_err(|error| persistence_error(&target, "write HAR attachment", error))?,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(persistence_error(&target, "create HAR attachment", error));
            }
        }
    }
    std::fs::write(path, export.json).map_err(|error| persistence_error(path, "write HAR", error))
}

#[cfg(feature = "har-recording")]
fn persist_zip(path: &Path, export: HarExport) -> Result<(), HostError> {
    let parent = parent_directory(path);
    std::fs::create_dir_all(parent)
        .map_err(|error| persistence_error(path, "create HAR ZIP directory", error))?;
    let file =
        File::create(path).map_err(|error| persistence_error(path, "create HAR ZIP", error))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file("har.har", options)
        .map_err(HostError::backend)?;
    zip.write_all(&export.json).map_err(HostError::backend)?;
    for attachment in export.attachments {
        safe_relative_path(&attachment.name).map_err(HostError::InvalidArgument)?;
        zip.start_file(&attachment.name, options)
            .map_err(HostError::backend)?;
        zip.write_all(&attachment.bytes)
            .map_err(HostError::backend)?;
    }
    zip.finish().map_err(HostError::backend)?;
    Ok(())
}

fn invalid_har(path: &Path, action: &str, error: impl std::fmt::Display) -> HostError {
    HostError::InvalidArgument(format!("{action} {}: {error}", path.display()))
}

#[cfg(feature = "har-recording")]
fn persistence_error(path: &Path, action: &str, error: impl std::fmt::Display) -> HostError {
    HostError::Backend(format!("failed to {action} {}: {error}", path.display()))
}

#[cfg(all(test, feature = "har-recording"))]
mod tests {
    use playwright_rust_har_recording::{HarAttachment, HarExport};
    use playwright_rust_har_replay::ReplayAction;

    use super::*;

    fn export() -> HarExport {
        HarExport {
            json: br#"{"log":{"entries":[{"request":{"method":"GET","url":"https://example.test/data","headers":[]},"response":{"status":200,"headers":[],"content":{"_file":"body.txt"}}}]}}"#.to_vec(),
            attachments: vec![HarAttachment {
                name: "body.txt".to_owned(),
                bytes: b"attached body".to_vec(),
            }],
        }
    }

    async fn assert_replays(path: &Path) {
        let archive = load(path.to_str().unwrap()).await.unwrap();
        let action = archive
            .lookup("https://example.test/data", "GET", &[], None, false)
            .unwrap();
        let ReplayAction::Fulfill(response) = action else {
            panic!("expected HAR fulfillment");
        };
        assert_eq!(response.body().as_ref(), b"attached body");
    }

    #[tokio::test]
    async fn plain_har_persists_official_sibling_attachment_and_replays() {
        // Arrange
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("capture.har");

        // Act
        persist(path.clone(), export()).await.unwrap();

        // Assert
        assert_eq!(
            std::fs::read(directory.path().join("body.txt")).unwrap(),
            b"attached body"
        );
        assert_replays(&path).await;
    }

    #[tokio::test]
    async fn zip_har_persists_official_root_layout_and_replays() {
        // Arrange
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("capture.zip");

        // Act
        persist(path.clone(), export()).await.unwrap();

        // Assert
        let file = File::open(&path).unwrap();
        let mut zip = ZipArchive::new(file).unwrap();
        assert!(zip.by_name("har.har").is_ok());
        assert!(zip.by_name("body.txt").is_ok());
        drop(zip);
        assert_replays(&path).await;
    }

    #[test]
    fn bare_har_path_uses_current_directory_as_its_resource_root() {
        // Arrange
        let path = Path::new("capture.har");

        // Act
        let parent = parent_directory(path);

        // Assert
        assert_eq!(parent, Path::new("."));
    }

    #[test]
    fn har_path_validation_has_no_private_length_cap() {
        // Arrange
        let path = "a".repeat(4 * 1024 + 1);

        // Act
        let result = validate_path(&path);

        // Assert
        assert!(result.is_ok());
    }
}
