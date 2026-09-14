// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{StartupBanner, should_emit_startup_banner, write_startup_banner};
use crate::config::{LogFormat, ServiceEnvironment};

const TEST_BANNER: StartupBanner = StartupBanner::new("test banner");

#[test]
fn startup_banner_preserves_host_supplied_text() {
    assert_eq!(TEST_BANNER.text(), "test banner");
}

#[test]
fn banner_is_enabled_only_for_local_or_dev_plain_text_logs() {
    assert!(should_emit_startup_banner(
        TEST_BANNER,
        ServiceEnvironment::Local,
        LogFormat::PlainText
    ));
    assert!(should_emit_startup_banner(
        TEST_BANNER,
        ServiceEnvironment::Dev,
        LogFormat::PlainText
    ));
    assert!(!should_emit_startup_banner(
        TEST_BANNER,
        ServiceEnvironment::Staging,
        LogFormat::PlainText
    ));
    assert!(!should_emit_startup_banner(
        TEST_BANNER,
        ServiceEnvironment::Prod,
        LogFormat::PlainText
    ));
    assert!(!should_emit_startup_banner(
        TEST_BANNER,
        ServiceEnvironment::Local,
        LogFormat::Json
    ));
    assert!(!should_emit_startup_banner(
        TEST_BANNER,
        ServiceEnvironment::Prod,
        LogFormat::Json
    ));
}

#[test]
fn banner_writer_respects_policy() {
    let mut local_output = Vec::new();
    let emitted = write_startup_banner(
        &mut local_output,
        TEST_BANNER,
        ServiceEnvironment::Local,
        LogFormat::PlainText,
    )
    .expect("writing local banner should succeed");
    assert!(emitted);
    assert_eq!(
        String::from_utf8(local_output).expect("banner output should be utf-8"),
        "test banner\n"
    );

    let mut prod_output = Vec::new();
    let emitted = write_startup_banner(
        &mut prod_output,
        TEST_BANNER,
        ServiceEnvironment::Prod,
        LogFormat::Json,
    )
    .expect("writing production banner should succeed");
    assert!(!emitted);
    assert!(prod_output.is_empty());
}

#[test]
fn empty_banner_is_never_emitted() {
    let mut output = Vec::new();
    let emitted = write_startup_banner(
        &mut output,
        StartupBanner::default(),
        ServiceEnvironment::Local,
        LogFormat::PlainText,
    )
    .expect("writing an empty banner should succeed");

    assert!(!emitted);
    assert!(output.is_empty());
}
