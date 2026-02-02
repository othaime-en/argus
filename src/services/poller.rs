use chrono::{Duration, Utc};
use std::time::Duration as StdDuration;
use tokio::sync::mpsc;

use crate::api::CIPlatform;
use crate::models::Pipeline;

/// Messages sent from polling tasks to the main application loop.
#[derive(Debug)]
pub enum PollUpdate {
    /// A source successfully returned new pipeline data.
    /// The `String` is the source name from config.
    PipelinesUpdated(String, Vec<Pipeline>),

    /// A source encountered an error during polling.
    Error(String, String), // (source_name, human-readable message)

    /// A source was rate-limited.  The Duration is how long to wait before
    /// the next attempt.
    RateLimited(String, Duration),
}

/// The background service that keeps pipeline data fresh.
///
/// Each configured CI source gets its own tokio task.  Tasks are completely
/// independent: one source being down or rate-limited does not affect others.
///
/// The service communicates exclusively through a channel.  The main event loop
/// calls `receiver()` to get the reading end, then polls it on every UI tick.
pub struct PipelinePoller {
    /// Channel sender – cloned into each spawned task
    tx: mpsc::Sender<PollUpdate>,
    /// Channel receiver – kept here so `main` can take it via `receiver()`
    rx: Option<mpsc::Receiver<PollUpdate>>,
}

impl PipelinePoller {
    /// Create a new poller.  No tasks are spawned yet; call `start()` next.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(64);
        Self { tx, rx: Some(rx) }
    }

    /// Take ownership of the receiver end.  Must be called exactly once before
    /// the event loop begins.
    pub fn receiver(&mut self) -> mpsc::Receiver<PollUpdate> {
        self.rx.take().expect("receiver() called more than once")
    }

    /// Launch a background polling task for the given source.
    ///
    /// `source` – the `CIPlatform` implementation (e.g. `GitHubClient`)
    /// `interval_secs` – base polling interval in seconds (jitter is added)
    ///
    /// The task runs forever until the channel is closed (i.e. when the
    /// `PipelinePoller` is dropped).
    pub fn start(&self, source: Box<dyn CIPlatform>, interval_secs: u64) {
        let tx = self.tx.clone();
        let source_name = source.source_name().to_string();

        tokio::spawn(async move {
            // Run an immediate first poll so the UI isn't empty for 30 s
            poll_once(&source, &source_name, &tx).await;

            loop {
                // Add ±5 s jitter to avoid thundering-herd when many sources
                // share the same base interval
                let jitter = {
                    // Simple deterministic-ish jitter: rotate through -5..+5
                    let nanos = Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64;
                    let offset = (nanos % 11) as i64 - 5; // -5 … +5
                    offset
                };
                let effective = (interval_secs as i64 + jitter).max(5) as u64;
                tokio::time::sleep(StdDuration::from_secs(effective)).await;

                // If the channel is closed (main loop exited) we stop
                if tx.is_closed() {
                    break;
                }

                poll_once(&source, &source_name, &tx).await;
            }
        });
    }
}

/// Perform a single poll cycle for one source and send the result.
async fn poll_once(
    source: &dyn CIPlatform,
    source_name: &str,
    tx: &mpsc::Sender<PollUpdate>,
) {
    match source.fetch_pipelines().await {
        Ok(pipelines) => {
            let _ = tx
                .send(PollUpdate::PipelinesUpdated(
                    source_name.to_string(),
                    pipelines,
                ))
                .await;
        }
        Err(e) => {
            // Check if the underlying error is a rate-limit
            let is_rate_limited = e.to_string().contains("Rate limit exceeded");

            if is_rate_limited {
                let _ = tx
                    .send(PollUpdate::RateLimited(
                        source_name.to_string(),
                        Duration::seconds(60),
                    ))
                    .await;
            } else {
                let _ = tx
                    .send(PollUpdate::Error(source_name.to_string(), e.to_string()))
                    .await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poller_creates_channel() {
        let mut poller = PipelinePoller::new();
        let rx = poller.receiver();
        // Receiver should be valid (not panicked)
        assert!(!rx.is_closed());
    }

    #[test]
    #[should_panic(expected = "receiver() called more than once")]
    fn test_poller_panics_on_double_receiver() {
        let mut poller = PipelinePoller::new();
        let _ = poller.receiver();
        let _ = poller.receiver(); // should panic
    }

    #[tokio::test]
    async fn test_poll_update_channel() {
        let (tx, mut rx) = mpsc::channel(8);

        let update = PollUpdate::PipelinesUpdated("test-source".into(), vec![]);
        tx.send(update).await.unwrap();

        let received = rx.recv().await.unwrap();
        match received {
            PollUpdate::PipelinesUpdated(name, pipes) => {
                assert_eq!(name, "test-source");
                assert!(pipes.is_empty());
            }
            _ => panic!("Expected PipelinesUpdated"),
        }
    }

    #[tokio::test]
    async fn test_poll_update_error_variant() {
        let (tx, mut rx) = mpsc::channel(8);
        tx.send(PollUpdate::Error("src".into(), "network down".into()))
            .await
            .unwrap();

        match rx.recv().await.unwrap() {
            PollUpdate::Error(name, msg) => {
                assert_eq!(name, "src");
                assert_eq!(msg, "network down");
            }
            _ => panic!("Expected Error variant"),
        }
    }
}