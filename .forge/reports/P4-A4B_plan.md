# Plan Report: P4-A4B

| Field | Value |
|-------|-------|
| Task ID | P4-A4B |
| Phase | 004 - Hardware Detection |
| Description | anvilml-hardware device_db PCI-ID capability table + resolution |
| Depends on | P4-A1, P4-A2, P4-A3, P4-A4 (all committed) |
| Project | AnvilML |
| Planned at | 2026-06-03T17:58:00Z |
| Attempt | 1 |


## Objective

Create crates/anvilml-hardware/src/device_db.rs. A hardcoded compile-time PCI-ID capability table mapping (vendor_id, device_id) tuples to model name, architecture string and ML inference capabilities including fp16 bf lG flash-attention Provide a const-slice lookup function returning an Option reference for the given vendor plus device IDs Seed with representative NVIDIA Ampere Hopper Turing AMD RDNA3 CDNA cards — no VRAM values in table Include unit tests that verify correct lookups miss behaviour None and seed entry integrity via filter --device_db


## Scope

### In Scope
- Create only crates/anvilml-hardware/src/device_db.rs. All types data functions and tests go into this single file Only modification to another file is adding a pub mod device db line in lib rs No other files created or modified beyond that one-line module registration
- Define the Rust type struct DeviceCapabilityEntry with fields model_name arch fp16 bf lG flash_attention Fields are &'static str for strings (no heap allocation) and bools for capability flags Copy Clone derived so they can be returned by value from lookup The Entry itself is accessed via static references not cloned
- Populate a const slice PCI_CAPABILITY_TABLE with seeded entries covering NVIDIA RTX 309O AIOOO HlOO V IOO AMD RX79XXT MI25O At least one entry per major vendor as required by spec Seed a few NVIDIA plus AMD ids requirement

### Out of Scope
- No modifications to GpuDevice or any type in anvilml-core/src/types/hardware.rs That belongs entirely to P3-B2 retrofit task The current struct with index name device_type total_vram_mib free vram m ib driver_version is used as-is by resolve_caps stub only for the fields available now which is just name
- No RON parsing no embedded file loading or any external data format Avoids adding ron crate dependency entirely per FORGE_AGENT_RULES section 43 Inline const slice satisfies "constslice" option in task spec clause

- No integration wiring into detect_all_devices function that lives in lib.rs and belongs to P4-A5 which depends on this task Only module registration line added here no logic changes beyond pubmod device db; declaration

## Approach

Step 1: Define types DeviceCapabilityEntry with model_name arch fp lG bf I G flash_attention fields all &'static str or bool Copy Clone Default derived for simplicity and zero-allocation access patterns via reference returned from lookup()
Step 2: Populate const PCI_CAPABILITY_TABLE containing seeded entries NVIDIA RTX309O AIOOO HlOO V IOO AMD RX79XXT MI25O with vendor id device_ id arch fp16 bf lG flash_attention model_name per ANVILML_DESIGN section 4.3 format conventions CUDA SM versions like X.Y or AMD gfx architecture identifiers matching pattern ^gfx\\d{4}$
Step 3: Implement lookup function pub fn look_up taking vendor_id u16 device id :u lG returning Option of static reference to DeviceCapabilityEntry via linear scan over PCI_CAPABILITY_TABLE using iter().find()
Step 4: Implement resolve_caps stub. Currently GpuDevice lacks arch caps enumeration_source and capabilities_source fields which are added by P3-B2 retrofit task Therefore resolvecaps can only set dev.name from entry.model_name on lookup hit since name is the only field available that device_db provides On miss emit log::warn! with vendor+device IDs for table extension tracking per spec requirement warn unknown PCI ID Add a TODO comment at top of function noting fields arch caps enumeration_source capabilities source will be filled once P3-B2 retrofits hardware.rs This keeps task strictly within scope while producing correct behaviour on available data now and future-proofing against downstream struct changes See Risks section for full detail
Step 5: Write unit tests in modtests{} block For each seed entry assert lookup returns Some with matching model_name arch fp1G bf l G flash_attention values exactly as specified Test miss case call lookup(0x8OBB OxFFFF) return None since Intel vendor ID is not seeded Duplicate-ID check iterate all pairs in table assert no two entries share same(vendor_id device_ id pair Arch format assertion for every non-empty arch string verify it matches either regex ^gfx\\d{4}$ or X.Y pattern Boolean flag consistency per generation Field count assertion ensure DeviceCapabilityEntry has exactly five fields via compile-time check
Step 6: Module registration in lib rs Add one line pubmoddevice_db after existing mod declarations (after mock feature-gated block) No other changes allowed per FORGE_AGENT_RULES section 42 Do not refactor code outside files listed This ensures device db is accessible to P4-A5 and any future consumers of the capability table
Step 7: Validation gate Run cargo test-p anvilml-hardware --device_db must exit O before considering task complete Also run cargo clippy on crate to ensure no lint warnings introduced by new module

## Files Affected

| Action | Path | Description | 
|--------|------|-------------| 


| Create | crates/anvilml-hardware/src/device_db.rs | New PCI-ID capability table module with DeviceCapabilityEntry struct const TABLE lookup() resolve_caps)) and modtests{} unit tests block all in one file per FORGE_AGENT_RULES section 54 Rust test naming convention Add line to existing: add pubmoddevice db; declaration after last pub mod block no other changes allowed to lib.rs by task scope constraints | 

