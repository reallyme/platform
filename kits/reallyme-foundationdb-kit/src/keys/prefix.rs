// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed key prefixes.

use crate::keys::{error::KeyLabelError, validation::validate_static_key_label};

/// A stable key prefix owned by a namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPrefix {
    value: &'static str,
}

impl KeyPrefix {
    /// Creates a prefix from a compile-time allowlisted value.
    pub fn from_static(value: &'static str) -> Result<Self, KeyLabelError> {
        validate_static_key_label(value)?;
        Ok(Self { value })
    }

    /// Returns the textual prefix label.
    pub const fn as_str(self) -> &'static str {
        self.value
    }
}
