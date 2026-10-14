use environment_manager::{
    DestructiveAction, EnvironmentMode, EnvironmentRecord, EnvironmentState, ExternalSandboxTarget,
    OwnershipError, authorize_destructive_action, authorize_external_sandbox_action,
};

fn managed() -> EnvironmentRecord {
    EnvironmentRecord {
        env_id: "env-1".to_owned(),
        instance_id: "instance-1".to_owned(),
        pack_id: "hr-sql-101".to_owned(),
        pack_version: "1.0.0".to_owned(),
        pack_digest: "a".repeat(64),
        namespace: "sj_env_1".to_owned(),
        ownership_nonce: "nonce-1".to_owned(),
        mode: EnvironmentMode::Managed,
        state: EnvironmentState::Ready,
    }
}

#[test]
fn matching_external_sandbox_target_authorizes_lifecycle_actions() {
    let mut record = managed();
    record.mode = EnvironmentMode::ExternalSandbox;
    let target = ExternalSandboxTarget {
        profile_id: "profile-training".to_owned(),
        server_identity: "postgresql://db.example.test:5432".to_owned(),
        database_identity: "training".to_owned(),
    };

    assert_eq!(
        authorize_external_sandbox_action(
            &record,
            Some(&target),
            DestructiveAction::Reset {
                env_id: "env-1".to_owned(),
                instance_id: "instance-1".to_owned(),
                ownership_nonce: "nonce-1".to_owned(),
                confirmed: true,
            }
        ),
        Ok(())
    );

    assert_eq!(
        authorize_external_sandbox_action(
            &record,
            None,
            DestructiveAction::Reset {
                env_id: "env-1".to_owned(),
                instance_id: "instance-1".to_owned(),
                ownership_nonce: "nonce-1".to_owned(),
                confirmed: true,
            }
        ),
        Err(OwnershipError::ExternalSandboxTargetRequired)
    );
}

#[test]
fn matching_managed_marker_and_confirmation_authorize_reset() {
    assert_eq!(
        authorize_destructive_action(
            &managed(),
            DestructiveAction::Reset {
                env_id: "env-1".to_owned(),
                instance_id: "instance-1".to_owned(),
                ownership_nonce: "nonce-1".to_owned(),
                confirmed: true,
            }
        ),
        Ok(())
    );
}

#[test]
fn forged_id_nonce_external_mode_and_running_state_are_denied() {
    let mut record = managed();
    record.state = EnvironmentState::Running;
    assert_eq!(
        authorize_destructive_action(
            &record,
            DestructiveAction::Delete {
                env_id: "env-1".to_owned(),
                instance_id: "instance-1".to_owned(),
                ownership_nonce: "nonce-1".to_owned(),
                confirmed: true,
            }
        ),
        Err(OwnershipError::Busy)
    );

    record.state = EnvironmentState::Ready;
    assert_eq!(
        authorize_destructive_action(
            &record,
            DestructiveAction::Reset {
                env_id: "forged".to_owned(),
                instance_id: "instance-1".to_owned(),
                ownership_nonce: "nonce-1".to_owned(),
                confirmed: true,
            }
        ),
        Err(OwnershipError::EnvironmentMismatch)
    );
    assert_eq!(
        authorize_destructive_action(
            &record,
            DestructiveAction::Reset {
                env_id: "env-1".to_owned(),
                instance_id: "instance-1".to_owned(),
                ownership_nonce: "wrong".to_owned(),
                confirmed: true,
            }
        ),
        Err(OwnershipError::OwnershipMismatch)
    );
    record.mode = EnvironmentMode::ExternalExplorer;
    assert_eq!(
        authorize_destructive_action(
            &record,
            DestructiveAction::Reset {
                env_id: "env-1".to_owned(),
                instance_id: "instance-1".to_owned(),
                ownership_nonce: "nonce-1".to_owned(),
                confirmed: true,
            }
        ),
        Err(OwnershipError::ExternalEnvironment)
    );
}