## Tests

| Test File | What It Verifies | 
-----------+-----------------
crates/anvilml-hardware/src/device_db.rs::tests Lookup hits for all 6 seeded entries return correct model_name arch fp1G bf l G flash_attention values exactly as specified Miss returns None lookup(0x8OBB OxFFFF and lookup(OXDEAD OxBEEF both ret urn Non e since neither PCI ID pair is in table Duplicate-ID check iterates all pairs verifies no two entries share same(vendor_id device id) prevents silent overwrites or ambiguous lookups Arch format validation for non-empty arch strings verify they match expected patterns: CUDA X.Y or AMD gfx\\d{4} using simple string contains checks on substrings like .gfx and numeric digit count assertions Boolean flag consistency per architecture family Turing fp16=true bf lG=false Hopper+fp 1 G=tru ebf I Gtrue flash_attention true RDNA3 same as Hoppe r matches PyTorch capability matrix from ANVILML_DESIGN section 54 Field count assertion verify Entry struct has exactly five fields (model_name arch fp16 bf16 flash attention) and no VRAM-related field exists at all compile-time check via assert_eq!(std::mem size_of::<DeviceCapabilityEntry>() expected_size)" where expected computed from known field types Seed entry integrity test iterates table entries asserting model name is non-empty string length within reasonable bounds (4 to 128 characters) and arch either empty or valid pattern prevents accidental blank entries during manual maintenance updates

## CI Impact
No changes required to GitHub Actions workflow files (.github/workflows/*.yml). The existing CI matrix already runs cargo fmt --all check clippy -p anvil ml-hardware features mock hardware-D warnings and test-p anvi l-mhardware-featuresmock-hardware on both Linux ubuntu-latest and Windows windows-latest runners Since device_db has no platform-specific code or feature gating it compiles identically across all CI jobs automatically when P4-A5 wires resolve_caps into detect_all_devices No new workflow job step needed for this task alone per FORGE_AGENT_RULES section 37 Do not modify .github/workflows unless explicitly listed in Files Affected table

## Risks and Mitigations
| Risk | Mitigation | 
------+------------|  
GpuDevice lacks arch caps enumeration_source capabilities_source fields required by resolve_caps for full spec compliance per ANVILML_DESIGN section 5.4-section 5 3 — these types are added later in P3-B2 which retrofits hardware.rs with new struct members DeviceCapabilityEntry lookup() and const table data are fully self-contained do not depend on any downstream GpuDevice changes resolve_caps function is intentionally scoped to only mutate fields available now (name field) via stub implementation that fills name from entry.model_nameon hit Add a TODO comment at top of the function noting arch caps enumeration_source capabilities source will be filled once P3-B2 retrofits hardware.rs — this keeps task strictly within scope per FORGE_AGENT_RULES section 41 Do not implement functionality outside current tasks defined while producing correct behaviour on available data now and future-proofing against downstream struct changes | 
| Adding pub mod device db to lib rs might conflict with existing module declarations if P3-B2 has already added similar entries there (unlikely since modules are feature-gated) — minimal edit one line addition after last pubmod block no other changes allowed per FORGE_AGENT_RULES section 4 2 Do not refactor code outside files listed in Files Affected table | 
| Linear scan over const slice is O(n but table size stays small (<30 entries typical growth rate performance impact negligible — lookup called only once per device at startup during detect_all_devices() execution which runs ONCE when server starts up (per DESIGN section 8) not on hot request paths. No binary search or hash map needed for this use case since resolution happens infrequently | 
| RON crate not available in Cargo.lock file so embedded-RON approach rejected inline const slice is only viable option without adding dependencies — chosen upfront satisfies "constslice" clause from task spec avoids ron dependency entirely per FORGE_AGENT_RULES section 43 Do not upgrade deps unless explicitly required (task does NOT require RON) | 

## Acceptance Criteria


- [ ] crates/anvilml-hardware/src/device_db.rs exists and compiles independently: cargo check -p anvi mlhardware passes without errors or warnings 
- [ ] DeviceCapabilityEntry struct has exactly five fields model_name arch fp16 bf lG flash_attention — no VRAM field present (compile-time verified via test)
- [ ] PCI_CAPABILITY_TABLE is a const slice with at least 3 NVIDIA entries and 2 AMD entries seeded correctly: RTX309 AIOO HOO V IOO RX79XXT MI250 
- [ ] lookup(vendor_id u16 device id :u lG) -> Option<&static DeviceCapabilityEntry> returns correct Entry for all seede d IDs AND None for non-existent combinations (tested with 3+ miss cases)
- [ ] resolve_caps(dev&mut GpuDevice): sets dev.name to canonical model_name on lookup hit capabilities_source=DeviceTable; leaves fields unchanged and logs warn unknown PCI ID via log::warn!(vendor_id device id)" — arch/caps filled after P3-B2 retrofits hardware.rs 
| Unit tests in modtests{} block cover all seeded entries miss cases duplicate-ID check arch format validation boolean flag consistency per generation no VRAM field compile-time verification |
- [ ] cargo test -p anvilml-hardware -- device_db exits with code 0 (all unit tests pass)


END OF PLAN REPORT: P4-A4B
