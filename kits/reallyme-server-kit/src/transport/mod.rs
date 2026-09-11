// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe Technologies Limited. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Transport-neutral correlation identifier primitives.

mod ids;

pub use ids::{IdentifierValueError, RequestId, TraceId};
