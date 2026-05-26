//! quote-generator — concurrent HTTP fetcher.
//!
//! The library is runtime-agnostic-ish in API shape; it currently uses Tokio because
//! `reqwest`/`tokio::sync::Semaphore`/`tokio::time::timeout` all want a Tokio reactor.

use std::time::Duration;

use futures::stream::{FuturesUnordered, StreamExt};
use thiserror::Error;
use tokio::sync::Semaphore;
use tokio::time::timeout;

/// Outcome of a single URL fetch.
#[derive(Debug, Clone)]
pub struct Fetched {
    pub url: String,
    pub status: u16,
    pub body_preview: String,
    pub elapsed: Duration,
}

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("timeout fetching {url} after {after:?}")]
    Timeout { url: String, after: Duration },

    #[error("network error fetching {url}: {source}")]
    Network {
        url: String,
        #[source]
        source: reqwest::Error,
    },
}

/// One result per input URL.
pub type FetchResult = Result<Fetched, FetchError>;

/// Configuration for a `fetch_all` invocation.
#[derive(Debug, Clone)]
pub struct FetchConfig {
    /// Maximum number of in-flight requests.
    pub concurrency: usize,
    /// Per-request timeout. Applied around every individual `reqwest::get`.
    pub per_request_timeout: Duration,
    /// Optional overall deadline for the whole batch. `None` = no deadline.
    pub overall_deadline: Option<Duration>,
    /// How many bytes of body to keep in the preview.
    pub preview_bytes: usize,
}

impl Default for FetchConfig {
    fn default() -> Self {
        Self {
            concurrency: 4,
            per_request_timeout: Duration::from_secs(5),
            overall_deadline: None,
            preview_bytes: 200,
        }
    }
}

/// Fetch `urls` concurrently and yield results in **completion order** via the returned
/// stream. Results are not collected eagerly so a caller can act on each one as it arrives.
///
/// Bounded concurrency: at most `cfg.concurrency` requests are in flight at any time.
/// Per-request timeout: any single fetch that exceeds `cfg.per_request_timeout` is reported
/// as `FetchError::Timeout` and the rest of the batch continues.
///
/// **Note on `overall_deadline`:** to keep the library simple, the deadline is enforced
/// by callers wrapping the consumption of the stream in `tokio::time::timeout`. See
/// `main.rs` for the canonical pattern.
///
/// # Panics
///
/// Will not panic in normal use. The internal `Semaphore::acquire().await` is documented
/// to fail only if the semaphore is `.close()`d, which this function never does.
// `reqwest::Client` is internally an Arc — cloning is O(1) and idiomatic.
#[allow(clippy::needless_pass_by_value)]
pub fn fetch_all<I>(
    client: reqwest::Client,
    urls: I,
    cfg: FetchConfig,
) -> impl futures::Stream<Item = FetchResult>
where
    I: IntoIterator<Item = String>,
{
    let urls: Vec<String> = urls.into_iter().collect();
    let sem = std::sync::Arc::new(Semaphore::new(cfg.concurrency.max(1)));
    let cfg = std::sync::Arc::new(cfg);

    let futs = FuturesUnordered::new();
    for url in urls {
        let sem = sem.clone();
        let cfg = cfg.clone();
        let client = client.clone();
        futs.push(async move {
            let _permit = sem
                .acquire()
                .await
                .expect("semaphore is never closed by this function");
            fetch_one(&client, url, &cfg).await
        });
    }

    futs.boxed()
}

async fn fetch_one(client: &reqwest::Client, url: String, cfg: &FetchConfig) -> FetchResult {
    let started = std::time::Instant::now();
    let request = client.get(&url).send();

    let resp = match timeout(cfg.per_request_timeout, request).await {
        Err(_) => {
            return Err(FetchError::Timeout {
                url,
                after: cfg.per_request_timeout,
            });
        }
        Ok(Err(e)) => return Err(FetchError::Network { url, source: e }),
        Ok(Ok(r)) => r,
    };

    let status = resp.status().as_u16();
    let bytes_result = timeout(cfg.per_request_timeout, resp.bytes()).await;
    let bytes = match bytes_result {
        Err(_) => {
            return Err(FetchError::Timeout {
                url,
                after: cfg.per_request_timeout,
            });
        }
        Ok(Err(e)) => return Err(FetchError::Network { url, source: e }),
        Ok(Ok(b)) => b,
    };

    let preview_len = bytes.len().min(cfg.preview_bytes);
    let body_preview = String::from_utf8_lossy(&bytes[..preview_len]).into_owned();

    Ok(Fetched {
        url,
        status,
        body_preview,
        elapsed: started.elapsed(),
    })
}

/// Render a single result for stdout, one line per fetched URL.
#[must_use]
pub fn render(r: &FetchResult) -> String {
    match r {
        Ok(f) => format!("OK   {:>4} {:>8.1?}  {}", f.status, f.elapsed, f.url),
        Err(FetchError::Timeout { url, after }) => format!("TIME      {after:?}  {url}"),
        Err(FetchError::Network { url, source }) => format!("ERR        --  {url}  ({source})"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_ok_starts_with_ok() {
        let f = Fetched {
            url: "http://x".into(),
            status: 200,
            body_preview: "hi".into(),
            elapsed: Duration::from_millis(12),
        };
        let line = render(&Ok(f));
        assert!(line.starts_with("OK"));
        assert!(line.contains("200"));
        assert!(line.contains("http://x"));
    }

    #[test]
    fn render_timeout_starts_with_time() {
        let e = FetchError::Timeout {
            url: "http://x".into(),
            after: Duration::from_secs(2),
        };
        let line = render(&Err(e));
        assert!(line.starts_with("TIME"));
        assert!(line.contains("http://x"));
    }

    #[test]
    fn config_default_is_concurrency_4() {
        let c = FetchConfig::default();
        assert_eq!(c.concurrency, 4);
        assert_eq!(c.per_request_timeout, Duration::from_secs(5));
        assert!(c.overall_deadline.is_none());
    }

    #[tokio::test]
    async fn fetch_all_returns_one_result_per_url() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::any())
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("hi"))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let urls = vec![
            format!("{}/a", server.uri()),
            format!("{}/b", server.uri()),
            format!("{}/c", server.uri()),
        ];
        let mut stream = fetch_all(client, urls, FetchConfig::default());

        let mut count = 0;
        while let Some(r) = stream.next().await {
            assert!(r.is_ok());
            count += 1;
        }
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn fetch_all_reports_timeout_per_request() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::any())
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(400))
                    .set_body_string("slow"),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let cfg = FetchConfig {
            per_request_timeout: Duration::from_millis(100),
            ..FetchConfig::default()
        };
        let urls = vec![format!("{}/slow", server.uri())];
        let mut stream = fetch_all(client, urls, cfg);
        let r = stream.next().await.expect("one result");
        assert!(matches!(r, Err(FetchError::Timeout { .. })));
    }
}
