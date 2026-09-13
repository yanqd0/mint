//! 派生状态纯函数与容器创建校验。

use super::*;
use crate::container::derive::{DeriveState, derive_status};
use crate::models::ContainerStatus;
use rstest::rstest;

/// derive_status 全分支参数化：子项状态组合 → 容器状态。
#[rstest]
#[case(&[], ContainerStatus::Open)]
#[case(&[DeriveState::Open, DeriveState::Open], ContainerStatus::Open)]
#[case(&[DeriveState::Active], ContainerStatus::Running)]
#[case(&[DeriveState::Active, DeriveState::Done], ContainerStatus::Running)]
#[case(&[DeriveState::Done, DeriveState::Done], ContainerStatus::Done)]
#[case(&[DeriveState::Dropped, DeriveState::Dropped], ContainerStatus::Dropped)]
#[case(&[DeriveState::Done, DeriveState::Dropped], ContainerStatus::Partial)]
#[case(&[DeriveState::Open, DeriveState::Done], ContainerStatus::Running)]
#[case(&[DeriveState::Open, DeriveState::Dropped], ContainerStatus::Running)]
#[case(
    &[DeriveState::Open, DeriveState::Done, DeriveState::Dropped],
    ContainerStatus::Running
)]
fn derive_status_cases(#[case] states: &[DeriveState], #[case] expected: ContainerStatus) {
    assert_eq!(derive_status(states), expected);
}

/// milestone create 必填 version。
#[test]
fn create_requires_version() {
    let (conn, _) = setup();
    let err = create(&conn, ContainerKind::Milestone, "r", None, None, None).unwrap_err();
    assert!(err.to_string().contains("--version"));
    let id = create(
        &conn,
        ContainerKind::Milestone,
        "r",
        Some("0.1.0"),
        None,
        None,
    )
    .unwrap();
    assert!(id > 0);
}
