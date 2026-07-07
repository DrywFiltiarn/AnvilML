/// Integration tests for `VramLedger` — per-device VRAM reservation tracking.
///
/// Each test constructs `VramLedger` via the public API and exercises
/// `reserve()`, `release()`, and `free_mib()` to verify correct behaviour
/// across single-device and multi-device scenarios.
use anvilml_scheduler::VramLedger;

/// Test that `reserve()` correctly reduces the amount of free VRAM.
///
/// Creates an empty ledger, reserves 4096 MiB on device 0, then calls
/// `free_mib(0, 8192)`. The result must be 4096 (8192 - 4096 = 4096).
#[test]
fn test_reserve_reduces_free_mib() {
    let mut ledger = VramLedger::new();
    ledger.reserve(0, 4096);
    assert_eq!(
        ledger.free_mib(0, 8192),
        4096,
        "free_mib must return total - reserved (8192 - 4096 = 4096)"
    );
}

/// Test that `release()` restores previously reserved capacity.
///
/// Reserves 4096 MiB on device 0, then releases 4096 MiB. After release,
/// `free_mib(0, 8192)` must return 8192 (the full capacity).
#[test]
fn test_release_restores_capacity() {
    let mut ledger = VramLedger::new();
    ledger.reserve(0, 4096);
    ledger.release(0, 4096);
    assert_eq!(
        ledger.free_mib(0, 8192),
        8192,
        "free_mib must return total after full release (8192 - 0 = 8192)"
    );
}

/// Test that releasing more than reserved uses saturating subtraction
/// and never panics.
///
/// Reserves 4096 MiB on device 0, then releases 8192 MiB (double the
/// reservation). The release must clamp the reservation to zero rather
/// than underflowing. `free_mib(0, 8192)` must return 8192.
#[test]
fn test_over_release_does_not_panic() {
    let mut ledger = VramLedger::new();
    ledger.reserve(0, 4096);
    ledger.release(0, 8192);
    assert_eq!(
        ledger.free_mib(0, 8192),
        8192,
        "free_mib must return total after over-release (reservation clamped to 0)"
    );
}

/// Test that `free_mib()` returns `total_mib` for an unknown device.
///
/// Creates an empty ledger and never reserves on any device. Calls
/// `free_mib(5, 16384)` for device index 5. Must return 16384 since
/// the device has zero reservation.
#[test]
fn test_unknown_device_returns_total_mib() {
    let ledger = VramLedger::new();
    assert_eq!(
        ledger.free_mib(5, 16384),
        16384,
        "free_mib must return total for an unknown device (zero reservation)"
    );
}

/// Test that multiple `reserve()` calls on the same device accumulate.
///
/// Reserves 4096 MiB on device 0 twice (total 8192 MiB). Calls
/// `free_mib(0, 8192)` which must return 0 (8192 - 8192 = 0).
#[test]
fn test_reserve_accumulates_on_same_device() {
    let mut ledger = VramLedger::new();
    ledger.reserve(0, 4096);
    ledger.reserve(0, 4096);
    assert_eq!(
        ledger.free_mib(0, 8192),
        0,
        "free_mib must return 0 when total reserved equals total capacity"
    );
}

/// Test that reservations on different devices are independent.
///
/// Reserves 4096 MiB on device 0 and 2048 MiB on device 1. Device 0
/// must show 4096 free (of 8192) and device 1 must show 6144 free
/// (of 8192), proving the devices do not share a reservation counter.
#[test]
fn test_multi_device_independent() {
    let mut ledger = VramLedger::new();
    ledger.reserve(0, 4096);
    ledger.reserve(1, 2048);

    assert_eq!(
        ledger.free_mib(0, 8192),
        4096,
        "Device 0 must show 4096 free (8192 - 4096), independent of device 1"
    );
    assert_eq!(
        ledger.free_mib(1, 8192),
        6144,
        "Device 1 must show 6144 free (8192 - 2048), independent of device 0"
    );
}
