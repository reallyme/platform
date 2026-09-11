// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::DeploymentRegion;
use super::{ServerName, StartupError, TaskName};

#[test]
fn accepts_dns_friendly_server_names() {
    let result = ServerName::new("reallyme-api");

    assert_eq!(
        result.map(|server_name| server_name.as_str().to_owned()),
        Ok("reallyme-api".to_owned())
    );
}

#[test]
fn rejects_invalid_server_names() {
    let result = ServerName::new("ReallyMe API");

    assert_eq!(result, Err(StartupError::InvalidServerName));
}

#[test]
fn rejects_server_names_with_invalid_boundaries() {
    let result = ServerName::new("-reallyme-api");

    assert_eq!(result, Err(StartupError::InvalidServerNameBoundary));
}

#[test]
fn rejects_too_long_server_names() {
    let name = "a".repeat(64);
    let result = ServerName::new(name);

    assert_eq!(result, Err(StartupError::ServerNameTooLong));
}

#[test]
fn accepts_deployment_region_labels() {
    let region = DeploymentRegion::new("eu-west-1").expect("valid deployment region");

    assert_eq!(region.as_str(), "eu-west-1");
}

#[test]
fn rejects_invalid_deployment_region_labels() {
    assert_eq!(
        DeploymentRegion::new("EU West 1"),
        Err(StartupError::InvalidDeploymentRegion)
    );
}

#[test]
fn accepts_dns_friendly_task_names() {
    let result = TaskName::new("domain-crawler");

    assert_eq!(
        result.map(|task_name| task_name.as_str().to_owned()),
        Ok("domain-crawler".to_owned())
    );
}

#[test]
fn rejects_invalid_task_names() {
    let result = TaskName::new("Domain Crawler");

    assert_eq!(result, Err(StartupError::InvalidTaskName));
}

#[test]
fn rejects_task_names_with_invalid_boundaries() {
    let result = TaskName::new("-domain-crawler");

    assert_eq!(result, Err(StartupError::InvalidTaskNameBoundary));
}

#[test]
fn rejects_too_long_task_names() {
    let name = "a".repeat(64);
    let result = TaskName::new(name);

    assert_eq!(result, Err(StartupError::TaskNameTooLong));
}
