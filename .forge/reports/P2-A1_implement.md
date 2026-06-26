# Implementation Report: P2-A1

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P2-A1                                       |
| Phase         | 2 — Core Domain Types: Config & Errors      |
| Description   | anvilml-core: AnvilError enum + IntoResponse impl |
| Implemented   | 2026-06-26T18:00:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Implemented the `AnvilError` enum with all 13 variants specified in `ANVILML_DESIGN.md §5.2` (including `ArtifactNotFound` per the addendum) in `crates/anvilml-core/src/error.rs`. Implemented `axum::response::IntoResponse` for `AnvilError` mapping each variant to the correct HTTP status code and structured JSON error body. Wired the module into `lib.rs` with `mod error;` and `pub use error::AnvilError;`. Added all required dependencies to `Cargo.toml`. Created 16 integration tests covering all 13 variants plus structural validation, all passing.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source           |
|--------|-----------|-----------------|-----------------|
| crate  | thiserror | 2.0.18           | Plan (confirmed: axum 0.8.9 workspace pin) |
| crate  | axum      | 0.8.9            | Plan (confirmed: workspace pin in anvilml-server/Cargo.toml, backend/Cargo.toml) |
| crate  | uuid      | 1.23.4           | Plan (added serde feature for ErrorBody serialization) |
| crate  | serde_json| 1.0              | Plan (semver-compatible range) |
| crate  | sqlx      | 0.9.0            | Plan (sqlite feature) |
| crate  | serde     | 1.0              | Added for ErrorBody Serialize derive (plan did not list but required by ErrorBody) |

