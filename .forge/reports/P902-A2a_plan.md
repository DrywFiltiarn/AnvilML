# Plan Report: P902-A2a

| Field | Value |
|-------|-------|
| Task ID | P902-A2a |
| Phase | 902 — Stabilisation Retrofit |
| Description | Unify WorkerPool.workers with shared_workers (pool.rs) |
| Depends on | none |
| Project | anvilml |
| Planned at | 2026-06-07T21:30:00Z |
| Attempt | 1 |

## Objective

Eliminate the split-brain between `WorkerPool.workers` (a plain `Vec<Arc<ManagedWorker>>`) and `shared_workers` (`Arc<RwLock<Vec>>`). After the first worker respawn, only `shared_workers` contains the replacement — all public methods iterate the stale original `Vec`, permanently operating on dead workers. The fix: promote `workers` to `Arc<RwLock<Vec<Arc<ManagedWorker>>>>`, make it the single source of truth, and have `shared_workers` alias the same Arc.

## Scope

### In Scope
- `crates/anvilml-worker/src/pool.rs` — struct field type change, construction sites, public method accessors, test-only accessor
- `crates/anvilml-worker/Cargo.toml` — bump patch version from `0.1.10` to `0.1.11`

### Out of Scope
- Any other crate (scheduler, server, backend)
- P902-A2b (respawn event listener spawn) — separate task
- P902-A4 (env isolation), P902-A5 (IPC DEBUG logs), P902-A6 (pool DEBUG logs) — separate tasks
- `managed.rs`, `lib.rs`, `env.rs` — no changes required

## Approach

Six numbered steps, each implementing one change listed in the task spec. All within `crates/anvilml-worker/src/pool.rs`.

### Step 1: Change the struct field type

Replace line 30 of pool.rs:
```rust
workers: Vec<Arc<ManagedWorker>>,
```
with:
```rust
workers: Arc<RwLock<Vec<Arc<ManagedWorker>>>>,
```
No other import changes needed — `Arc` and `RwLock` are already imported at the top.

### Step 2: Update construction in `spawn_all` (GPU loop + CPU fallback)

Change line 63 from:
```rust
let mut workers: Vec<Arc<ManagedWorker>> = Vec::with_capacity(hw.gpus.len().max(1));
```
to:
```rust
let workers: Arc<RwLock<Vec<Arc<ManagedWorker>>>> = Arc::new(RwLock::new(Vec::with_capacity(hw.gpus.len().max(1))));
```

Both `push` sites (lines 71 and 98) change from:
```rust
workers.push(worker);
```
to:
```rust
workers.write().await.push(worker);
```

### Step 3: Replace `shared_workers` construction

Remove line 109:
```rust
let shared_workers = Arc::new(tokio::sync::RwLock::new(workers.clone()));
```
Replace with:
```rust
let shared_workers = workers.clone();
```
Both names now refer to the same `Arc<RwLock<Vec>>`. The respawn task (line 151+) already uses `shared_workers` correctly via `workers_clone.read().await` / `.write().await` — no changes needed inside the respawn closure.

### Step 4: Update `pool_workers` snapshot (line 103)

Change from:
```rust
let pool_workers = workers.clone();
```
to:
```rust
let pool_workers = { let l = workers.read().await; l.clone() };
```

### Step 5: Update all public method accessors to use read-lock

For each of these methods, add `let locked = self.workers.read().await;` and iterate over `&*locked`:

- **`list()`** (line 295–301): Replace `self.workers` with the locked snapshot
- **`acquire_idle()`** (line 307–321): Same pattern
- **`set_busy()`** (line 324–333): Same pattern
- **`set_idle()`** (line 336–345): Same pattern
- **`send()`** (line 353–366): Same pattern

Each method follows the same transformation:
```rust
// Before:
for worker in &self.workers { ... }

// After:
let locked = self.workers.read().await;
for worker in &*locked { ... }
```

### Step 6: Update test-only accessor and struct literal construction sites

