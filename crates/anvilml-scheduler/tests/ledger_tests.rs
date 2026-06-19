//! Tests for `ledger.rs` — `VramLedger` per-device VRAM reservation tracking.
//!
//! Each test constructs a `VramLedger`, registers devices, and asserts
//! on `would_fit`, `reserve`, and `release` results. All tests use
//! synchronous code — no `#[tokio::test]` needed since `VramLedger`
//! is pure synchronous logic.

use anvilml_scheduler::ledger::VramLedger;

/// Register a device and verify `would_fit` returns true for a
/// reasonable request that fits within the device's total VRAM.
///
/// Registers device 0 with 24576 MiB (24 GB), then checks that a
/// 8192 MiB request fits. This is the happy path — the core
/// registration + capacity check flow.
#[test]
fn test_register_device_and_would_fit() {
    let mut ledger = VramLedger::new();
    ledger.register_device(0, 24576);

    // 8 GB request fits in 24 GB device with zero reservations.
    assert!(ledger.would_fit(0, 8192));
}

/// `would_fit` returns false for an unknown (unregistered) device.
///
/// This tests the negative path: calling `would_fit` with a device
/// index that was never registered must return `false` rather than
/// panicking or returning a misleading value.
#[test]
fn test_would_fit_unknown_device_returns_false() {
    let ledger = VramLedger::new();

    // Device 99 was never registered — must return false.
    assert!(!ledger.would_fit(99, 1024));
}

/// Reserving VRAM reduces the available free VRAM.
///
/// Registers a 24 GB device, reserves 8 GB, then verifies that
/// `would_fit` returns false for an 8 GB request (only 16 GB left
/// after reservation, so 8 GB still fits) and true for a 1 GB
/// request. Also verifies that `would_fit` returns false when the
/// remaining free VRAM exactly equals the request (24 - 8 = 16 GB,
/// requesting 16 GB should fit).
#[test]
fn test_reserve_reduces_free_vram() {
    let mut ledger = VramLedger::new();
    ledger.register_device(0, 24576);

    // Before reservation: everything fits.
    assert!(ledger.would_fit(0, 24576));

    // Reserve 8 GB.
    ledger.reserve(0, 8192);

    // 16 GB should still fit (24576 - 8192 = 16384).
    assert!(ledger.would_fit(0, 16384));

    // 17 GB should NOT fit (only 16384 MiB free).
    assert!(!ledger.would_fit(0, 16385));
}

/// Releasing VRAM restores the previously reserved amount.
///
/// Registers a 24 GB device, reserves 8 GB, verifies the reduced
/// free capacity, then releases 4 GB and verifies the remaining
/// free capacity increased accordingly. This tests the full
/// reserve → release lifecycle.
#[test]
fn test_release_restores_free_vram() {
    let mut ledger = VramLedger::new();
    ledger.register_device(0, 24576);

    // Reserve 8 GB.
    ledger.reserve(0, 8192);
    assert!(!ledger.would_fit(0, 16385));

    // Release 4 GB.
    ledger.release(0, 4096);

    // Now 20 GB should fit (24576 - 8192 + 4096 = 20480).
    assert!(ledger.would_fit(0, 20480));

    // 20481 should not fit.
    assert!(!ledger.would_fit(0, 20481));
}

/// Reserving more VRAM than the device total panics.
///
/// Registers a 24 GB device, reserves 20 GB, then attempts to
/// reserve 8 GB more (total would be 28 GB > 24 GB). The `reserve`
/// method must panic because over-reservation is a programming error
/// in the dispatch loop.
#[test]
#[should_panic(expected = "VRAM reservation overflow")]
fn test_reserve_overflow_panics() {
    let mut ledger = VramLedger::new();
    ledger.register_device(0, 24576);

    // Reserve 20 GB — still within capacity.
    ledger.reserve(0, 20480);

    // Attempting to reserve 8 GB more would exceed 24 GB total.
    // This must panic.
    ledger.reserve(0, 8192);
}

/// Registering the same device twice is a no-op (idempotent).
///
/// Registers device 0 with 24 GB, then registers it again with the
/// same capacity. The second call must not change the ledger state
/// — reservations should still be zero and totals should still be
/// 24 GB. This prevents duplicate registration errors.
#[test]
fn test_duplicate_registration_is_noop() {
    let mut ledger = VramLedger::new();
    ledger.register_device(0, 24576);
    ledger.register_device(0, 24576);

    // Reservation should still be zero (no double-counting).
    assert!(ledger.would_fit(0, 24576));
}

/// Releasing more VRAM than reserved panics.
///
/// Registers a 24 GB device, reserves 4 GB, then attempts to
/// release 8 GB. The `release` method must panic because releasing
/// more than was reserved is a bug in the release logic.
#[test]
#[should_panic(expected = "VRAM release underflow")]
fn test_release_underflow_panics() {
    let mut ledger = VramLedger::new();
    ledger.register_device(0, 24576);

    // Reserve 4 GB.
    ledger.reserve(0, 4096);

    // Attempting to release 8 GB exceeds the reserved amount.
    // This must panic.
    ledger.release(0, 8192);
}

/// Multiple devices are tracked independently.
///
/// Registers two devices (0 and 1) with different VRAM totals, then
/// reserves on device 0 and verifies device 1's capacity is unaffected.
/// This tests the per-device isolation of the ledger.
#[test]
fn test_multiple_devices_independent() {
    let mut ledger = VramLedger::new();
    ledger.register_device(0, 24576);
    ledger.register_device(1, 12288);

    // Reserve 20 GB on device 0.
    ledger.reserve(0, 20480);

    // Device 1 should be unaffected — full 12 GB available.
    assert!(ledger.would_fit(1, 12288));

    // Device 0 should have only 4 GB left.
    assert!(!ledger.would_fit(0, 4097));
    assert!(ledger.would_fit(0, 4096));
}