Note: `serde` was added as a direct dependency because `ErrorBody` derives `Serialize` and uses `uuid::Uuid` (which requires the `serde` feature flag). This was not in the original plan but is required for compilation.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/error.rs` | `AnvilError` enum (13 variants) + `IntoResponse` impl + `ErrorBody` struct |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Added `mod error;` and `pub use error::AnvilError;` |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Added `thiserror`, `axum`, `uuid`, `serde_json`, `sqlx`, `serde` deps; `tokio` dev-dep; bumped version to 0.1.1 |
| CREATE | `crates/anvilml-core/tests/error_tests.rs` | 16 integration tests for variant-to-status mappings |
| MODIFY | `docs/TESTS.md` | Added 16 test catalogue entries for error_tests tests |

## Commit Log

```
 .../lib-futures_sink.json                          |    1 +
 .../dep-lib-futures_task                           |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../futures-task-5dad006b4bd23b12/lib-futures_task |    1 +
 .../lib-futures_task.json                          |    1 +
 .../dep-lib-futures_util                           |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../futures-util-d40b90145b0efd4b/lib-futures_util |    1 +
 .../lib-futures_util.json                          |    1 +
 .../run-build-script-build-script-build            |    1 +
 .../run-build-script-build-script-build.json       |    1 +
 .../dep-lib-generic_array                          |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../lib-generic_array                              |    1 +
 .../lib-generic_array.json                         |    1 +
 .../run-build-script-build-script-build            |    1 +
 .../run-build-script-build-script-build.json       |    1 +
 .../getrandom-eb7fda387c72051a/dep-lib-getrandom   |  Bin 0 -> 14 bytes
 .../getrandom-eb7fda387c72051a/invoked.timestamp   |    1 +
 .../getrandom-eb7fda387c72051a/lib-getrandom       |    1 +
 .../getrandom-eb7fda387c72051a/lib-getrandom.json  |    1 +
 .../hashbrown-1111c3b23d930e8b/dep-lib-hashbrown   |  Bin 0 -> 14 bytes
 .../hashbrown-1111c3b23d930e8b/invoked.timestamp   |    1 +
 .../hashbrown-1111c3b23d930e8b/lib-hashbrown       |    1 +
 .../hashbrown-1111c3b23d930e8b/lib-hashbrown.json  |    1 +
 .../hashbrown-4b308be5599da50a/dep-lib-hashbrown   |  Bin 0 -> 14 bytes
 .../hashbrown-4b308be5599da50a/invoked.timestamp   |    1 +
 .../hashbrown-4b308be5599da50a/lib-hashbrown       |    1 +
 .../hashbrown-4b308be5599da50a/lib-hashbrown.json  |    1 +
 .../hashlink-f35e73614c0deb13/dep-lib-hashlink     |  Bin 0 -> 14 bytes
 .../hashlink-f35e73614c0deb13/invoked.timestamp    |    1 +
 .../hashlink-f35e73614c0deb13/lib-hashlink         |    1 +
 .../hashlink-f35e73614c0deb13/lib-hashlink.json    |    1 +
 .../http-1247c2b6960eab76/dep-lib-http             |  Bin 0 -> 14 bytes
 .../http-1247c2b6960eab76/invoked.timestamp        |    1 +
 .../.fingerprint/http-1247c2b6960eab76/lib-http    |    1 +
 .../http-1247c2b6960eab76/lib-http.json            |    1 +
 .../http-body-91c5d3e3cf12f69d/dep-lib-http_body   |  Bin 0 -> 14 bytes
 .../http-body-91c5d3e3cf12f69d/invoked.timestamp   |    1 +
 .../http-body-91c5d3e3cf12f69d/lib-http_body       |    1 +
 .../http-body-91c5d3e3cf12f69d/lib-http_body.json  |    1 +
 .../dep-lib-http_body_util                         |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../lib-http_body_util                             |    1 +
 .../lib-http_body_util.json                        |    1 +
 .../httparse-ee7f8d9422e33d40/dep-lib-httparse     |  Bin 0 -> 14 bytes
 .../httparse-ee7f8d9422e33d40/invoked.timestamp    |    1 +
 .../httparse-ee7f8d9422e33d40/lib-httparse         |    1 +
 .../httparse-ee7f8d9422e33d40/lib-httparse.json    |    1 +
 .../run-build-script-build-script-build            |    1 +
 .../run-build-script-build-script-build.json       |    1 +
 .../httpdate-6817f98c5ffb8b45/dep-lib-httpdate     |  Bin 0 -> 14 bytes
 .../httpdate-6817f98c5ffb8b45/invoked.timestamp    |    1 +
 .../httpdate-6817f98c5ffb8b45/lib-httpdate         |    1 +
 .../httpdate-6817f98c5ffb8b45/lib-httpdate.json    |    1 +
 .../hyper-b379a1e497cb52a2/dep-lib-hyper           |  Bin 0 -> 14 bytes
 .../hyper-b379a1e497cb52a2/invoked.timestamp       |    1 +
 .../.fingerprint/hyper-b379a1e497cb52a2/lib-hyper  |    1 +
 .../hyper-b379a1e497cb52a2/lib-hyper.json          |    1 +
 .../hyper-util-04bfbecb96584ae1/dep-lib-hyper_util |  Bin 0 -> 14 bytes
 .../hyper-util-04bfbecb96584ae1/invoked.timestamp  |    1 +
 .../hyper-util-04bfbecb96584ae1/lib-hyper_util     |    1 +
 .../lib-hyper_util.json                            |    1 +
 .../dep-lib-icu_collections                        |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../lib-icu_collections                            |    1 +
 .../lib-icu_collections.json                       |    1 +
 .../dep-lib-icu_locale_core                        |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../lib-icu_locale_core                            |    1 +
 .../lib-icu_locale_core.json                       |    1 +
 .../dep-lib-icu_normalizer                         |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../lib-icu_normalizer                             |    1 +
 .../lib-icu_normalizer.json                        |    1 +
 .../run-build-script-build-script-build            |    1 +
 .../run-build-script-build-script-build.json       |    1 +
 .../dep-lib-icu_normalizer_data                    |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../lib-icu_normalizer_data                        |    1 +
 .../lib-icu_normalizer_data.json                   |    1 +
 .../dep-lib-icu_properties                         |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../lib-icu_properties                             |    1 +
 .../lib-icu_properties.json                        |    1 +
 .../run-build-script-build-script-build            |    1 +
 .../run-build-script-build-script-build.json       |    1 +
 .../dep-lib-icu_properties_data                    |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../lib-icu_properties_data                        |    1 +
 .../lib-icu_properties_data.json                   |    1 +
 .../dep-lib-icu_provider                           |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../icu_provider-7f05da8e60683a15/lib-icu_provider |    1 +
 .../lib-icu_provider.json                          |    1 +
 .../idna-8f829d12fa4f87e0/dep-lib-idna             |  Bin 0 -> 14 bytes
 .../idna-8f829d12fa4f87e0/invoked.timestamp        |    1 +
 .../.fingerprint/idna-8f829d12fa4f87e0/lib-idna    |    1 +
 .../idna-8f829d12fa4f87e0/lib-idna.json            |    1 +
 .../dep-lib-idna_adapter                           |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../idna_adapter-88769aa14e94bba1/lib-idna_adapter |    1 +
 .../lib-idna_adapter.json                          |    1 +
 .../indexmap-5f74cfa3b09b595c/dep-lib-indexmap     |  Bin 0 -> 14 bytes
 .../indexmap-5f74cfa3b09b595c/invoked.timestamp    |    1 +
 .../indexmap-5f74cfa3b09b595c/lib-indexmap         |    1 +
 .../indexmap-5f74cfa3b09b595c/lib-indexmap.json    |    1 +
 .../dep-lib-is_terminal_polyfill                   |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../lib-is_terminal_polyfill                       |    1 +
 .../lib-is_terminal_polyfill.json                  |    1 +
 .../itoa-33db2b7cec95af6d/dep-lib-itoa             |  Bin 0 -> 14 bytes
 .../itoa-33db2b7cec95af6d/invoked.timestamp        |    1 +
 .../.fingerprint/itoa-33db2b7cec95af6d/lib-itoa    |    1 +
 .../itoa-33db2b7cec95af6d/lib-itoa.json            |    1 +
 .../run-build-script-build-script-build            |    1 +
 .../run-build-script-build-script-build.json       |    1 +
 .../dep-lib-libsqlite3_sys                         |  Bin 0 -> 100 bytes
 .../invoked.timestamp                              |    1 +
 .../lib-libsqlite3_sys                             |    1 +
 .../lib-libsqlite3_sys.json                        |    1 +
 .../litemap-b67fb40615c80f2c/dep-lib-litemap       |  Bin 0 -> 14 bytes
 .../litemap-b67fb40615c80f2c/invoked.timestamp     |    1 +
 .../litemap-b67fb40615c80f2c/lib-litemap           |    1 +
 .../litemap-b67fb40615c80f2c/lib-litemap.json      |    1 +
 .../lock_api-5848eda82ccf8c09/dep-lib-lock_api     |  Bin 0 -> 14 bytes
 .../lock_api-5848eda82ccf8c09/invoked.timestamp    |    1 +
 .../lock_api-5848eda82ccf8c09/lib-lock_api         |    1 +
 .../lock_api-5848eda82ccf8c09/lib-lock_api.json    |    1 +
 .../.fingerprint/log-02816c9ddfb714e5/dep-lib-log  |  Bin 0 -> 14 bytes
 .../log-02816c9ddfb714e5/invoked.timestamp         |    1 +
 .../.fingerprint/log-02816c9ddfb714e5/lib-log      |    1 +
 .../.fingerprint/log-02816c9ddfb714e5/lib-log.json |    1 +
 .../matchit-ffc0aa833c3d15d0/dep-lib-matchit       |  Bin 0 -> 14 bytes
 .../matchit-ffc0aa833c3d15d0/invoked.timestamp     |    1 +
 .../matchit-ffc0aa833c3d15d0/lib-matchit           |    1 +
 .../matchit-ffc0aa833c3d15d0/lib-matchit.json      |    1 +
 .../memchr-18fea648d20f8e04/dep-lib-memchr         |  Bin 0 -> 14 bytes
 .../memchr-18fea648d20f8e04/invoked.timestamp      |    1 +
 .../memchr-18fea648d20f8e04/lib-memchr             |    1 +
 .../memchr-18fea648d20f8e04/lib-memchr.json        |    1 +
 .../mime-8a5f3e026c075afc/dep-lib-mime             |  Bin 0 -> 14 bytes
 .../mime-8a5f3e026c075afc/invoked.timestamp        |    1 +
 .../.fingerprint/mime-8a5f3e026c075afc/lib-mime    |    1 +
 .../mime-8a5f3e026c075afc/lib-mime.json            |    1 +
 .../.fingerprint/mio-c5ed47f2512781f4/dep-lib-mio  |  Bin 0 -> 14 bytes
 .../mio-c5ed47f2512781f4/invoked.timestamp         |    1 +
 .../.fingerprint/mio-c5ed47f2512781f4/lib-mio      |    1 +
 .../.fingerprint/mio-c5ed47f2512781f4/lib-mio.json |    1 +
 .../run-build-script-build-script-build            |    1 +
 .../run-build-script-build-script-build.json       |    1 +
 .../num-traits-b1e55a3a951e567a/dep-lib-num_traits |  Bin 0 -> 14 bytes
 .../num-traits-b1e55a3a951e567a/invoked.timestamp  |    1 +
 .../num-traits-b1e55a3a951e567a/lib-num_traits     |    1 +
 .../lib-num_traits.json                            |    1 +
 .../once_cell-2779b49c3849fca0/dep-lib-once_cell   |  Bin 0 -> 14 bytes
 .../once_cell-2779b49c3849fca0/invoked.timestamp   |    1 +
 .../once_cell-2779b49c3849fca0/lib-once_cell       |    1 +
 .../once_cell-2779b49c3849fca0/lib-once_cell.json  |    1 +
 .../dep-lib-once_cell_polyfill                     |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../lib-once_cell_polyfill                         |    1 +
 .../lib-once_cell_polyfill.json                    |    1 +
 .../parking-0b7b4040bb7a428b/dep-lib-parking       |  Bin 0 -> 14 bytes
 .../parking-0b7b4040bb7a428b/invoked.timestamp     |    1 +
 .../parking-0b7b4040bb7a428b/lib-parking           |    1 +
 .../parking-0b7b4040bb7a428b/lib-parking.json      |    1 +
 .../dep-lib-parking_lot                            |  Bin 0 -> 14 bytes
 .../parking_lot-a439919e56a6115d/invoked.timestamp |    1 +
 .../parking_lot-a439919e56a6115d/lib-parking_lot   |    1 +
 .../lib-parking_lot.json                           |    1 +
 .../dep-lib-parking_lot_core                       |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../lib-parking_lot_core                           |    1 +
 .../lib-parking_lot_core.json                      |    1 +
 .../run-build-script-build-script-build            |    1 +
 .../run-build-script-build-script-build.json       |    1 +
 .../dep-lib-percent_encoding                       |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../lib-percent_encoding                           |    1 +
 .../lib-percent_encoding.json                      |    1 +
 .../dep-lib-pin_project_lite                       |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../lib-pin_project_lite                           |    1 +
 .../lib-pin_project_lite.json                      |    1 +
 .../dep-lib-potential_utf                          |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../lib-potential_utf                              |    1 +
 .../lib-potential_utf.json                         |    1 +
 .../.fingerprint/ryu-3b2e873a4fe9dfe4/dep-lib-ryu  |  Bin 0 -> 14 bytes
 .../ryu-3b2e873a4fe9dfe4/invoked.timestamp         |    1 +
 .../.fingerprint/ryu-3b2e873a4fe9dfe4/lib-ryu      |    1 +
 .../.fingerprint/ryu-3b2e873a4fe9dfe4/lib-ryu.json |    1 +
 .../scopeguard-fef4712d3f25d2a7/dep-lib-scopeguard |  Bin 0 -> 14 bytes
 .../scopeguard-fef4712d3f25d2a7/invoked.timestamp  |    1 +
 .../scopeguard-fef4712d3f25d2a7/lib-scopeguard     |    1 +
 .../lib-scopeguard.json                            |    1 +
 .../run-build-script-build-script-build            |    1 +
 .../run-build-script-build-script-build.json       |    1 +
 .../serde-af877e32508284a7/dep-lib-serde           |  Bin 0 -> 91 bytes
 .../serde-af877e32508284a7/invoked.timestamp       |    1 +
 .../.fingerprint/serde-af877e32508284a7/lib-serde  |    1 +
 .../serde-af877e32508284a7/lib-serde.json          |    1 +
 .../run-build-script-build-script-build            |    1 +
 .../run-build-script-build-script-build.json       |    1 +
 .../serde_core-d291707330e45a52/dep-lib-serde_core |  Bin 0 -> 96 bytes
 .../serde_core-d291707330e45a52/invoked.timestamp  |    1 +
 .../serde_core-d291707330e45a52/lib-serde_core     |    1 +
 .../lib-serde_core.json                            |    1 +
 .../run-build-script-build-script-build            |    1 +
 .../run-build-script-build-script-build.json       |    1 +
 .../serde_json-5d230c5f1d448a3d/dep-lib-serde_json |  Bin 0 -> 14 bytes
 .../serde_json-5d230c5f1d448a3d/invoked.timestamp  |    1 +
 .../serde_json-5d230c5f1d448a3d/lib-serde_json     |    1 +
 .../lib-serde_json.json                            |    1 +
 .../dep-lib-serde_path_to_error                    |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../lib-serde_path_to_error                        |    1 +
 .../lib-serde_path_to_error.json                   |    1 +
 .../dep-lib-serde_urlencoded                       |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../lib-serde_urlencoded                           |    1 +
 .../lib-serde_urlencoded.json                      |    1 +
 .../sha2-74baf0e8b497907d/dep-lib-sha2             |  Bin 0 -> 14 bytes
 .../sha2-74baf0e8b497907d/invoked.timestamp        |    1 +
 .../.fingerprint/sha2-74baf0e8b497907d/lib-sha2    |    1 +
 .../sha2-74baf0e8b497907d/lib-sha2.json            |    1 +
 .../slab-e64c3e7af05aca81/dep-lib-slab             |  Bin 0 -> 14 bytes
 .../slab-e64c3e7af05aca81/invoked.timestamp        |    1 +
 .../.fingerprint/slab-e64c3e7af05aca81/lib-slab    |    1 +
 .../slab-e64c3e7af05aca81/lib-slab.json            |    1 +
 .../smallvec-fa499917f6cfe4f3/dep-lib-smallvec     |  Bin 0 -> 14 bytes
 .../smallvec-fa499917f6cfe4f3/invoked.timestamp    |    1 +
 .../smallvec-fa499917f6cfe4f3/lib-smallvec         |    1 +
 .../smallvec-fa499917f6cfe4f3/lib-smallvec.json    |    1 +
 .../socket2-abcba000c21f9343/dep-lib-socket2       |  Bin 0 -> 14 bytes
 .../socket2-abcba000c21f9343/invoked.timestamp     |    1 +
 .../socket2-abcba000c21f9343/lib-socket2           |    1 +
 .../socket2-abcba000c21f9343/lib-socket2.json      |    1 +
 .../spin-5270311b953784d8/dep-lib-spin             |  Bin 0 -> 14 bytes
 .../spin-5270311b953784d8/invoked.timestamp        |    1 +
 .../.fingerprint/spin-5270311b953784d8/lib-spin    |    1 +
 .../spin-5270311b953784d8/lib-spin.json            |    1 +
 .../sqlx-core-31dac1e98296e2ce/dep-lib-sqlx_core   |  Bin 0 -> 14 bytes
 .../sqlx-core-31dac1e98296e2ce/invoked.timestamp   |    1 +
 .../sqlx-core-31dac1e98296e2ce/lib-sqlx_core       |    1 +
 .../sqlx-core-31dac1e98296e2ce/lib-sqlx_core.json  |    1 +
 .../sqlx-f8e24a55c3a97c65/dep-lib-sqlx             |  Bin 0 -> 14 bytes
 .../sqlx-f8e24a55c3a97c65/invoked.timestamp        |    1 +
 .../.fingerprint/sqlx-f8e24a55c3a97c65/lib-sqlx    |    1 +
 .../sqlx-f8e24a55c3a97c65/lib-sqlx.json            |    1 +
 .../dep-lib-sqlx_sqlite                            |  Bin 0 -> 14 bytes
 .../sqlx-sqlite-6fb81d30905e7d95/invoked.timestamp |    1 +
 .../sqlx-sqlite-6fb81d30905e7d95/lib-sqlx_sqlite   |    1 +
 .../lib-sqlx_sqlite.json                           |    1 +
 .../dep-lib-stable_deref_trait                     |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../lib-stable_deref_trait                         |    1 +
 .../lib-stable_deref_trait.json                    |    1 +
 .../strsim-e51399f6a3e60186/dep-lib-strsim         |  Bin 0 -> 14 bytes
 .../strsim-e51399f6a3e60186/invoked.timestamp      |    1 +
 .../strsim-e51399f6a3e60186/lib-strsim             |    1 +
 .../strsim-e51399f6a3e60186/lib-strsim.json        |    1 +
 .../dep-lib-sync_wrapper                           |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../sync_wrapper-f34a40796c862f5f/lib-sync_wrapper |    1 +
 .../lib-sync_wrapper.json                          |    1 +
 .../run-build-script-build-script-build            |    1 +
 .../run-build-script-build-script-build.json       |    1 +
 .../thiserror-ba02c471537cd1de/dep-lib-thiserror   |  Bin 0 -> 95 bytes
 .../thiserror-ba02c471537cd1de/invoked.timestamp   |    1 +
 .../thiserror-ba02c471537cd1de/lib-thiserror       |    1 +
 .../thiserror-ba02c471537cd1de/lib-thiserror.json  |    1 +
 .../tinystr-ad7d502c4bafe7a1/dep-lib-tinystr       |  Bin 0 -> 14 bytes
 .../tinystr-ad7d502c4bafe7a1/invoked.timestamp     |    1 +
 .../tinystr-ad7d502c4bafe7a1/lib-tinystr           |    1 +
 .../tinystr-ad7d502c4bafe7a1/lib-tinystr.json      |    1 +
 .../tokio-e07d07b8ea59b346/dep-lib-tokio           |  Bin 0 -> 14 bytes
 .../tokio-e07d07b8ea59b346/invoked.timestamp       |    1 +
 .../.fingerprint/tokio-e07d07b8ea59b346/lib-tokio  |    1 +
 .../tokio-e07d07b8ea59b346/lib-tokio.json          |    1 +
 .../tower-95f8dc8466943d7b/dep-lib-tower           |  Bin 0 -> 14 bytes
 .../tower-95f8dc8466943d7b/invoked.timestamp       |    1 +
 .../.fingerprint/tower-95f8dc8466943d7b/lib-tower  |    1 +
 .../tower-95f8dc8466943d7b/lib-tower.json          |    1 +
 .../dep-lib-tower_layer                            |  Bin 0 -> 14 bytes
 .../tower-layer-92f2304324034f26/invoked.timestamp |    1 +
 .../tower-layer-92f2304324034f26/lib-tower_layer   |    1 +
 .../lib-tower_layer.json                           |    1 +
 .../dep-lib-tower_service                          |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../lib-tower_service                              |    1 +
 .../lib-tower_service.json                         |    1 +
 .../dep-lib-tracing_core                           |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../tracing-core-2858a7200499f1b5/lib-tracing_core |    1 +
 .../lib-tracing_core.json                          |    1 +
 .../tracing-f553fdb7c78d8121/dep-lib-tracing       |  Bin 0 -> 14 bytes
 .../tracing-f553fdb7c78d8121/invoked.timestamp     |    1 +
 .../tracing-f553fdb7c78d8121/lib-tracing           |    1 +
 .../tracing-f553fdb7c78d8121/lib-tracing.json      |    1 +
 .../typenum-60fc9221c6d2e470/dep-lib-typenum       |  Bin 0 -> 14 bytes
 .../typenum-60fc9221c6d2e470/invoked.timestamp     |    1 +
 .../typenum-60fc9221c6d2e470/lib-typenum           |    1 +
 .../typenum-60fc9221c6d2e470/lib-typenum.json      |    1 +
 .../.fingerprint/url-94d1069f492dc93b/dep-lib-url  |  Bin 0 -> 14 bytes
 .../url-94d1069f492dc93b/invoked.timestamp         |    1 +
 .../.fingerprint/url-94d1069f492dc93b/lib-url      |    1 +
 .../.fingerprint/url-94d1069f492dc93b/lib-url.json |    1 +
 .../utf8_iter-92865f1d0fb90837/dep-lib-utf8_iter   |  Bin 0 -> 14 bytes
 .../utf8_iter-92865f1d0fb90837/invoked.timestamp   |    1 +
 .../utf8_iter-92865f1d0fb90837/lib-utf8_iter       |    1 +
 .../utf8_iter-92865f1d0fb90837/lib-utf8_iter.json  |    1 +
 .../utf8parse-5445efc84cc000d7/dep-lib-utf8parse   |  Bin 0 -> 14 bytes
 .../utf8parse-5445efc84cc000d7/invoked.timestamp   |    1 +
 .../utf8parse-5445efc84cc000d7/lib-utf8parse       |    1 +
 .../utf8parse-5445efc84cc000d7/lib-utf8parse.json  |    1 +
 .../uuid-3e964b0f7d79f283/dep-lib-uuid             |  Bin 0 -> 14 bytes
 .../uuid-3e964b0f7d79f283/invoked.timestamp        |    1 +
 .../.fingerprint/uuid-3e964b0f7d79f283/lib-uuid    |    1 +
 .../uuid-3e964b0f7d79f283/lib-uuid.json            |    1 +
 .../dep-lib-windows_link                           |  Bin 0 -> 14 bytes
 .../invoked.timestamp                              |    1 +
 .../windows-link-553d59391106fe5c/lib-windows_link |    1 +
 .../lib-windows_link.json                          |    1 +
 .../dep-lib-windows_sys                            |  Bin 0 -> 14 bytes
 .../windows-sys-abd82277c412b9e1/invoked.timestamp |    1 +
 .../windows-sys-abd82277c412b9e1/lib-windows_sys   |    1 +
 .../lib-windows_sys.json                           |    1 +
 .../writeable-bbb689a21439f365/dep-lib-writeable   |  Bin 0 -> 14 bytes
 .../writeable-bbb689a21439f365/invoked.timestamp   |    1 +
 .../writeable-bbb689a21439f365/lib-writeable       |    1 +
 .../writeable-bbb689a21439f365/lib-writeable.json  |    1 +
 .../yoke-e85007fa4509aa79/dep-lib-yoke             |  Bin 0 -> 14 bytes
 .../yoke-e85007fa4509aa79/invoked.timestamp        |    1 +
 .../.fingerprint/yoke-e85007fa4509aa79/lib-yoke    |    1 +
 .../yoke-e85007fa4509aa79/lib-yoke.json            |    1 +
 .../zerofrom-8e9bb4425404b034/dep-lib-zerofrom     |  Bin 0 -> 14 bytes
 .../zerofrom-8e9bb4425404b034/invoked.timestamp    |    1 +
 .../zerofrom-8e9bb4425404b034/lib-zerofrom         |    1 +
 .../zerofrom-8e9bb4425404b034/lib-zerofrom.json    |    1 +
 .../zerotrie-185b5d5760b4f64b/dep-lib-zerotrie     |  Bin 0 -> 14 bytes
 .../zerotrie-185b5d5760b4f64b/invoked.timestamp    |    1 +
 .../zerotrie-185b5d5760b4f64b/lib-zerotrie         |    1 +
 .../zerotrie-185b5d5760b4f64b/lib-zerotrie.json    |    1 +
 .../zerovec-91b199928eaf7f90/dep-lib-zerovec       |  Bin 0 -> 14 bytes
 .../zerovec-91b199928eaf7f90/invoked.timestamp     |    1 +
 .../zerovec-91b199928eaf7f90/lib-zerovec           |    1 +
 .../zerovec-91b199928eaf7f90/lib-zerovec.json      |    1 +
 .../zmij-0f2ba072a948bcd3/dep-lib-zmij             |  Bin 0 -> 14 bytes
 .../zmij-0f2ba072a948bcd3/invoked.timestamp        |    1 +
 .../.fingerprint/zmij-0f2ba072a948bcd3/lib-zmij    |    1 +
 .../zmij-0f2ba072a948bcd3/lib-zmij.json            |    1 +
 .../run-build-script-build-script-build            |    1 +
 .../run-build-script-build-script-build.json       |    1 +
 .../invoked.timestamp                              |    1 +
 .../build/crossbeam-utils-a0621c141bbc5f66/output  |    2 +
 .../crossbeam-utils-a0621c141bbc5f66/root-output   |    1 +
 .../build/crossbeam-utils-a0621c141bbc5f66/stderr  |    0
 .../invoked.timestamp                              |    1 +
 .../build/generic-array-6b3d56a29be17e4e/output    |    4 +
 .../generic-array-6b3d56a29be17e4e/root-output     |    1 +
 .../build/generic-array-6b3d56a29be17e4e/stderr    |    0
 .../getrandom-575343c79871d3f2/invoked.timestamp   |    1 +
 .../debug/build/getrandom-575343c79871d3f2/output  |    1 +
 .../build/getrandom-575343c79871d3f2/root-output   |    1 +
 .../debug/build/getrandom-575343c79871d3f2/stderr  |    0
 .../httparse-f1a854733f92305f/invoked.timestamp    |    1 +
 .../debug/build/httparse-f1a854733f92305f/output   |    2 +
 .../build/httparse-f1a854733f92305f/root-output    |    1 +
 .../debug/build/httparse-f1a854733f92305f/stderr   |    0
 .../invoked.timestamp                              |    1 +
 .../icu_normalizer_data-e0a0e7959b8e2445/output    |    2 +
 .../root-output                                    |    1 +
 .../icu_normalizer_data-e0a0e7959b8e2445/stderr    |    0
 .../invoked.timestamp                              |    1 +
 .../icu_properties_data-b94a4fe16f310577/output    |    2 +
 .../root-output                                    |    1 +
 .../icu_properties_data-b94a4fe16f310577/stderr    |    0
 .../invoked.timestamp                              |    1 +
 .../libsqlite3-sys-4439b1e75d16ca67/out/bindgen.rs | 3589 ++++++++++++++++++++
 .../out/c877a2978823c39d-sqlite3.o                 |  Bin 0 -> 6409374 bytes
 .../out/libsqlite3.a                               |  Bin 0 -> 6417086 bytes
 .../build/libsqlite3-sys-4439b1e75d16ca67/output   |   70 +
 .../libsqlite3-sys-4439b1e75d16ca67/root-output    |    1 +
 .../build/libsqlite3-sys-4439b1e75d16ca67/stderr   |    0
 .../num-traits-34275f92cc531ddd/invoked.timestamp  |    1 +
 .../debug/build/num-traits-34275f92cc531ddd/output |    3 +
 .../build/num-traits-34275f92cc531ddd/root-output  |    1 +
 .../debug/build/num-traits-34275f92cc531ddd/stderr |    0
 .../invoked.timestamp                              |    1 +
 .../build/parking_lot_core-b17c8da2a74f066f/output |    2 +
 .../parking_lot_core-b17c8da2a74f066f/root-output  |    1 +
 .../build/parking_lot_core-b17c8da2a74f066f/stderr |    0
 .../build/serde-2cd1c8426a4c81cb/invoked.timestamp |    1 +
 .../debug/build/serde-2cd1c8426a4c81cb/out/private.rs    |    6 +
 .../debug/build/serde-2cd1c8426a4c81cb/output      |   13 +
 .../debug/build/serde-2cd1c8426a4c81cb/root-output |    1 +
 .../debug/build/serde-2cd1c8426a4c81cb/stderr      |    0
 .../serde_core-35db9949fe952f21/invoked.timestamp  |    1 +
 .../serde_core-35db9949fe952f21/out/private.rs     |    5 +
 .../debug/build/serde_core-35db9949fe952f21/output |   11 +
 .../build/serde_core-35db9949fe952f21/root-output  |    1 +
 .../debug/build/serde_core-35db9949fe952f21/stderr |    0
 .../serde_json-3b4a80b1c3751e98/invoked.timestamp  |    1 +
 .../debug/build/serde_json-3b4a80b1c3751e98/output |    3 +
 .../build/serde_json-3b4a80b1c3751e98/root-output  |    1 +
 .../debug/build/serde_json-3b4a80b1c3751e98/stderr |    0
 .../thiserror-b0026d311a3bc5cc/invoked.timestamp   |    1 +
 .../debug/build/thiserror-b0026d311a3bc5cc/out/private.rs      |    5 +
 .../debug/build/thiserror-b0026d311a3bc5cc/output  |    5 +
 .../build/thiserror-b0026d311a3bc5cc/root-output   |    1 +
 .../debug/build/thiserror-b0026d311a3bc5cc/stderr  |    0
 .../build/zmij-1e5389112e79a1a9/invoked.timestamp  |    1 +
 .../debug/build/zmij-1e5389112e79a1a9/output       |    3 +
 .../debug/build/zmij-1e5389112e79a1a9/root-output  |    1 +
 .../debug/build/zmij-1e5389112e79a1a9/stderr       |    0
 .../debug/deps/allocator_api2-c005594d546f336b.d   |   19 +
 .../debug/deps/anstream-79e1797a501ae1ee.d         |   15 +
 .../debug/deps/anstyle-ef8989be75615ea2.d          |   10 +
 .../debug/deps/anstyle_parse-89ed28aeaa0c5c8e.d    |    9 +
 .../debug/deps/anstyle_query-468f1bad9a3373a4.d    |    6 +
 .../debug/deps/anstyle_wincon-093eeaf4fde10ca9.d   |    8 +
 .../debug/deps/anvilml-b9974bf43b5c3c6e.d          |    6 +
 .../debug/deps/anvilml-bef43214308623c1.d          |    6 +
 .../debug/deps/anvilml-c2fceda839b47553.d          |    6 +
 .../debug/deps/anvilml-e8e81f0dcfc148ed.d          |    6 +
 .../deps/anvilml_artifacts-c5f438a60fc7590d.d      |    5 +
 .../debug/deps/anvilml_core-280b4a8b3cdee509.d     |    6 +
 .../debug/deps/anvilml_hardware-083fd03b9bef5d5c.d |    5 +
 .../debug/deps/anvilml_hardware-4868f6b7f9423884.d |    5 +
 .../debug/deps/anvilml_ipc-0790952ee62642bf.d      |    5 +
 .../debug/deps/anvilml_openapi-7173169da8cafe60.d  |    5 +
 .../debug/deps/anvilml_registry-e2776a04f801ad70.d |    5 +
 .../deps/anvilml_scheduler-35cec301528d98df.d      |    5 +
 .../deps/anvilml_scheduler-b11884f833d5e19d.d      |    5 +
 .../debug/deps/anvilml_server-0fb13a07f5000556.d   |    7 +
 .../debug/deps/anvilml_server-2844092ce7abd77d.d   |    7 +
 .../debug/deps/anvilml_worker-67a5fd84c7f48c28.d   |    5 +
 .../debug/deps/anvilml_worker-d2a5c69aeb91332b.d   |    5 +
 .../debug/deps/atoi-78e2ba7c31d7ceba.d             |    5 +
 .../debug/deps/atomic_waker-f4e43e4522d78c56.d     |    5 +
 .../debug/deps/axum-3463f72e7c5406dd.d             |   72 +
 .../debug/deps/axum_core-90230d4de87a524e.d        |   22 +
 .../debug/deps/base64-bedf05073541d7a2.d           |   15 +
 .../debug/deps/block_buffer-2f25de0d23b3df40.d     |    6 +
 .../debug/deps/bytes-d7b8d484cb31b5de.d            |   22 +
 .../debug/deps/cfg_if-82f15ab1f86a6ef5.d           |    5 +
 .../debug/deps/clap-fab9f85fc05fa2dc.d             |    7 +
 .../debug/deps/clap_builder-f0ed1f4bb94ccd73.d     |   59 +
 .../debug/deps/clap_lex-bbdf52239761cfc0.d         |    6 +
 .../debug/deps/colorchoice-2b5d3b391305ba40.d      |    5 +
 .../debug/deps/concurrent_queue-2347621bc9e1af66.d |    9 +
 .../debug/deps/cpufeatures-a2f96a33d4923769.d      |    6 +
 .../debug/deps/crc-c839f323a0300d80.d              |   12 +
 .../debug/deps/crc_catalog-5782bd9f84f2a313.d      |    7 +
 .../debug/deps/crossbeam_queue-5de67a7164c4494b.d  |    7 +
 .../debug/deps/crossbeam_utils-e9077ada970e3223.d  |   17 +
 .../debug/deps/crypto_common-4de70b43f9ab8ac5.d    |    5 +
 .../debug/deps/digest-a9bbe63123c7c09d.d           |   11 +
 .../debug/deps/either-cfb383dc9146c715.d           |    9 +
 .../debug/deps/equivalent-24ddf9bd662ef2d0.d       |    5 +
 .../debug/deps/event_listener-641594697b721f4d.d   |    7 +
 .../debug/deps/flume-c32c895db72b7f37.d            |    7 +
 .../debug/deps/foldhash-f789545bb71bddcb.d         |    8 +
 .../debug/deps/form_urlencoded-dea1f4fc7d31c34b.d  |    5 +
 .../debug/deps/futures_channel-d0e168c418386317.d  |   10 +
 .../debug/deps/futures_core-ca4c3b14fe7fad58.d     |   11 +
 .../debug/deps/futures_executor-dee0175459769ed6.d |    7 +
 .../deps/futures_intrusive-9683da98bd4ada24.d      |   26 +
 .../debug/deps/futures_io-090c98f3b411d1ca.d       |    5 +
 .../debug/deps/futures_sink-df578d2515b56090.d     |    5 +
 .../debug/deps/futures_task-5dad006b4bd23b12.d     |   11 +
 .../debug/deps/futures_util-d40b90145b0efd4b.d     |  173 +
 .../debug/deps/generic_array-74f4bc2011284f94.d    |   11 +
 .../debug/deps/getrandom-eb7fda387c72051a.d        |   10 +
 .../debug/deps/hashbrown-1111c3b23d930e8b.d        |   20 +
 .../debug/deps/hashbrown-4b308be5599da50a.d        |   21 +
 .../debug/deps/hashlink-f35e73614c0deb13.d         |    8 +
 .../debug/deps/http-1247c2b6960eab76.d             |   24 +
 .../debug/deps/http_body-91c5d3e3cf12f69d.d        |    7 +
 .../debug/deps/http_body_util-e847d18421b494d2.d   |   19 +
 .../debug/deps/httparse-ee7f8d9422e33d40.d         |   12 +
 .../debug/deps/httpdate-6817f98c5ffb8b45.d         |    6 +
 .../debug/deps/hyper-b379a1e497cb52a2.d            |   44 +
 .../debug/deps/hyper_util-04bfbecb96584ae1.d       |   21 +
 .../debug/deps/icu_collections-30252bf0cf20f494.d  |   17 +
 .../debug/deps/icu_locale_core-fc7a7efdd8297031.d  |   64 +
 .../debug/deps/icu_normalizer-f684dddf5f8ea9bb.d   |    8 +
 .../deps/icu_normalizer_data-fbeb9f08442df82a.d    |   13 +
 .../debug/deps/icu_properties-5716706615c6f530.d   |   16 +
 .../deps/icu_properties_data-c46881417b72739b.d    |  143 +
 .../debug/deps/icu_provider-7f05da8e60683a15.d     |   17 +
 .../debug/deps/idna-8f829d12fa4f87e0.d             |    8 +
 .../debug/deps/idna_adapter-88769aa14e94bba1.d     |    5 +
 .../debug/deps/indexmap-5f74cfa3b09b595c.d         |   21 +
 .../deps/is_terminal_polyfill-4f9a4e49de449032.d   |    5 +
 .../debug/deps/itoa-33db2b7cec95af6d.d             |    6 +
 .../deps/liballocator_api2-c005594d546f336b.rmeta  |  Bin 0 -> 475864 bytes
 .../debug/deps/libanstream-79e1797a501ae1ee.rmeta  |  Bin 0 -> 176944 bytes
 .../debug/deps/libanstyle-ef8989be75615ea2.rmeta   |  Bin 0 -> 132748 bytes
 .../deps/libanstyle_parse-89ed28aeaa0c5c8e.rmeta   |  Bin 0 -> 134926 bytes
 .../deps/libanstyle_query-468f1bad9a3373a4.rmeta   |  Bin 0 -> 9048 bytes
 .../deps/libanstyle_wincon-093eeaf4fde10ca9.rmeta  |  Bin 0 -> 25963 bytes
 .../debug/deps/libanvilml-b9974bf43b5c3c6e.rmeta   |    0
 .../debug/deps/libanvilml-bef43214308623c1.rmeta   |  Bin 0 -> 6693 bytes
 .../debug/deps/libanvilml-c2fceda839b47553.rmeta   |  Bin 0 -> 6693 bytes
 .../debug/deps/libanvilml-e8e81f0dcfc148ed.rmeta   |    0
 .../libanvilml_artifacts-c5f438a60fc7590d.rmeta    |  Bin 0 -> 2101 bytes
 .../deps/libanvilml_core-280b4a8b3cdee509.rmeta    |  Bin 0 -> 25502 bytes
 .../libanvilml_hardware-083fd03b9bef5d5c.rmeta     |  Bin 0 -> 2140 bytes
 .../libanvilml_hardware-4868f6b7f9423884.rmeta     |  Bin 0 -> 2140 bytes
 .../deps/libanvilml_ipc-0790952ee62642bf.rmeta     |  Bin 0 -> 2107 bytes
 .../deps/libanvilml_openapi-7173169da8cafe60.rmeta |    0
 .../libanvilml_registry-e2776a04f801ad70.rmeta     |  Bin 0 -> 2138 bytes
 .../libanvilml_scheduler-35cec301528d98df.rmeta    |  Bin 0 -> 2120 bytes
 .../libanvilml_scheduler-b11884f833d5e19d.rmeta    |  Bin 0 -> 2120 bytes
 .../deps/libanvilml_server-0fb13a07f5000556.rmeta  |  Bin 0 -> 8037 bytes
 .../deps/libanvilml_server-2844092ce7abd77d.rmeta  |  Bin 0 -> 8037 bytes
 .../deps/libanvilml_worker-67a5fd84c7f48c28.rmeta  |  Bin 0 -> 2098 bytes
 .../deps/libanvilml_worker-d2a5c69aeb91332b.rmeta  |  Bin 0 -> 2098 bytes
 .../debug/deps/libatoi-78e2ba7c31d7ceba.rmeta      |  Bin 0 -> 32443 bytes
 .../deps/libatomic_waker-f4e43e4522d78c56.rmeta    |  Bin 0 -> 13866 bytes
 .../debug/deps/libaxum-3463f72e7c5406dd.rmeta      |  Bin 0 -> 4614580 bytes
 .../debug/deps/libaxum_core-90230d4de87a524e.rmeta |  Bin 0 -> 1566809 bytes
 .../debug/deps/libbase64-bedf05073541d7a2.rmeta    |  Bin 0 -> 165187 bytes
 .../deps/libblock_buffer-2f25de0d23b3df40.rmeta    |  Bin 0 -> 28051 bytes
 .../debug/deps/libbytes-d7b8d484cb31b5de.rmeta     |  Bin 0 -> 584987 bytes
 .../debug/deps/libcfg_if-82f15ab1f86a6ef5.rmeta    |  Bin 0 -> 5471 bytes
 .../debug/deps/libclap-fab9f85fc05fa2dc.rmeta      |  Bin 0 -> 12507 bytes
 .../deps/libclap_builder-f0ed1f4bb94ccd73.rmeta    |  Bin 0 -> 1637807 bytes
 .../debug/deps/libclap_lex-bbdf52239761cfc0.rmeta  |  Bin 0 -> 55816 bytes
 .../deps/libcolorchoice-2b5d3b391305ba40.rmeta     |  Bin 0 -> 11409 bytes
 .../libconcurrent_queue-2347621bc9e1af66.rmeta     |  Bin 0 -> 95708 bytes
 .../deps/libcpufeatures-a2f96a33d4923769.rmeta     |  Bin 0 -> 39280 bytes
 .../debug/deps/libcrc-c839f323a0300d80.rmeta       |  Bin 0 -> 286147 bytes
 .../deps/libcrc_catalog-5782bd9f84f2a313.rmeta     |  Bin 0 -> 101229 bytes
 .../deps/libcrossbeam_queue-5de67a7164c4494b.rmeta |  Bin 0 -> 40537 bytes
 .../deps/libcrossbeam_utils-e9077ada970e3223.rmeta |  Bin 0 -> 291127 bytes
 .../deps/libcrypto_common-4de70b43f9ab8ac5.rmeta   |  Bin 0 -> 22018 bytes
 .../debug/deps/libdigest-a9bbe63123c7c09d.rmeta    |  Bin 0 -> 105793 bytes
 .../debug/deps/libeither-cfb383dc9146c715.rmeta    |  Bin 0 -> 187310 bytes
 .../deps/libequivalent-24ddf9bd662ef2d0.rmeta      |  Bin 0 -> 7609 bytes
 .../deps/libevent_listener-641594697b721f4d.rmeta  |  Bin 0 -> 239793 bytes
 .../debug/deps/libflume-c32c895db72b7f37.rmeta     |  Bin 0 -> 187163 bytes
 .../debug/deps/libfoldhash-f789545bb71bddcb.rmeta  |  Bin 0 -> 65262 bytes
 .../deps/libform_urlencoded-dea1f4fc7d31c34b.rmeta |  Bin 0 -> 27066 bytes
 .../deps/libfutures_channel-d0e168c418386317.rmeta |  Bin 0 -> 142573 bytes
 .../deps/libfutures_core-ca4c3b14fe7fad58.rmeta    |  Bin 0 -> 54909 bytes
 .../libfutures_executor-dee0175459769ed6.rmeta     |  Bin 0 -> 42565 bytes
 .../libfutures_intrusive-9683da98bd4ada24.rmeta    |  Bin 0 -> 618822 bytes
 .../deps/libfutures_io-090c98f3b411d1ca.rmeta      |  Bin 0 -> 54129 bytes
 .../deps/libfutures_sink-df578d2515b56090.rmeta    |  Bin 0 -> 14598 bytes
 .../deps/libfutures_task-5dad006b4bd23b12.rmeta    |  Bin 0 -> 72168 bytes
 .../deps/libfutures_util-d40b90145b0efd4b.rmeta    |  Bin 0 -> 4605933 bytes
 .../deps/libgeneric_array-74f4bc2011284f94.rmeta   |  Bin 0 -> 665640 bytes
 .../debug/deps/libgetrandom-eb7fda387c72051a.rmeta |  Bin 0 -> 51936 bytes
 .../debug/deps/libhashbrown-1111c3b23d930e8b.rmeta |  Bin 0 -> 880250 bytes
 .../debug/deps/libhashbrown-4b308be5599da50a.rmeta |  Bin 0 -> 971549 bytes
 .../debug/deps/libhashlink-f35e73614c0deb13.rmeta  |  Bin 0 -> 302368 bytes
 .../debug/deps/libhttp-1247c2b6960eab76.rmeta      |  Bin 0 -> 1068315 bytes
 .../debug/deps/libhttp_body-91c5d3e3cf12f69d.rmeta |  Bin 0 -> 33422 bytes
 .../deps/libhttp_body_util-e847d18421b494d2.rmeta  |  Bin 0 -> 259962 bytes
 .../debug/deps/libhttparse-ee7f8d9422e33d40.rmeta  |  Bin 0 -> 118271 bytes
 .../debug/deps/libhttpdate-6817f98c5ffb8b45.rmeta  |  Bin 0 -> 23878 bytes
 .../debug/deps/libhyper-b379a1e497cb52a2.rmeta     |  Bin 0 -> 718793 bytes
 .../deps/libhyper_util-04bfbecb96584ae1.rmeta      |  Bin 0 -> 303474 bytes
 .../deps/libicu_collections-30252bf0cf20f494.rmeta |  Bin 0 -> 292815 bytes
 .../deps/libicu_locale_core-fc7a7efdd8297031.rmeta |  Bin 0 -> 1302136 bytes
 .../deps/libicu_normalizer-f684dddf5f8ea9bb.rmeta  |  Bin 0 -> 439343 bytes
 .../libicu_normalizer_data-fbeb9f08442df82a.rmeta  |  Bin 0 -> 435731 bytes
 .../deps/libicu_properties-5716706615c6f530.rmeta  |  Bin 0 -> 2295606 bytes
 .../libicu_properties_data-c46881417b72739b.rmeta  |  Bin 0 -> 1853739 bytes
 .../deps/libicu_provider-7f05da8e60683a15.rmeta    |  Bin 0 -> 342473 bytes
 .../debug/deps/libidna-8f829d12fa4f87e0.rmeta      |  Bin 0 -> 141257 bytes
 .../deps/libidna_adapter-88769aa14e94bba1.rmeta    |  Bin 0 -> 27255 bytes
 .../debug/deps/libindexmap-5f74cfa3b09b595c.rmeta  |  Bin 0 -> 902247 bytes
 .../libis_terminal_polyfill-4f9a4e49de449032.rmeta |  Bin 0 -> 8396 bytes
 .../debug/deps/libitoa-33db2b7cec95af6d.rmeta      |  Bin 0 -> 49196 bytes
 .../deps/liblibsqlite3_sys-f9b8ac94e3c98d70.rmeta  |  Bin 0 -> 394480 bytes
 .../debug/deps/liblitemap-b67fb40615c80f2c.rmeta   |  Bin 0 -> 190003 bytes
 .../debug/deps/liblock_api-5848eda82ccf8c09.rmeta  |  Bin 0 -> 285819 bytes
 .../debug/deps/liblog-02816c9ddfb714e5.rmeta       |  Bin 0 -> 162540 bytes
 .../debug/deps/libmatchit-ffc0aa833c3d15d0.rmeta   |  Bin 0 -> 102000 bytes
 .../debug/deps/libmemchr-18fea648d20f8e04.rmeta    |  Bin 0 -> 974499 bytes
 .../debug/deps/libmime-8a5f3e026c075afc.rmeta      |  Bin 0 -> 106024 bytes
 .../debug/deps/libmio-c5ed47f2512781f4.rmeta       |  Bin 0 -> 413007 bytes
 .../deps/libnum_traits-b1e55a3a951e567a.rmeta      |  Bin 0 -> 1250902 bytes
 .../debug/deps/libonce_cell-2779b49c3849fca0.rmeta |  Bin 0 -> 138363 bytes
 .../libonce_cell_polyfill-45c2e8ac13ff7e72.rmeta   |  Bin 0 -> 9463 bytes
 .../debug/deps/libparking-0b7b4040bb7a428b.rmeta   |  Bin 0 -> 18563 bytes
 .../deps/libparking_lot-a439919e56a6115d.rmeta     |  Bin 0 -> 152240 bytes
 .../libparking_lot_core-5c7ae4c5c84b561c.rmeta     |  Bin 0 -> 111035 bytes
 .../libpercent_encoding-fe35a79fb310a134.rmeta     |  Bin 0 -> 44979 bytes
 .../libpin_project_lite-091d61193fc3c465.rmeta     |  Bin 0 -> 92710 bytes
 .../deps/libpotential_utf-328f69d5421fe5af.rmeta   |  Bin 0 -> 37176 bytes
 .../debug/deps/libryu-3b2e873a4fe9dfe4.rmeta       |  Bin 0 -> 51800 bytes
 .../deps/libscopeguard-fef4712d3f25d2a7.rmeta      |  Bin 0 -> 21869 bytes
 .../debug/deps/libserde-af877e32508284a7.rmeta     |  Bin 0 -> 515283 bytes
 .../deps/libserde_core-d291707330e45a52.rmeta      |  Bin 0 -> 2664350 bytes
 .../deps/libserde_json-5d230c5f1d448a3d.rmeta      |  Bin 0 -> 1043451 bytes
 .../libserde_path_to_error-dcf4e04b6ebe4aaa.rmeta  |  Bin 0 -> 224091 bytes
 .../libserde_urlencoded-96805d660f2731d6.rmeta     |  Bin 0 -> 132818 bytes
 .../debug/deps/libsha2-74baf0e8b497907d.rmeta      |  Bin 0 -> 633698 bytes
 .../debug/deps/libslab-e64c3e7af05aca81.rmeta      |  Bin 0 -> 79061 bytes
 .../debug/deps/libsmallvec-fa499917f6cfe4f3.rmeta  |  Bin 0 -> 113758 bytes
 .../debug/deps/libsocket2-abcba000c21f9343.rmeta   |  Bin 0 -> 303423 bytes
 .../debug/deps/libspin-5270311b953784d8.rmeta      |  Bin 0 -> 198918 bytes
 .../debug/deps/libsqlite3_sys-f9b8ac94e3c98d70.d   |    9 +
 .../debug/deps/libsqlx-f8e24a55c3a97c65.rmeta      |  Bin 0 -> 124129 bytes
 .../debug/deps/libsqlx_core-31dac1e98296e2ce.rmeta |  Bin 0 -> 2780578 bytes
 .../deps/libsqlx_sqlite-6fb81d30905e7d95.rmeta     |  Bin 0 -> 1207394 bytes
 .../libstable_deref_trait-4d82d7797a1912a8.rmeta   |  Bin 0 -> 10262 bytes
 .../debug/deps/libstrsim-e51399f6a3e60186.rmeta    |  Bin 0 -> 35414 bytes
 .../deps/libsync_wrapper-f34a40796c862f5f.rmeta    |  Bin 0 -> 15792 bytes
 .../debug/deps/libthiserror-ba02c471537cd1de.rmeta |  Bin 0 -> 28571 bytes
 .../debug/deps/libtinystr-ad7d502c4bafe7a1.rmeta   |  Bin 0 -> 251301 bytes
 .../debug/deps/libtokio-e07d07b8ea59b346.rmeta     |  Bin 0 -> 7586916 bytes
 .../debug/deps/libtower-95f8dc8466943d7b.rmeta     |  Bin 0 -> 656748 bytes
 .../deps/libtower_layer-92f2304324034f26.rmeta     |  Bin 0 -> 59968 bytes
 .../deps/libtower_service-93873efef5849f12.rmeta   |  Bin 0 -> 21934 bytes
 .../debug/deps/libtracing-f553fdb7c78d8121.rmeta   |  Bin 0 -> 561504 bytes
 .../deps/libtracing_core-2858a7200499f1b5.rmeta    |  Bin 0 -> 490436 bytes
 .../debug/deps/libtypenum-60fc9221c6d2e470.rmeta   |  Bin 0 -> 2489561 bytes
 .../debug/deps/liburl-94d1069f492dc93b.rmeta       |  Bin 0 -> 290889 bytes
 .../debug/deps/libutf8_iter-92865f1d0fb90837.rmeta |  Bin 0 -> 26436 bytes
 .../debug/deps/libutf8parse-5445efc84cc000d7.rmeta |  Bin 0 -> 20605 bytes
 .../debug/deps/libuuid-3e964b0f7d79f283.rmeta      |  Bin 0 -> 430509 bytes
 .../deps/libwindows_link-553d59391106fe5c.rmeta    |  Bin 0 -> 4091 bytes
 .../deps/libwindows_sys-abd82277c412b9e1.rmeta     |  Bin 0 -> 9381235 bytes
 .../debug/deps/libwriteable-bbb689a21439f365.rmeta |  Bin 0 -> 140521 bytes
 .../debug/deps/libyoke-e85007fa4509aa79.rmeta      |  Bin 0 -> 187580 bytes
 .../debug/deps/libzerofrom-8e9bb4425404b034.rmeta  |  Bin 0 -> 53257 bytes
 .../debug/deps/libzerotrie-185b5d5760b4f64b.rmeta  |  Bin 0 -> 340017 bytes
 .../debug/deps/libzerovec-91b199928eaf7f90.rmeta   |  Bin 0 -> 802957 bytes
 .../debug/deps/libzmij-0f2ba072a948bcd3.rmeta      |  Bin 0 -> 367642 bytes
 .../debug/deps/litemap-b67fb40615c80f2c.d          |    8 +
 .../debug/deps/lock_api-5848eda82ccf8c09.d         |    8 +
 .../debug/deps/log-02816c9ddfb714e5.d              |    8 +
 .../debug/deps/matchit-ffc0aa833c3d15d0.d          |   10 +
 .../debug/deps/memchr-18fea648d20f8e04.d           |   31 +
 .../debug/deps/mime-8a5f3e026c075afc.d             |    6 +
 .../debug/deps/mio-c5ed47f2512781f4.d              |   34 +
 .../debug/deps/num_traits-b1e55a3a951e567a.d       |   23 +
 .../debug/deps/once_cell-2779b49c3849fca0.d        |    7 +
 .../deps/once_cell_polyfill-45c2e8ac13ff7e72.d     |    6 +
 .../debug/deps/parking-0b7b4040bb7a428b.d          |    5 +
 .../debug/deps/parking_lot-a439919e56a6115d.d      |   17 +
 .../debug/deps/parking_lot_core-5c7ae4c5c84b561c.d |   14 +
 .../debug/deps/percent_encoding-fe35a79fb310a134.d |    6 +
 .../debug/deps/pin_project_lite-091d61193fc3c465.d |    5 +
 .../debug/deps/potential_utf-328f69d5421fe5af.d    |    7 +
 .../debug/deps/ryu-3b2e873a4fe9dfe4.d              |   16 +
 .../debug/deps/scopeguard-fef4712d3f25d2a7.d       |    5 +
 .../debug/deps/serde-af877e32508284a7.d            |   12 +
 .../debug/deps/serde_core-d291707330e45a52.d       |   25 +
 .../debug/deps/serde_json-5d230c5f1d448a3d.d       |   21 +
 .../deps/serde_path_to_error-dcf4e04b6ebe4aaa.d    |    9 +
 .../debug/deps/serde_urlencoded-96805d660f2731d6.d |   11 +
 .../debug/deps/sha2-74baf0e8b497907d.d             |   13 +
 .../debug/deps/slab-e64c3e7af05aca81.d             |    6 +
 .../debug/deps/smallvec-fa499917f6cfe4f3.d         |    5 +
 .../debug/deps/socket2-abcba000c21f9343.d          |    9 +
 .../debug/deps/spin-5270311b953784d8.d             |   12 +
 .../debug/deps/sqlx-f8e24a55c3a97c65.d             |   12 +
 .../debug/deps/sqlx_core-31dac1e98296e2ce.d        |   97 +
 .../debug/deps/sqlx_sqlite-6fb81d30905e7d95.d      |   50 +
 .../deps/stable_deref_trait-4d82d7797a1912a8.d     |    5 +
 .../debug/deps/strsim-e51399f6a3e60186.d           |    5 +
 .../debug/deps/sync_wrapper-f34a40796c862f5f.d     |    5 +
 .../debug/deps/thiserror-ba02c471537cd1de.d        |   12 +
 .../debug/deps/tinystr-ad7d502c4bafe7a1.d          |   12 +
 .../debug/deps/tokio-e07d07b8ea59b346.d            |  275 ++
 .../debug/deps/tower-95f8dc8466943d7b.d            |   41 +
 .../debug/deps/tower_layer-92f2304324034f26.d      |    9 +
 .../debug/deps/tower_service-93873efef5849f12.d    |    5 +
 .../debug/deps/tracing-f553fdb7c78d8121.d          |   12 +
 .../debug/deps/tracing_core-2858a7200499f1b5.d     |   14 +
 .../debug/deps/typenum-60fc9221c6d2e470.d          |   17 +
 .../debug/deps/url-94d1069f492dc93b.d              |   11 +
 .../debug/deps/utf8_iter-92865f1d0fb90837.d        |    7 +
 .../debug/deps/utf8parse-5445efc84cc000d7.d        |    6 +
 .../debug/deps/uuid-3e964b0f7d79f283.d             |   16 +
 .../debug/deps/windows_link-553d59391106fe5c.d     |    6 +
 .../debug/deps/windows_sys-abd82277c412b9e1.d      |   28 +
 .../debug/deps/writeable-bbb689a21439f365.d        |   11 +
 .../debug/deps/yoke-e85007fa4509aa79.d             |   13 +
 .../debug/deps/zerofrom-8e9bb4425404b034.d         |    7 +
 .../debug/deps/zerotrie-185b5d5760b4f64b.d         |   19 +
 .../debug/deps/zerovec-91b199928eaf7f90.d          |   28 +
 .../debug/deps/zmij-0f2ba072a948bcd3.d             |    7 +
 .../dep-graph.bin                                  |  Bin 0 -> 80239 bytes
 .../metadata.rmeta                                 |  Bin 0 -> 6693 bytes
 .../query-cache.bin                                |  Bin 0 -> 11432 bytes
 .../work-products.bin                              |  Bin 0 -> 100 bytes
 .../s-hju7fu1xic-0tncbwm.lock                      |    0
 .../dep-graph.bin                                  |  Bin 0 -> 1749464 bytes
 .../query-cache.bin                                |  Bin 0 -> 602200 bytes
 .../work-products.bin                              |  Bin 0 -> 50 bytes
 .../s-hju7fu305z-18hsf1t.lock                      |    0
 .../dep-graph.bin                                  |  Bin 0 -> 1749464 bytes
 .../query-cache.bin                                |  Bin 0 -> 602200 bytes
 .../work-products.bin                              |  Bin 0 -> 50 bytes
 .../s-hju7fukks1-057zogw.lock                      |    0
 .../dep-graph.bin                                  |  Bin 0 -> 80239 bytes
 .../metadata.rmeta                                 |  Bin 0 -> 6693 bytes
 .../query-cache.bin                                |  Bin 0 -> 11432 bytes
 .../work-products.bin                              |  Bin 0 -> 100 bytes
 .../s-hju7fuji27-157v0ya.lock                      |    0
 .../dep-graph.bin                                  |  Bin 0 -> 17090 bytes
 .../metadata.rmeta                                 |  Bin 0 -> 2101 bytes
 .../query-cache.bin                                |  Bin 0 -> 803 bytes
 .../work-products.bin                              |  Bin 0 -> 100 bytes
 .../s-hju7ftz7j5-024fb2v.lock                      |    0
 .../dep-graph.bin                                  |  Bin 0 -> 998045 bytes
 .../metadata.rmeta                                 |  Bin 0 -> 25502 bytes
 .../query-cache.bin                                |  Bin 0 -> 408568 bytes
 .../work-products.bin                              |  Bin 0 -> 100 bytes
 .../s-hju7ftwdhy-08rh31y.lock                      |    0
 .../dep-graph.bin                                  |  Bin 0 -> 17090 bytes
 .../metadata.rmeta                                 |  Bin 0 -> 2140 bytes
 .../query-cache.bin                                |  Bin 0 -> 803 bytes
 .../work-products.bin                              |  Bin 0 -> 100 bytes
 .../s-hju7ftz70f-15fnpzc.lock                      |    0
 .../dep-graph.bin                                  |  Bin 0 -> 17090 bytes
 .../metadata.rmeta                                 |  Bin 0 -> 2140 bytes
 .../query-cache.bin                                |  Bin 0 -> 803 bytes
 .../work-products.bin                              |  Bin 0 -> 100 bytes
 .../s-hju7fugv1l-1sfeeew.lock                      |    0
 .../dep-graph.bin                                  |  Bin 0 -> 17090 bytes
 .../metadata.rmeta                                 |  Bin 0 -> 2107 bytes
 .../query-cache.bin                                |  Bin 0 -> 803 bytes
 .../work-products.bin                              |  Bin 0 -> 100 bytes
 .../s-hju7ftz7at-0iwiw85.lock                      |    0
 .../dep-graph.bin                                  |  Bin 0 -> 24951 bytes
 .../query-cache.bin                                |  Bin 0 -> 1644 bytes
 .../work-products.bin                              |  Bin 0 -> 50 bytes
 .../s-hju7fu1x8h-14u6a0f.lock                      |    0
 .../dep-graph.bin                                  |  Bin 0 -> 17090 bytes
 .../metadata.rmeta                                 |  Bin 0 -> 2138 bytes
 .../query-cache.bin                                |  Bin 0 -> 803 bytes
 .../work-products.bin                              |  Bin 0 -> 100 bytes
 .../s-hju7ftz8tm-0v5yvo8.lock                      |    0
 .../dep-graph.bin                                  |  Bin 0 -> 17090 bytes
 .../metadata.rmeta                                 |  Bin 0 -> 2120 bytes
 .../query-cache.bin                                |  Bin 0 -> 803 bytes
 .../work-products.bin                              |  Bin 0 -> 100 bytes
 .../s-hju7fuiap2-0svqhz4.lock                      |    0
 .../dep-graph.bin                                  |  Bin 0 -> 17090 bytes
 .../metadata.rmeta                                 |  Bin 0 -> 2120 bytes
 .../query-cache.bin                                |  Bin 0 -> 803 bytes
 .../work-products.bin                              |  Bin 0 -> 100 bytes
 .../s-hju7fu0mnx-1qh0yyt.lock                      |    0
 .../dep-graph.bin                                  |  Bin 0 -> 156012 bytes
 .../metadata.rmeta                                 |  Bin 0 -> 8037 bytes
 .../query-cache.bin                                |  Bin 0 -> 8659 bytes
 .../work-products.bin                              |  Bin 0 -> 100 bytes
 .../s-hju7fu0myw-0ardbod.lock                      |    0
 .../dep-graph.bin                                  |  Bin 0 -> 156012 bytes
 .../metadata.rmeta                                 |  Bin 0 -> 8037 bytes
 .../query-cache.bin                                |  Bin 0 -> 8659 bytes
 .../work-products.bin                              |  Bin 0 -> 100 bytes
 .../s-hju7fuiazm-0yjmm6s.lock                      |    0
 .../dep-graph.bin                                  |  Bin 0 -> 17090 bytes
 .../metadata.rmeta                                 |  Bin 0 -> 2098 bytes
 .../query-cache.bin                                |  Bin 0 -> 803 bytes
 .../work-products.bin                              |  Bin 0 -> 100 bytes
 .../s-hju7ftzwou-1nt4nh8.lock                      |    0
 .../dep-graph.bin                                  |  Bin 0 -> 17090 bytes
 .../metadata.rmeta                                 |  Bin 0 -> 2098 bytes
 .../query-cache.bin                                |  Bin 0 -> 803 bytes
 .../work-products.bin                              |  Bin 0 -> 100 bytes
 .../s-hju7fuhl8n-1mxuomf.lock                      |    0
 5900 files changed, 31040 insertions(+), 24 deletions(-)
