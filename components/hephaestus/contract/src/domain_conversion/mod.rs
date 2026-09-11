// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Conversion between Hephaestus wire DTOs and domain types.

mod hephaestus;
mod vultr;

pub use hephaestus::{
    agent_boot_report_from_proto, agent_report_from_proto, dns_desired_state_from_proto,
    node_actual_state_from_proto, proto_agent_boot_report_from_domain,
    proto_agent_boot_report_result_from_domain, proto_agent_report_from_domain,
    proto_dns_desired_state_from_domain, proto_node_actual_state_from_domain,
    proto_provisioning_event_snapshot_from_domain, proto_region_definition_from_domain,
    proto_server_inventory_row_from_domain, region_definition_from_proto,
};
pub use vultr::{
    proto_vultr_container_artifact_from_domain, proto_vultr_container_registry_from_domain,
    proto_vultr_container_repository_from_domain, proto_vultr_instance_from_domain,
    proto_vultr_instance_status_from_domain, proto_vultr_instance_template_from_domain,
    proto_vultr_plan_from_domain, proto_vultr_plan_type_from_domain,
    proto_vultr_region_from_domain, proto_vultr_vpc_from_domain,
    vultr_container_artifact_from_proto, vultr_container_registry_from_proto,
    vultr_container_repository_from_proto, vultr_instance_from_proto,
    vultr_instance_status_from_proto_i32, vultr_instance_template_from_proto,
    vultr_plan_from_proto, vultr_plan_type_from_proto_i32, vultr_region_from_proto,
    vultr_vpc_from_proto,
};
