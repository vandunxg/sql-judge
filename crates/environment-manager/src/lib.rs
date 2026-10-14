use serde::{Deserialize, Serialize};
use thiserror::Error;

mod ledger;

pub use ledger::{EnvironmentLedger, LifecycleError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvironmentMode {
    Managed,
    ExternalExplorer,
    ExternalSandbox,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExternalSandboxTarget {
    pub profile_id: String,
    pub server_identity: String,
    pub database_identity: String,
}

impl ExternalSandboxTarget {
    pub fn is_valid(&self) -> bool {
        is_safe_identifier(&self.profile_id)
            && is_safe_identity(&self.server_identity)
            && is_safe_identity(&self.database_identity)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvironmentState {
    Provisioning,
    Ready,
    Running,
    Rebuilding,
    Deleting,
    NeedsRecovery,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentRecord {
    pub env_id: String,
    pub instance_id: String,
    pub pack_id: String,
    pub pack_version: String,
    pub pack_digest: String,
    pub namespace: String,
    pub ownership_nonce: String,
    pub mode: EnvironmentMode,
    pub state: EnvironmentState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DestructiveAction {
    Reset {
        env_id: String,
        instance_id: String,
        ownership_nonce: String,
        confirmed: bool,
    },
    Delete {
        env_id: String,
        instance_id: String,
        ownership_nonce: String,
        confirmed: bool,
    },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum OwnershipError {
    #[error("destructive actions are only allowed for managed environments")]
    ExternalEnvironment,
    #[error("external sandbox requires a valid target binding")]
    ExternalSandboxTargetRequired,
    #[error("environment identity does not match the durable marker")]
    EnvironmentMismatch,
    #[error("database instance identity does not match the durable marker")]
    InstanceMismatch,
    #[error("ownership marker does not match")]
    OwnershipMismatch,
    #[error("explicit destructive confirmation is required")]
    ConfirmationRequired,
    #[error("environment is busy and cannot be destroyed")]
    Busy,
    #[error("environment is not in a destructible state")]
    InvalidState,
}

pub fn authorize_destructive_action(
    record: &EnvironmentRecord,
    action: DestructiveAction,
) -> Result<(), OwnershipError> {
    if record.mode != EnvironmentMode::Managed {
        return Err(OwnershipError::ExternalEnvironment);
    }
    authorize_action_identity(record, action)
}

pub fn authorize_external_sandbox_action(
    record: &EnvironmentRecord,
    target: Option<&ExternalSandboxTarget>,
    action: DestructiveAction,
) -> Result<(), OwnershipError> {
    if record.mode == EnvironmentMode::ExternalExplorer {
        return Err(OwnershipError::ExternalEnvironment);
    }
    if record.mode != EnvironmentMode::ExternalSandbox
        || !target.is_some_and(ExternalSandboxTarget::is_valid)
    {
        return Err(OwnershipError::ExternalSandboxTargetRequired);
    }
    authorize_action_identity(record, action)
}

fn authorize_action_identity(
    record: &EnvironmentRecord,
    action: DestructiveAction,
) -> Result<(), OwnershipError> {
    if !matches!(
        record.state,
        EnvironmentState::Ready | EnvironmentState::Error
    ) {
        return Err(
            if matches!(
                record.state,
                EnvironmentState::Running
                    | EnvironmentState::Rebuilding
                    | EnvironmentState::Deleting
            ) {
                OwnershipError::Busy
            } else {
                OwnershipError::InvalidState
            },
        );
    }

    let (env_id, instance_id, nonce, confirmed) = match action {
        DestructiveAction::Reset {
            env_id,
            instance_id,
            ownership_nonce,
            confirmed,
        }
        | DestructiveAction::Delete {
            env_id,
            instance_id,
            ownership_nonce,
            confirmed,
        } => (env_id, instance_id, ownership_nonce, confirmed),
    };
    if env_id != record.env_id {
        return Err(OwnershipError::EnvironmentMismatch);
    }
    if instance_id != record.instance_id {
        return Err(OwnershipError::InstanceMismatch);
    }
    if nonce != record.ownership_nonce {
        return Err(OwnershipError::OwnershipMismatch);
    }
    if !confirmed {
        return Err(OwnershipError::ConfirmationRequired);
    }
    Ok(())
}

fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn is_safe_identity(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
