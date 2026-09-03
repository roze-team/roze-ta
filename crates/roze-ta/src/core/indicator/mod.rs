// SPDX-License-Identifier: Apache-2.0
// Derived from Yata v0.7.0, src/core/indicator/mod.rs.
// Copyright 2020 AMvDev (amv-dev@protonmail.com).
// Modified 2026 by roze-ta contributors: native module integration, fixed types,
// unconditional serialization, safe indexing and Rust formatting.
// See THIRD-PARTY-NOTICES.md and docs/patches/yata-native-migration.md.

//! Every indicator has it's own **Configuration** and **State**.
//!
//! Every indicator **Configuration** must implement [`IndicatorConfig`].
//!
//! Every indicator **State** must implement [`IndicatorInstance`].

mod config;
mod dd;
mod instance;
mod result;

pub use config::*;
pub use dd::*;
pub use instance::*;
pub use result::*;
