# Tests

- After an edit: `cargo test` (debug build, ~4 s). Narrow it to the area you
  touched while working, e.g. `cargo test sim::tests::night`.
- Before merging: `cargo test --release --features slow-tests`.
- Keep new tests lean: simulate only as long as the point needs, build one
  world, no second run to check determinism (`same_seed_same_history` covers
  it). Long scenarios get
  `#[cfg_attr(not(feature = "slow-tests"), ignore = "slow: cargo test --features slow-tests")]`.
