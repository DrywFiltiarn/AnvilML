# Addendum: `EnumerationSource::Cpu` variant

**Status:** Resolved this session. Recorded here because this session's tooling has
read-only access to `docs/ANVILML_DESIGN.md` via `project_knowledge_search` — the
actual repository file must be hand-edited by whoever next has write access, using
the exact diff below. Phases 3 and 4's task definitions in this delivery already
assume this change is in effect.

---

## Background

Phase 4's `CpuDetector` (task `P4-A2`) needs to set `GpuDevice.enumeration_source` for
the synthesized CPU fallback device. `ANVILML_DESIGN.md §5.5`'s `EnumerationSource`
enum, as originally specified, was:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum EnumerationSource { Vulkan, Dxgi, Sysfs, Nvml, Mock, Override }
```

None of the six existing variants is a correct semantic fit for "this device was
synthesized by the unconditional CPU fallback, not actually enumerated by anything."
The prior session flagged this as a Deviation rather than guessing.

## Resolution

Add a seventh variant, `Cpu`, to the enum:

```diff
 #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
-pub enum EnumerationSource { Vulkan, Dxgi, Sysfs, Nvml, Mock, Override }
+pub enum EnumerationSource { Vulkan, Dxgi, Sysfs, Nvml, Mock, Override, Cpu }
```

No other field on `GpuDevice`, no other enum, and no migration/seed data is affected
— this is an additive, non-breaking change to one enum's variant list. `serde`'s
default `rename_all` behavior for this enum (if any is configured elsewhere) should
be re-checked for the new variant's serialized form once the live edit lands, but no
`rename_all` attribute is present on this enum as specified, so it serializes as the
literal string `"Cpu"` by default — consistent with the sibling `DeviceType::Cpu`
variant's own default serialization.

## Where this is reflected in this delivery

- **`tasks/tasks_phase003.json` / `docs/TASKS_PHASE003.md`**, task `P3-A5`: now
  specifies `EnumerationSource` with all seven variants, `Cpu` included, at the
  point the enum is first defined.
- **`tasks/tasks_phase004.json` / `docs/TASKS_PHASE004.md`**, task `P4-A2`: now sets
  `enumeration_source: EnumerationSource::Cpu` directly, with the prior session's
  Deviation note removed since the gap it flagged is now closed.

## Action required by the repository maintainer

Apply the diff above to the live `docs/ANVILML_DESIGN.md §5.5` before or during
Phase 3/4 implementation, so the agent's `project_knowledge_search` reads reflect the
corrected enum when those phases are actually executed.
