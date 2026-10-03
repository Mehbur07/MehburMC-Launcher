//! Resilient parallel downloader.
//!
//! - bounded concurrency (`buffer_unordered`)
//! - resume via `<file>.part` + `Range` header
//! - exponential backoff with jitter on transient failures
//! - size + checksum verification, then atomic rename into place
//! - free-space preflight, cancellation at any point

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::{StreamExt, TryStreamExt, stream};
use reqwest::StatusCode;
use reqwest::header::RANGE;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

use super::{Http, network};
use crate::error::{CoreError, Result};
use crate::events::Progress;
use crate::hash::Checksum;

#[derive(Debug, Clone)]
pub struct DownloadItem {
    pub url: String,
    pub dest: PathBuf,
    pub checksum: Option<Checksum>,
    pub size: Option<u64>,
}

/// How thoroughly already-present files are checked before being reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verify {
    /// Existence + size (fast; used on every launch).
    #[default]
    Quick,
    /// Existence + size + checksum ("repair").
    Full,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DownloadStats {
    pub downloaded: usize,
    pub reused: usize,
}

/// Extra headroom kept free on the target disk.
const DISK_RESERVE: u64 = 64 * 1024 * 1024;

pub struct Downloader {
    http: Http,
    concurrency: usize,
    max_attempts: u32,
    base_backoff: Duration,
}

impl Downloader {
    pub fn new(http: Http, concurrency: usize) -> Self {
        Self {
            http,
            concurrency: concurrency.max(1),
            max_attempts: 5,
            base_backoff: Duration::from_millis(500),
        }
    }

    #[cfg(test)]
    pub fn with_backoff(mut self, base: Duration, attempts: u32) -> Self {
        self.base_backoff = base;
        self.max_attempts = attempts;
        self
    }

    pub async fn run(
        &self,
        items: Vec<DownloadItem>,
        verify: Verify,
        progress: &Progress,
        cancel: &CancellationToken,
    ) -> Result<DownloadStats> {
        // Quick pre-pass: what is missing, and does it fit on disk?
        let missing_bytes: u64 = items
            .iter()
            .filter(|i| !quick_valid(i))
            .filter_map(|i| i.size)
            .sum();
        let total_bytes: u64 = items.iter().filter_map(|i| i.size).sum();
        progress.set_totals(items.len() as u64, total_bytes);
        if let Some(first) = items.first() {
            check_free_space(&first.dest, missing_bytes)?;
        }

        let stats = std::sync::Mutex::new(DownloadStats::default());
        stream::iter(items.into_iter().map(Ok::<_, CoreError>))
            .try_for_each_concurrent(self.concurrency, |item| {
                let stats = &stats;
                async move {
                    if cancel.is_cancelled() {
                        return Err(CoreError::Cancelled);
                    }
                    if is_valid(&item, verify).await {
                        progress.add_bytes(item.size.unwrap_or(0));
                        stats.lock().expect("stats").reused += 1;
                    } else {
                        self.fetch(&item, progress, cancel).await?;
                        stats.lock().expect("stats").downloaded += 1;
                    }
                    progress.item_done();
                    Ok(())
                }
            })
            .await?;
        progress.finish();
        Ok(stats.into_inner().expect("stats"))
    }

