// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typesense schema field types.

use serde::{Deserialize, Serialize};

/// Supported Typesense field kinds used by ReallyMe index schemas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldKind {
    /// UTF-8 string field.
    #[serde(rename = "string")]
    String,
    /// String array field.
    #[serde(rename = "string[]")]
    StringArray,
    /// Signed 32-bit integer field.
    #[serde(rename = "int32")]
    Int32,
    /// Signed 32-bit integer array field.
    #[serde(rename = "int32[]")]
    Int32Array,
    /// Signed 64-bit integer field.
    #[serde(rename = "int64")]
    Int64,
    /// Signed 64-bit integer array field.
    #[serde(rename = "int64[]")]
    Int64Array,
    /// 32-bit floating point field.
    #[serde(rename = "float")]
    Float,
    /// 32-bit floating point array field.
    #[serde(rename = "float[]")]
    FloatArray,
    /// Boolean field.
    #[serde(rename = "bool")]
    Bool,
    /// Boolean array field.
    #[serde(rename = "bool[]")]
    BoolArray,
    /// Latitude and longitude field.
    #[serde(rename = "geopoint")]
    Geopoint,
    /// Array of latitude and longitude points.
    #[serde(rename = "geopoint[]")]
    GeopointArray,
    /// Nested object field.
    #[serde(rename = "object")]
    Object,
    /// Array of nested objects.
    #[serde(rename = "object[]")]
    ObjectArray,
}
