// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Generic host-neutral app core wrapper.
///
/// App-kit owns this small wrapper because most apps need the same shape: a
/// core object containing validated app state. Concrete apps still own their
/// use-case functions, request/response DTOs, and domain behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StandardAppCore<TState> {
    state: TState,
}

impl<TState> StandardAppCore<TState> {
    /// Constructs an app core from app-owned state.
    pub const fn new(state: TState) -> Self {
        Self { state }
    }

    /// Returns the app-owned state.
    pub const fn state(&self) -> &TState {
        &self.state
    }
}