    async fn fetch(
        &self,
        item: &DownloadItem,
        progress: &Progress,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            let result = tokio::select! {
                _ = cancel.cancelled() => return Err(CoreError::Cancelled),
                r = self.fetch_once(item, progress) => r,
            };
            match result {
                Ok(()) => return Ok(()),
                Err(e) if e.is_transient() && attempt < self.max_attempts => {
                    let delay = backoff(self.base_backoff, attempt);
                    tracing::warn!(url = %item.url, attempt, ?delay, error = %e.detail(), "download failed, retrying");
                    tokio::select! {
                        _ = cancel.cancelled() => return Err(CoreError::Cancelled),
                        _ = tokio::time::sleep(delay) => {}
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn fetch_once(&self, item: &DownloadItem, progress: &Progress) -> Result<()> {
        let url = self.http.allowlist().check(&item.url)?;
        let part = part_path(&item.dest);
        if let Some(parent) = part.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| CoreError::io(parent, e))?;
        }

        let mut existing = tokio::fs::metadata(&part)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        if let Some(size) = item.size {
            if existing == size {
                // A previous run finished the transfer but not the rename.
                return finalize(item, &part).await;
            }
            if existing > size {
                let _ = tokio::fs::remove_file(&part).await;
                existing = 0;
            }
        }

        let mut req = self.http.client().get(url);
        if existing > 0 {
            req = req.header(RANGE, format!("bytes={existing}-"));
        }
        let resp = req.send().await.map_err(|e| network(&item.url, e))?;
        let status = resp.status();

        let append = match status {
            StatusCode::PARTIAL_CONTENT if existing > 0 => true,
            s if s.is_success() => false,
            StatusCode::RANGE_NOT_SATISFIABLE => {
                // Stale partial file: drop it and let the retry start fresh.
                let _ = tokio::fs::remove_file(&part).await;
                return Err(CoreError::io(
                    &part,
                    std::io::Error::new(std::io::ErrorKind::Interrupted, "range not satisfiable"),
                ));
            }
            s => {
                return Err(CoreError::HttpStatus {
                    url: item.url.clone(),
                    status: s.as_u16(),
                });
            }
        };
        if append {
            // Count the bytes we already had towards progress.
            progress.add_bytes(existing);
        }

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(&part)
            .await
            .map_err(|e| CoreError::io(&part, e))?;

        let mut written: u64 = if append { existing } else { 0 };
        let mut body = resp.bytes_stream();
        let result: Result<()> = async {
            while let Some(chunk) = body.next().await {
                let chunk = chunk.map_err(|e| network(&item.url, e))?;
                file.write_all(&chunk)
                    .await
                    .map_err(|e| CoreError::io(&part, e))?;
                written += chunk.len() as u64;
                progress.add_bytes(chunk.len() as u64);
            }
            file.flush().await.map_err(|e| CoreError::io(&part, e))?;
            Ok(())
        }
        .await;
        drop(file);
        if let Err(e) = result {
            // Keep the .part file so the retry can resume; undo progress.
            progress.sub_bytes(written);
            return Err(e);
        }

        finalize(item, &part).await.inspect_err(|_| {
            progress.sub_bytes(written);
        })
    }
}

/// Verifies the finished `.part` file and atomically moves it into place.
async fn finalize(item: &DownloadItem, part: &Path) -> Result<()> {
    if let Some(size) = item.size {
        let actual = tokio::fs::metadata(part)
            .await
            .map_err(|e| CoreError::io(part, e))?
            .len();
        if actual != size {
            let _ = tokio::fs::remove_file(part).await;
            return Err(CoreError::HashMismatch {
                path: item.dest.clone(),
                expected: format!("{size} bytes"),
                actual: format!("{actual} bytes"),
            });
        }
    }
    if let Some(sum) = item.checksum.clone() {
        let p = part.to_owned();
        let res = tokio::task::spawn_blocking(move || sum.verify(&p))
            .await
            .expect("hash task panicked");
        if let Err(e) = res {
            let _ = tokio::fs::remove_file(part).await;
            return Err(match e {
                CoreError::HashMismatch {
                    expected, actual, ..
                } => CoreError::HashMismatch {
                    path: item.dest.clone(),
                    expected,
                    actual,
                },
                other => other,
            });
        }
    }
    tokio::fs::rename(part, &item.dest)
        .await
        .map_err(|e| CoreError::io(&item.dest, e))
}

fn quick_valid(item: &DownloadItem) -> bool {
    match std::fs::metadata(&item.dest) {
        Ok(m) => m.is_file() && item.size.is_none_or(|s| s == m.len()),
        Err(_) => false,
    }
}

async fn is_valid(item: &DownloadItem, verify: Verify) -> bool {
    if !quick_valid(item) {
        return false;
    }
    match (verify, item.checksum.clone()) {
        (Verify::Full, Some(sum)) => {
            let p = item.dest.clone();
            tokio::task::spawn_blocking(move || sum.verify(&p).is_ok())
                .await
                .unwrap_or(false)
        }
        _ => true,
    }
}

pub fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

/// `base * 2^(attempt-1)`, capped at 30 s, with "equal jitter" (50–100 %).
fn backoff(base: Duration, attempt: u32) -> Duration {
    let exp = base.saturating_mul(1u32 << (attempt - 1).min(16));
    let capped = exp.min(Duration::from_secs(30));
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let jitter = 0.5 + f64::from(nanos % 1000) / 2000.0;
    capped.mul_f64(jitter)
}

fn check_free_space(dest: &Path, needed: u64) -> Result<()> {
    if needed == 0 {
        return Ok(());
    }
    // Walk up to the first existing ancestor (the target may not exist yet).
    let mut dir = dest.parent();
    while let Some(d) = dir {
        if d.exists() {
            break;
        }
        dir = d.parent();
    }
    let Some(dir) = dir else { return Ok(()) };
    match fs4::available_space(dir) {
        Ok(available) if available < needed + DISK_RESERVE => Err(CoreError::DiskFull {
            path: dir.to_owned(),
            needed: needed + DISK_RESERVE,
            available,
        }),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::events::{NullSink, Stage};
    use crate::net::Allowlist;

    const BODY: &[u8] = b"hello minecraft";
    // sha1("hello minecraft")
    fn body_sha1() -> String {
        hex::encode(<sha1::Sha1 as sha1::Digest>::digest(BODY))
    }

    fn downloader() -> Downloader {
        Downloader::new(Http::new(Allowlist::with_loopback()).unwrap(), 4)
            .with_backoff(Duration::from_millis(1), 3)
    }

    fn progress() -> Progress {
        Progress::new(Arc::new(NullSink), "t", Stage::Libraries)
    }

    fn item(server: &MockServer, dir: &Path, sha1: Option<String>) -> DownloadItem {
        DownloadItem {
            url: format!("{}/file.bin", server.uri()),
            dest: dir.join("sub").join("file.bin"),
            checksum: sha1.map(Checksum::Sha1),
            size: Some(BODY.len() as u64),
        }
    }

    #[tokio::test]
    async fn downloads_verifies_and_reuses() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/file.bin"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(BODY))
            .expect(1)
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let it = item(&server, dir.path(), Some(body_sha1()));
        let d = downloader();
        let c = CancellationToken::new();

        let s = d
            .run(vec![it.clone()], Verify::Full, &progress(), &c)
            .await
            .unwrap();
        assert_eq!(s.downloaded, 1);
        assert_eq!(std::fs::read(&it.dest).unwrap(), BODY);
        assert!(!part_path(&it.dest).exists());

        // Second run: valid file is reused without any request (expect(1)).
        let s = d
            .run(vec![it], Verify::Full, &progress(), &c)
            .await
            .unwrap();
        assert_eq!(s.reused, 1);
    }

    #[tokio::test]
    async fn retries_transient_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(2)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(BODY))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let it = item(&server, dir.path(), Some(body_sha1()));
        downloader()
            .run(
                vec![it.clone()],
                Verify::Quick,
                &progress(),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(std::fs::read(&it.dest).unwrap(), BODY);
    }

    #[tokio::test]
    async fn does_not_retry_404() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let err = downloader()
            .run(
                vec![item(&server, dir.path(), None)],
                Verify::Quick,
                &progress(),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code(), "net.httpStatus");
    }

