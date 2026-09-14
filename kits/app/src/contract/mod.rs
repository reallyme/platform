// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Host-neutral app contract descriptors.

mod binding;
mod descriptor;
mod http;
mod name;

pub use binding::{
    AppDependencyBinding, AppDependencyBindingKey, AppDependencyBindingMode,
    AppDependencyBindingTarget,
};
pub use descriptor::{AppContractDescriptor, AppContractService};
pub use http::{AppHttpDtoConvention, AppHttpRouteContract};
pub use name::AppContractName;
