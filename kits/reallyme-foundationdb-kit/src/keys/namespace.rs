// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed key namespaces.

use crate::keys::{error::KeyLabelError, validation::validate_static_key_label};

/// Stable namespace label for a family of keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyNamespace {
    value: &'static str,
}

impl KeyNamespace {
    /// Creates a namespace from a compile-time allowlisted value.
    pub fn from_static(value: &'static str) -> Result<Self, KeyLabelError> {
        validate_static_key_label(value)?;
        Ok(Self { value })
    }

    /// Returns the namespace label.
    pub const fn as_str(self) -> &'static str {
        self.value
    }
}
