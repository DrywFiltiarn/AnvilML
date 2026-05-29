# PHASES.md — AnvilML Phase Registry

**Location:** `forge/docs/PHASES.md`  
**Authoritative source for:** phase number → name → milestone mapping  
**Referenced by:** `.clinerules §12`, `FORGE_TASK_AUTHORING_SPEC.md §6`, all `TASKS_PHASE<NNN>.md` headers

Task IDs use the short phase number (no leading zeros): `P1-A1`, `P2-C3`.  
File names use zero-padded three-digit numbers: `tasks_phase001.json`, `TASKS_PHASE001.md`.

---

## Phase Map

| Phase | Name                        | ANVIL Milestone | Status    | Task file                       | Phase doc                    |
|------:|-----------------------------|-----------------|-----------|---------------------------------|------------------------------|
|   001 | Workspace Scaffold          | M0              | Draft     | `tasks/tasks_phase001.json`     | `docs/TASKS_PHASE001.md`     |
|   002 | Core Types & IPC            | M1              | Draft     | `tasks/tasks_phase002.json`     | `docs/TASKS_PHASE002.md`     |
|   003 | Hardware Detection          | M1              | Draft     | `tasks/tasks_phase003.json`     | `docs/TASKS_PHASE003.md`     |
|   004 | Persistence & Model Registry| M2              | Planned   | `tasks/tasks_phase004.json`     | `docs/TASKS_PHASE004.md`     |
|   005 | Worker Management           | M2              | Planned   | `tasks/tasks_phase005.json`     | `docs/TASKS_PHASE005.md`     |
|   006 | Scheduler                   | M3              | Planned   | `tasks/tasks_phase006.json`     | `docs/TASKS_PHASE006.md`     |
|   007 | HTTP & WebSocket Server     | M4              | Planned   | `tasks/tasks_phase007.json`     | `docs/TASKS_PHASE007.md`     |
|   008 | Launcher & Graceful Shutdown| M4              | Planned   | `tasks/tasks_phase008.json`     | `docs/TASKS_PHASE008.md`     |
|   009 | Python Worker — ZiT         | M5              | Planned   | `tasks/tasks_phase009.json`     | `docs/TASKS_PHASE009.md`     |
|   010 | SDXL & Hardening            | M6              | Planned   | `tasks/tasks_phase010.json`     | `docs/TASKS_PHASE010.md`     |

> BloomeryUI phases (011+) are deferred until the BloomeryUI design document exists.  
> SindriStudio integration phases are deferred until both AnvilML and BloomeryUI are functional.

---

## Milestone Summary

| Milestone | Phases  | Exit Criterion |
|-----------|---------|----------------|
| **M0 — Scaffold**              | 001     | `cargo build/test --workspace --features mock-hardware` exits 0 |
| **M1 — Core & Contracts**      | 002–003 | Round-trip + detector fixture tests green; `openapi.json` generates |
| **M2 — Persistence & Workers** | 004–005 | Mock Python worker does `Ping→Pong`; models scan into DB |
| **M3 — Scheduling**            | 006     | Cycle/unknown-type rejection; dispatch + cancel work |
| **M4 — Server & API**          | 007–008 | All `api_*.rs` integration tests green; release binary starts |
| **M5 — Python Worker (ZiT)**   | 009     | `Execute→Progress→ImageReady→Completed` in mock; ZiT smoke on hardware |
| **M6 — SDXL & Hardening**      | 010     | Both pipelines run; cancel + crash-recovery smoke pass; CI green |

---

## Phase Dependency Chain

```
001 (Scaffold)
 └─► 002 (Core Types & IPC)
      └─► 003 (Hardware Detection)
           └─► 004 (Persistence & Model Registry)
                └─► 005 (Worker Management)
                     └─► 006 (Scheduler)
                          └─► 007 (HTTP & WebSocket Server)
                               └─► 008 (Launcher & Graceful Shutdown)
                                    └─► 009 (Python Worker — ZiT)
                                         └─► 010 (SDXL & Hardening)
```

Each phase gate: the first task of phase N+1 carries a `prereq` on the terminal task(s) of phase N.

---

## Registered Projects

| Project      | Repository  | Scope                                       |
|--------------|-------------|---------------------------------------------|
| `anvilml`    | `AnvilML/`  | Rust backend + Python worker (phases 001–010) |
| `bloomeryui` | `BloomeryUI/` | TypeScript frontend (phases 011+, deferred) |
| `sindristudio` | `SindriStudio/` | Integration / mono-repo shell (deferred) |

---

*Last updated: generated from ANVILML_DESIGN.md Rev 3 (M0–M6 roadmap).*
