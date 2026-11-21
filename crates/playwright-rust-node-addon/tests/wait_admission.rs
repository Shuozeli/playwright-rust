#[path = "../src/wait_admission.rs"]
mod wait_admission;

use wait_admission::WaitAdmissions;

#[test]
fn long_waits_are_allocator_backed_without_a_fabricated_count_cap() {
    // Arrange
    let waits = WaitAdmissions::new();

    // Act
    let held: Vec<_> = (0..129)
        .map(|_| waits.admit().expect("long wait slot"))
        .collect();

    // Assert
    assert_eq!(held.len(), 129);
}

#[test]
fn cloned_sessions_do_not_reintroduce_a_process_local_wait_cap() {
    // Arrange
    let first_session = WaitAdmissions::new();
    let second_session = first_session.clone();
    let _held: Vec<_> = (0..129)
        .map(|_| first_session.admit().expect("long wait slot"))
        .collect();

    // Act
    let admitted = second_session.admit();

    // Assert
    assert!(admitted.is_ok());
}

#[test]
fn sequential_waits_remain_available_after_each_admission() {
    // Arrange
    let waits = WaitAdmissions::new();

    // Act
    let first = waits.admit();
    let second = waits.admit();

    // Assert
    assert!(first.is_ok());
    assert!(second.is_ok());
}
