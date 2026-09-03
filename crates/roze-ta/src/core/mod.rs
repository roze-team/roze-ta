// SPDX-License-Identifier: Apache-2.0
// Derived from Yata v0.7.0, src/core/mod.rs.
// Copyright 2020 AMvDev (amv-dev@protonmail.com).
// Modified 2026 by roze-ta contributors: native module integration, fixed types,
// unconditional serialization, safe indexing and Rust formatting.
// See THIRD-PARTY-NOTICES.md and docs/patches/yata-native-migration.md.

// #![warn(missing_docs)]
#![warn(missing_debug_implementations)]
//! Some useful features and definitions

mod action;
mod candles;
mod errors;
mod indicator;
mod method;
mod moving_average;
mod ohlcv;
mod sequence;
mod window;

pub use action::Action;
pub use candles::*;
pub use errors::Error;
pub use indicator::*;
pub use method::Method;
pub use moving_average::*;
pub use ohlcv::OHLCV;
pub use sequence::*;
pub use window::Window;

/// Calculation value type, fixed to preserve the engine and snapshot contracts.
pub type ValueType = f64;

/// Low-level period type. Existing methods validate their supported 1..=254 range.
/// Wider periods require a separately versioned algorithm and snapshot migration.
pub type PeriodType = u8;