```

## Test Results

```
     Running tests/error_tests.rs (target/debug/deps/error_tests-5ada70e8acf995f0)

running 16 tests
test test_artifact_not_found_returns_404 ... ok
test test_cycle_detected_returns_400 ... ok
test test_db_returns_500 ... ok
test test_error_body_has_request_id ... ok
test test_invalid_graph_returns_400 ... ok
test test_error_body_message_contains_variant_info ... ok
test test_error_field_is_snake_case ... ok
test test_io_returns_500 ... ok
test test_internal_returns_500 ... ok
test test_ipc_returns_400 ... ok
test test_job_not_found_returns_404 ... ok
test test_model_not_found_returns_404 ... ok
test test_payload_too_large_returns_413 ... ok
test test_serde_returns_400 ... ok
test test_workers_unavailable_returns_503 ... ok
test test_worker_not_found_returns_404 ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, all files formatted)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.59s

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.92s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.61s

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 14.76s
```

All four platform cross-checks passed.

## Project Gates

None applicable — task does not touch config fields, handler signatures, or node types.

## Public API Delta

```
+pub use error::AnvilError;
```

New pub items:
- `pub enum AnvilError` — `anvilml_core::AnvilError` (13-variant error enum, re-exported from `lib.rs`)
- `impl IntoResponse for AnvilError` — `anvilml_core::AnvilError` (implements `axum::response::IntoResponse`)

## Deviations from Plan

- **Added `serde` dependency**: The plan did not list `serde` as a dependency, but it is required because `ErrorBody` derives `serde::Serialize` and contains a `uuid::Uuid` field (which requires the `serde` feature flag). Added `serde = { version = "1.0", features = ["derive"] }` to `[dependencies]`.
- **Added `uuid` serde feature**: The plan specified `uuid = { version = "1.23.4", features = ["v4"] }` but the `serde` feature is also needed for `ErrorBody` serialization. Changed to `uuid = { version = "1.23.4", features = ["v4", "serde"] }`.
- **`StatusCode` import path**: Plan implied `axum::response::StatusCode` but `StatusCode` is not re-exported from `axum::response` in axum 0.8.9. Used `axum::http::StatusCode` instead.
- **Response body reading in tests**: Plan suggested using `.into_response()` and inspecting the body. The implementation uses `axum::body::to_bytes().await` in async test helpers since `block_on` cannot be called from within a `#[tokio::test]`.
- **Version bump**: Changed `version.workspace = true` to `version = "0.1.1"` (from workspace default 0.1.0 → 0.1.1) since the crate needs its own version per the bump convention.

## Blockers

None.
