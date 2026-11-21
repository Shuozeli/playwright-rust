//! Bounded retirement for the one native session driver.

use std::future::Future;
use std::time::Duration;

use tokio::task::JoinHandle;

use crate::HostError;

pub(crate) struct OnDrop<Callback>
where
    Callback: FnOnce(),
{
    callback: Option<Callback>,
}

impl<Callback> OnDrop<Callback>
where
    Callback: FnOnce(),
{
    pub(crate) const fn new(callback: Callback) -> Self {
        Self {
            callback: Some(callback),
        }
    }
}

impl<Callback> Drop for OnDrop<Callback>
where
    Callback: FnOnce(),
{
    fn drop(&mut self) {
        if let Some(callback) = self.callback.take() {
            callback();
        }
    }
}

/// Deadlines for graceful and post-disconnect native-driver retirement.
///
/// The graceful window preserves browser-side cleanup receipts. If it expires,
/// the caller disconnects the borrowed transport and gives local cleanup the
/// forced window before aborting the driver task as a final safety boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostSessionCloseTimeouts {
    pub graceful: Duration,
    pub forced: Duration,
}

impl HostSessionCloseTimeouts {
    pub const fn new(graceful: Duration, forced: Duration) -> Self {
        Self { graceful, forced }
    }

    pub(crate) fn validate(self) -> Result<Self, HostError> {
        if self.graceful.is_zero() {
            return Err(HostError::InvalidArgument(
                "HostSession graceful close timeout must be nonzero".into(),
            ));
        }
        if self.forced.is_zero() {
            return Err(HostError::InvalidArgument(
                "HostSession forced close timeout must be nonzero".into(),
            ));
        }
        Ok(self)
    }
}

impl Default for HostSessionCloseTimeouts {
    fn default() -> Self {
        Self::new(Duration::from_secs(30), Duration::from_secs(5))
    }
}

pub(crate) async fn join_driver_bounded<Disconnect, DisconnectResult>(
    driver: &mut JoinHandle<Result<(), HostError>>,
    timeouts: HostSessionCloseTimeouts,
    disconnect: Disconnect,
) -> Result<(), HostError>
where
    Disconnect: FnOnce() -> DisconnectResult,
    DisconnectResult: Future<Output = ()>,
{
    match tokio::time::timeout(timeouts.graceful, &mut *driver).await {
        Ok(result) => return result.map_err(HostError::backend)?,
        Err(_) => disconnect().await,
    }

    match tokio::time::timeout(timeouts.forced, &mut *driver).await {
        Ok(result) => result.map_err(HostError::backend)?,
        Err(_) => {
            driver.abort();
            let _ = driver.await;
            Err(HostError::backend(format_args!(
                "HostSession teardown did not finish within {:?} after forced transport disconnect",
                timeouts.forced
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use tokio::sync::Notify;

    use super::*;

    struct DropWitness(Arc<AtomicBool>);

    impl Drop for DropWitness {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn graceful_timeout_disconnects_and_joins_a_released_driver() {
        // Arrange
        let release = Arc::new(Notify::new());
        let disconnects = Arc::new(AtomicUsize::new(0));
        let driver_release = Arc::clone(&release);
        let mut driver = tokio::spawn(async move {
            driver_release.notified().await;
            Ok(())
        });
        let disconnect_release = Arc::clone(&release);
        let disconnect_count = Arc::clone(&disconnects);

        // Act
        let result = join_driver_bounded(
            &mut driver,
            HostSessionCloseTimeouts::new(Duration::from_millis(10), Duration::from_millis(10)),
            move || async move {
                disconnect_count.fetch_add(1, Ordering::SeqCst);
                disconnect_release.notify_one();
            },
        )
        .await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(disconnects.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn forced_timeout_aborts_and_drops_a_stuck_driver() {
        // Arrange
        let dropped = Arc::new(AtomicBool::new(false));
        let disconnects = Arc::new(AtomicUsize::new(0));
        let driver_dropped = Arc::clone(&dropped);
        let mut driver = tokio::spawn(async move {
            let _witness = DropWitness(driver_dropped);
            std::future::pending::<()>().await;
            Ok(())
        });
        let disconnect_count = Arc::clone(&disconnects);

        // Act
        let error = join_driver_bounded(
            &mut driver,
            HostSessionCloseTimeouts::new(Duration::from_millis(10), Duration::from_millis(10)),
            move || async move {
                disconnect_count.fetch_add(1, Ordering::SeqCst);
            },
        )
        .await
        .expect_err("a permanently stuck driver must hit the hard boundary");

        // Assert
        assert!(
            error
                .to_string()
                .contains("after forced transport disconnect")
        );
        assert_eq!(disconnects.load(Ordering::SeqCst), 1);
        assert!(dropped.load(Ordering::SeqCst));
    }

    #[test]
    fn zero_deadlines_are_rejected() {
        // Arrange
        let zero_grace = HostSessionCloseTimeouts::new(Duration::ZERO, Duration::from_secs(1));
        let zero_forced = HostSessionCloseTimeouts::new(Duration::from_secs(1), Duration::ZERO);

        // Act
        let grace_error = zero_grace.validate().expect_err("zero grace must fail");
        let forced_error = zero_forced.validate().expect_err("zero forced must fail");

        // Assert
        assert!(grace_error.to_string().contains("graceful"));
        assert!(forced_error.to_string().contains("forced"));
    }

    #[test]
    fn close_cancellation_guard_runs_exactly_once() {
        // Arrange
        let disconnects = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&disconnects);
        let guard = OnDrop::new(move || {
            observed.fetch_add(1, Ordering::SeqCst);
        });

        // Act
        drop(guard);

        // Assert
        assert_eq!(disconnects.load(Ordering::SeqCst), 1);
    }
}