    #[tokio::test]
    async fn resumes_partial_file_with_range() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(header("range", "bytes=6-"))
            .respond_with(ResponseTemplate::new(206).set_body_bytes(&BODY[6..]))
            .expect(1)
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let it = item(&server, dir.path(), Some(body_sha1()));
        std::fs::create_dir_all(it.dest.parent().unwrap()).unwrap();
        std::fs::write(part_path(&it.dest), &BODY[..6]).unwrap();

        downloader()
            .run(
                vec![it.clone()],
                Verify::Quick,
                &progress(),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(std::fs::read(&it.dest).unwrap(), BODY);
    }

    #[tokio::test]
    async fn checksum_mismatch_fails_after_retries() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(BODY))
            .expect(3)
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let it = item(&server, dir.path(), Some("deadbeef".into()));
        let err = downloader()
            .run(
                vec![it.clone()],
                Verify::Quick,
                &progress(),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code(), "download.hashMismatch");
        assert!(!it.dest.exists());
    }

    #[tokio::test]
    async fn blocks_urls_off_the_allowlist() {
        let dir = tempfile::tempdir().unwrap();
        let it = DownloadItem {
            url: "https://evil.example.com/x.jar".into(),
            dest: dir.path().join("x.jar"),
            checksum: None,
            size: None,
        };
        let err = downloader()
            .run(
                vec![it],
                Verify::Quick,
                &progress(),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code(), "net.notAllowed");
    }

    #[tokio::test]
    async fn cancellation_stops_work() {
        let dir = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        let c = CancellationToken::new();
        c.cancel();
        let err = downloader()
            .run(
                vec![item(&server, dir.path(), None)],
                Verify::Quick,
                &progress(),
                &c,
            )
            .await
            .unwrap_err();
        assert_eq!(err.code(), "task.cancelled");
    }

    #[test]
    fn backoff_grows_and_is_capped() {
        let b = Duration::from_millis(500);
        assert!(backoff(b, 1) <= Duration::from_millis(500));
        assert!(backoff(b, 3) >= Duration::from_millis(1000));
        assert!(backoff(b, 30) <= Duration::from_secs(30));
    }
}
