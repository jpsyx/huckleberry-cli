//! Ownership of the modal's single cancelable background check.

use std::future::Future;
use tokio::task::JoinHandle;

use crate::version::VersionReport;

/// One request, dropped on close so its result cannot reach a later modal.
#[derive(Default)]
pub struct VersionTask {
    pending: Option<JoinHandle<VersionReport>>,
}

impl VersionTask {
    /// Start only if the previous check is no longer owned by this manager.
    pub fn start(&mut self, future: impl Future<Output = VersionReport> + Send + 'static) -> bool {
        if self.pending.is_some() {
            return false;
        }
        self.pending = Some(tokio::spawn(future));
        true
    }

    /// Poll completion without ever waiting for an unfinished network request.
    pub async fn collect(&mut self) -> Option<VersionReport> {
        if !self.pending.as_ref().is_some_and(JoinHandle::is_finished) {
            return None;
        }
        let result = self.pending.take()?.await;
        Some(result.unwrap_or_else(|_| {
            VersionReport::unavailable(env!("CARGO_PKG_VERSION"), "The version check stopped")
        }))
    }

    /// Abort and discard the current request on close or shell exit.
    pub fn cancel(&mut self) {
        if let Some(pending) = self.pending.take() {
            pending.abort();
        }
    }
}

impl Drop for VersionTask {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::VersionTask;
    use crate::version::{UpdateStatus, VersionReport};
    use std::time::Duration;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn a_pending_check_never_blocks_collection_or_starts_twice() {
        let mut task = VersionTask::default();
        assert!(task.start(std::future::pending()));
        assert!(!task.start(std::future::pending()));
        assert!(
            tokio::time::timeout(Duration::from_millis(20), task.collect())
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn cancel_and_reopen_cannot_deliver_the_old_result() {
        let (sender, receiver) = oneshot::channel();
        let mut task = VersionTask::default();
        task.start(async { receiver.await.unwrap() });
        task.cancel();
        tokio::task::yield_now().await;
        assert!(sender.send(VersionReport::offline("old")).is_err());
        assert!(task.start(async { VersionReport::offline("new") }));
        tokio::task::yield_now().await;
        assert_eq!(task.collect().await.unwrap().installed, "new");
        assert!(task.collect().await.is_none());
    }

    #[tokio::test]
    async fn dropping_the_task_manager_aborts_its_request() {
        let (sender, receiver) = oneshot::channel::<()>();
        let mut task = VersionTask::default();
        task.start(async move {
            let _sender = sender;
            std::future::pending().await
        });
        drop(task);
        assert!(
            tokio::time::timeout(Duration::from_secs(1), receiver)
                .await
                .unwrap()
                .is_err()
        );
    }

    #[tokio::test]
    async fn a_task_failure_is_reported_as_unavailable() {
        let mut task = VersionTask::default();
        task.start(async { panic!("simulated version task failure") });
        tokio::task::yield_now().await;
        assert_eq!(
            task.collect().await.unwrap().status,
            UpdateStatus::Unavailable
        );
    }
}
