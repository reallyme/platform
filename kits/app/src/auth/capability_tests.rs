// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{AppCapability, AppCapabilityName};

#[test]
fn app_capabilities_are_typed() {
    let capability_name =
        AppCapabilityName::new("connect_rpc").expect("valid capability name fixture");
    let capability = AppCapability::new(capability_name);

    assert_eq!(capability.name().as_str(), "connect_rpc");
}