**`pid_for()`** (line 382–389): Same read-lock pattern as other public methods.

**Test struct literals** — three tests construct `WorkerPool` manually:
- `spawn_all_creates_cpu_worker_when_no_gpus` (line 537)
- `pid_for_returns_none_for_missing_worker` (line 610)
- `pid_for_returns_child_pid_when_spawned` (line 656)

Each test's `workers: vec![...]` becomes:
```rust
workers: Arc::new(RwLock::new(vec![...])),
```

Direct `pool.workers[0]` accesses in tests (lines 577, 673) become:
```rust
{ let l = pool.workers.read().await; l[0].set_status(WorkerStatus::Idle).await }
// and:
{ let l = pool.workers.read().await; l[0].set_child_for_test(dummy).await }
```

### Step 7: Bump crate version

Update `crates/anvilml-worker/Cargo.toml` line 3 from:
```toml
version = "0.1.10"
```
to:
```toml
version = "0.1.11"
```

### Step 8: Verification (PLAN session pre-stop)

Run the three verification commands:
1. `head -1 .forge/reports/P902-A2a_plan.md` — must print `# Plan Report: P902-A2a`
2. `grep "^## " .forge/reports/P902-A2a_plan.md` — must show all 8 section headings
3. `wc -l .forge/reports/P902-A2a_plan.md` — must be > 30 lines

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/pool.rs` | Unify workers field, update all accessors, fix test struct literals |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version 0.1.10 → 0.1.11 |

## Tests

<table>
<tr><th>Test File</th><th>Test Name</th><th>What It Verifies</th></tr>
<tr><td>crates/anvilml-worker/src/pool.rs (mod tests)</td><td>pool_event_listener_merges_ready_capabilities</td><td>No struct literal change — test uses broadcast channel directly, no WorkerPool construction. Unchanged.</td></tr>
<tr><td>crates/anvilml-worker/src/pool.rs (mod tests)</td><td>spawn_all_creates_cpu_worker_when_no_gpus</td><td>Struct literal updated: `workers` field is Arc::new(RwLock::new(vec![...])); direct index access wrapped in read-lock. Verifies list() and status transitions still work.</td></tr>
<tr><td>crates/anvilml-worker/src/pool.rs (mod tests)</td><td>pid_for_returns_none_for_missing_worker</td><td>Struct literal updated; pid_for() uses read-lock internally. Verifies None for existing and missing workers.</td></tr>
<tr><td>crates/anvilml-worker/src/pool.rs (mod tests)</td><td>pid_for_returns_child_pid_when_spawned</td><td>Struct literal updated; direct index access via read-lock. Verifies PID returns correctly.</td></tr>
</table>

## CI Impact

No CI workflow changes required. The task only modifies source code and a Cargo.toml within `anvilml-worker`. The existing CI gates (clippy, test, format, cross-check) apply automatically. No new gates needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Read-lock contention in hot path — `list()` called on every HTTP request to `/v1/workers` | Low | Negligible — RwLock read is fast; pool has at most ~8 workers | Accept as-is; benchmark if latency issues surface later |
| Test struct literal updates miss a site | Medium | Compilation failure in tests | Run `cargo test -p anvilml-worker --features mock-hardware` after changes; all 4 tests must pass |
| `pool_workers` clone semantics change — was a shallow Vec clone, now a read-lock snapshot clone | Low | Logic change if pool_workers is mutated later (it isn't) | The task spec explicitly requires this change; respawn task already uses shared_workers correctly |
| Import conflict: `tokio::sync::RwLock` vs `std::sync::RwLock` | None | Compile error | Already imported as `use tokio::sync::{broadcast, RwLock};` at top of file — no change needed |

## Acceptance Criteria

- [ ] `cargo clippy -p anvilml-worker --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0 (all 4 pool tests passing)
- [ ] No public API signature changes on `WorkerPool` — only internal access pattern changed
- [ ] Version bumped to `0.1.11` in `crates/anvilml-worker/Cargo.toml`
