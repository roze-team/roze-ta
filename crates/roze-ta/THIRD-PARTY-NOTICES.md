# Third-party notices

The native algorithm modules in `crates/roze-ta/src/core/`, `helpers/`,
`indicators/`, `methods/`, and `prelude.rs` are derived from **Yata v0.7.0**:

- Author and copyright: Copyright 2020 AMvDev (amv-dev@protonmail.com).
- Source: https://github.com/amv-dev/yata
- Upstream commit: `5030e2349cedde60b0e367a9de9400d466ff644f`.
- License: Apache License, Version 2.0.

The original archive, including its README, license and copyright notices,
remains unmodified in `vendor/yata/` for audit. It is excluded from the Cargo
workspace and is not a build dependency. `UPSTREAM.json` records original hashes.

Roze modifications (2026): native module integration, fixed f64/u8 types,
unconditional serde support, removal of optional unsafe implementations,
documentation imports, formatting, and compiler/Clippy style adjustments.
The existing formulas and serialized state layout are preserved. The migration
manifest and patch are documented in `docs/patches/yata-native-migration.md`.

These derived files retain Apache-2.0 terms; they are not relicensed as MIT-only
or represented as original Roze work. Original Roze code remains under MIT.
The combined `roze-ta` crate declares `MIT AND Apache-2.0` and includes
`LICENSE-MIT`, `LICENSE-APACHE`, and this notice in its package directory.

For a standalone crate copy, the derived module paths above are relative to
that crate's `src/`; its per-file headers retain the upstream source paths.
