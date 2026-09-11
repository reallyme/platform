// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! gRPC message-size policy helpers.

/// Default maximum decoded inbound gRPC message size for app-owned services.
pub const DEFAULT_GRPC_DECODING_MESSAGE_SIZE_BYTES: usize = 4 * 1024 * 1024;

/// Default maximum encoded outbound gRPC message size for app-owned services.
pub const DEFAULT_GRPC_ENCODING_MESSAGE_SIZE_BYTES: usize = 4 * 1024 * 1024;
