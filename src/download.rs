use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

use crate::progress;

const RETRY_DELAYS: [Duration; 2] = [Duration::from_millis(500), Duration::from_secs(1)];

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct DownloadRecord {
    pub name: String,
    pub source_url: String,
    pub resolved_url: String,
    pub bytes: u64,
    pub sha256: String,
}

pub fn to_file(
    client: &Client,
    name: &str,
    url: &str,
    destination: &Path,
) -> Result<DownloadRecord> {
    to_file_inner(client, name, url, destination, true)
}

pub fn to_file_quiet(
    client: &Client,
    name: &str,
    url: &str,
    destination: &Path,
) -> Result<DownloadRecord> {
    to_file_inner(client, name, url, destination, false)
}

fn to_file_inner(
    client: &Client,
    name: &str,
    url: &str,
    destination: &Path,
    show_progress: bool,
) -> Result<DownloadRecord> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    retry(url, &RETRY_DELAYS, || {
        download_once(client, name, url, destination, show_progress)
    })
}

fn download_once(
    client: &Client,
    name: &str,
    url: &str,
    destination: &Path,
    show_progress: bool,
) -> Result<DownloadRecord> {
    let mut response = client
        .get(url)
        .send()
        .with_context(|| format!("failed to download {url}"))?
        .error_for_status()
        .with_context(|| format!("download returned an error for {url}"))?;
    let mut resolved_url = response.url().clone();
    resolved_url.set_query(None);
    resolved_url.set_fragment(None);
    let resolved_url = resolved_url.to_string();
    let parent = destination
        .parent()
        .context("download destination has no parent directory")?;
    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temporary file in {}", parent.display()))?;
    let progress_bar = show_progress.then(|| progress::bytes(name, response.content_length()));
    let result = copy_and_hash(&mut response, &mut temporary, progress_bar.as_ref());
    if let Some(bar) = progress_bar {
        bar.finish_and_clear();
    }
    let (bytes, sha256) = result
        .with_context(|| format!("failed to save download from {url}"))?;
    if bytes == 0 {
        bail!("download from {url} was empty");
    }
    temporary.flush().context("failed to flush downloaded file")?;
    temporary
        .persist(destination)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to install {}", destination.display()))?;

    Ok(DownloadRecord {
        name: name.to_owned(),
        source_url: url.to_owned(),
        resolved_url,
        bytes,
        sha256,
    })
}

fn copy_and_hash(
    mut reader: impl Read,
    mut writer: impl Write,
    progress_bar: Option<&indicatif::ProgressBar>,
) -> Result<(u64, String)> {
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        writer.write_all(&buffer[..count])?;
        hasher.update(&buffer[..count]);
        bytes += count as u64;
        if let Some(bar) = progress_bar {
            bar.inc(count as u64);
        }
    }
    Ok((bytes, format!("{:x}", hasher.finalize())))
}

fn retry<T>(
    description: &str,
    delays: &[Duration],
    mut operation: impl FnMut() -> Result<T>,
) -> Result<T> {
    let mut last_error = None;
    for attempt in 0..=delays.len() {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) => {
                last_error = Some(error);
                if let Some(delay) = delays.get(attempt) {
                    eprintln!(
                        "Download attempt {} failed for {description}; retrying...",
                        attempt + 1
                    );
                    thread::sleep(*delay);
                }
            }
        }
    }
    Err(last_error.expect("retry always performs at least one attempt"))
        .with_context(|| format!("download failed after {} attempts", delays.len() + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn hashes_while_copying() {
        let mut output = Vec::new();
        let (bytes, digest) =
            copy_and_hash(Cursor::new(b"abc"), &mut output, None).unwrap();

        assert_eq!(bytes, 3);
        assert_eq!(output, b"abc");
        assert_eq!(
            digest,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn retries_until_operation_succeeds() {
        let mut attempts = 0;
        let result = retry("test", &[Duration::ZERO, Duration::ZERO], || {
            attempts += 1;
            if attempts < 3 {
                bail!("temporary failure");
            }
            Ok(42)
        })
        .unwrap();

        assert_eq!(result, 42);
        assert_eq!(attempts, 3);
    }
}
