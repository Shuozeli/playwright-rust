//! Allocator-backed language-binding session admission.
//!
//! Upstream Playwright does not impose a finite count on live driver sessions.
//! This helper protects only the machine-word counter from arithmetic overflow;
//! it does not reject an ordinary object count.

use crate::HostError;

pub fn admit_pending(pending: &mut usize, binding: &str) -> Result<(), HostError> {
    *pending = pending.checked_add(1).ok_or_else(|| {
        HostError::ResourceExhausted(format!(
            "{binding} environment pending session count overflowed"
        ))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_session_admission_grows_past_the_legacy_count() {
        // Arrange
        let mut pending = 0;

        // Act
        for _ in 0..129 {
            admit_pending(&mut pending, "test")
                .expect("pending session admission should be allocator-backed");
        }

        // Assert
        assert_eq!(pending, 129);
    }

    #[test]
    fn pending_session_admission_rejects_only_counter_overflow() {
        // Arrange
        let mut pending = usize::MAX;

        // Act
        let error = admit_pending(&mut pending, "test").expect_err("counter overflow");

        // Assert
        assert!(matches!(error, HostError::ResourceExhausted(_)));
        assert_eq!(pending, usize::MAX);
    }
}
