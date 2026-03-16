//! Serialized access control for Codex model requests.

use std::{
    future::Future,
    sync::Arc,
    time::Duration,
};

use arky_provider::ProviderError;
use tokio::{
    sync::{
        OwnedSemaphorePermit,
        Semaphore,
    },
    time::timeout,
};

/// Guard representing one scheduled model access slot.
#[derive(Debug)]
pub struct SchedulerPermit {
    _permit: OwnedSemaphorePermit,
    _label: String,
}

/// Serializes access to the Codex app-server model lane.
#[derive(Debug, Clone)]
pub struct Scheduler {
    semaphore: Arc<Semaphore>,
    acquire_timeout: Duration,
}

impl Scheduler {
    /// Creates a scheduler that allows only one active model request.
    #[must_use]
    pub fn new(acquire_timeout: Duration) -> Self {
        Self::with_limit(1, acquire_timeout)
    }

    /// Creates a scheduler with an explicit concurrency limit.
    #[must_use]
    pub fn with_limit(limit: usize, acquire_timeout: Duration) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(limit.max(1))),
            acquire_timeout,
        }
    }

    /// Acquires a scheduling permit, waiting up to the configured timeout.
    pub async fn acquire(
        &self,
        label: impl Into<String>,
    ) -> Result<SchedulerPermit, ProviderError> {
        let label = label.into();
        let permit =
            timeout(self.acquire_timeout, self.semaphore.clone().acquire_owned())
                .await
                .map_err(|_| {
                    ProviderError::stream_interrupted(format!(
                        "timed out waiting for scheduler slot for `{label}`"
                    ))
                })?
                .map_err(|_| {
                    ProviderError::stream_interrupted(format!(
                        "scheduler closed while waiting for `{label}`"
                    ))
                })?;

        Ok(SchedulerPermit {
            _permit: permit,
            _label: label,
        })
    }

    /// Runs a future while holding one scheduler slot.
    pub async fn run<F, T>(
        &self,
        label: impl Into<String>,
        future: F,
    ) -> Result<T, ProviderError>
    where
        F: Future<Output = Result<T, ProviderError>>,
    {
        let _permit = self.acquire(label).await?;
        future.await
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new(Duration::from_secs(300))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pretty_assertions::assert_eq;
    use tokio::{
        sync::{
            Barrier,
            oneshot,
        },
        time::{
            Duration,
            sleep,
        },
    };

    use super::Scheduler;

    #[tokio::test]
    async fn scheduler_should_serialize_access_until_first_permit_releases() {
        let scheduler = Scheduler::new(Duration::from_secs(1));
        let barrier = Arc::new(Barrier::new(2));
        let (first_release_tx, first_release_rx) = oneshot::channel::<()>();
        let (second_ran_tx, mut second_ran_rx) = oneshot::channel::<()>();

        let first_scheduler = scheduler.clone();
        let first_barrier = barrier.clone();
        let first_task = tokio::spawn(async move {
            let _permit = first_scheduler
                .acquire("first")
                .await
                .expect("first permit should acquire");
            first_barrier.wait().await;
            let _ = first_release_rx.await;
        });

        let second_scheduler = scheduler.clone();
        let second_barrier = barrier.clone();
        let second_task = tokio::spawn(async move {
            second_barrier.wait().await;
            let _permit = second_scheduler
                .acquire("second")
                .await
                .expect("second permit should eventually acquire");
            let _ = second_ran_tx.send(());
        });

        sleep(Duration::from_millis(30)).await;
        assert_eq!(second_ran_rx.try_recv().is_err(), true);

        let _ = first_release_tx.send(());
        first_task.await.expect("first task should finish");
        second_task.await.expect("second task should finish");
        second_ran_rx.await.expect("second task should run");
    }
}
