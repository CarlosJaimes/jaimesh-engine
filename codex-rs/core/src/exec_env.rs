// Modified by JaiMesh contributors in 2026 from the OpenAI Codex source.
pub use codex_apply_patch::CODEX_APPLY_PATCH_PRESERVE_LINE_ENDINGS_ENV_VAR;
use codex_apply_patch::JAIMESH_APPLY_PATCH_PRESERVE_LINE_ENDINGS_ENV_VAR;
use codex_features::Feature;
use codex_features::Features;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
#[cfg(test)]
use codex_protocol::config_types::EnvironmentVariablePattern;
use codex_protocol::config_types::ShellEnvironmentPolicy;
use codex_protocol::models::ActivePermissionProfile;
use codex_protocol::shell_environment;
use std::collections::HashMap;

pub use codex_protocol::shell_environment::CODEX_SESSION_ID_ENV_VAR;
pub use codex_protocol::shell_environment::CODEX_THREAD_ID_ENV_VAR;
pub use codex_protocol::shell_environment::JAIMESH_THREAD_ID_ENV_VAR;

pub(crate) const CODEX_VERSION_ENV_VAR: &str = "CODEX_VERSION";
pub(crate) const JAIMESH_VERSION_ENV_VAR: &str = "JAIMESH_VERSION";

/// Informational name of the active permission profile. Child processes can
/// overwrite this value, so it must not be treated as proof of enforcement.
pub const CODEX_PERMISSION_PROFILE_ENV_VAR: &str = "CODEX_PERMISSION_PROFILE";
pub const JAIMESH_PERMISSION_PROFILE_ENV_VAR: &str = "JAIMESH_PERMISSION_PROFILE";

pub(crate) fn is_jaimesh_process() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.file_stem().map(|stem| stem == "jaimesh"))
        .unwrap_or(false)
}

/// Construct an environment map based on the rules in the specified policy. The
/// resulting map can be passed directly to `Command::envs()` after calling
/// `env_clear()` to ensure no unintended variables are leaked to the spawned
/// process.
///
/// The derivation follows the algorithm documented in the struct-level comment
/// for [`ShellEnvironmentPolicy`].
///
/// `CODEX_THREAD_ID` is injected when a thread id is provided, even when
/// `include_only` is set.
pub fn create_env(
    policy: &ShellEnvironmentPolicy,
    thread_id: Option<ThreadId>,
) -> HashMap<String, String> {
    let thread_id = thread_id.map(|thread_id| thread_id.to_string());
    shell_environment::create_env(policy, thread_id.as_deref())
}

/// Exposes the shared root-session identity and harness version to shell commands.
pub(crate) fn inject_session_env(env: &mut HashMap<String, String>, session_id: SessionId) {
    let (session_key, version_key) = if is_jaimesh_process() {
        ("JAIMESH_SESSION_ID", JAIMESH_VERSION_ENV_VAR)
    } else {
        (CODEX_SESSION_ID_ENV_VAR, CODEX_VERSION_ENV_VAR)
    };
    env.remove(CODEX_SESSION_ID_ENV_VAR);
    env.remove(CODEX_VERSION_ENV_VAR);
    env.insert(session_key.to_string(), session_id.to_string());
    if cfg!(windows) {
        env.retain(|key, _| !key.eq_ignore_ascii_case(version_key));
    }
    env.insert(
        version_key.to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
    );
}

/// Injects the selected named permission profile into a shell tool's environment.
///
/// This is applied after the shell environment policy so the runtime-selected
/// profile wins over inherited or configured values.
pub(crate) fn inject_permission_profile_env(
    env: &mut HashMap<String, String>,
    active_permission_profile: Option<&ActivePermissionProfile>,
) {
    let permission_key = if is_jaimesh_process() {
        JAIMESH_PERMISSION_PROFILE_ENV_VAR
    } else {
        CODEX_PERMISSION_PROFILE_ENV_VAR
    };
    if cfg!(windows) {
        env.retain(|key, _| !key.eq_ignore_ascii_case(CODEX_PERMISSION_PROFILE_ENV_VAR));
    } else {
        env.remove(CODEX_PERMISSION_PROFILE_ENV_VAR);
    }
    env.remove(JAIMESH_PERMISSION_PROFILE_ENV_VAR);
    if let Some(active_permission_profile) = active_permission_profile {
        env.insert(
            permission_key.to_string(),
            active_permission_profile.id.clone(),
        );
    }
}

/// Carries the configured apply-patch line-ending rollout state into child
/// processes.
///
/// Apply this after inherited or client-provided environment overrides so the
/// active feature configuration remains authoritative. The in-process
/// apply-patch path reads the feature directly.
pub fn inject_apply_patch_env(env: &mut HashMap<String, String>, features: &Features) {
    env.retain(|key, _| !key.eq_ignore_ascii_case(CODEX_APPLY_PATCH_PRESERVE_LINE_ENDINGS_ENV_VAR));
    env.retain(|key, _| {
        !key.eq_ignore_ascii_case(JAIMESH_APPLY_PATCH_PRESERVE_LINE_ENDINGS_ENV_VAR)
    });
    if features.enabled(Feature::ApplyPatchPreserveLineEndings) {
        let key = if is_jaimesh_process() {
            JAIMESH_APPLY_PATCH_PRESERVE_LINE_ENDINGS_ENV_VAR
        } else {
            CODEX_APPLY_PATCH_PRESERVE_LINE_ENDINGS_ENV_VAR
        };
        env.insert(key.to_string(), "1".to_string());
    }
}

#[cfg(all(test, target_os = "windows"))]
fn create_env_from_vars<I>(
    vars: I,
    policy: &ShellEnvironmentPolicy,
    thread_id: Option<ThreadId>,
) -> HashMap<String, String>
where
    I: IntoIterator<Item = (String, String)>,
{
    let thread_id = thread_id.map(|thread_id| thread_id.to_string());
    shell_environment::create_env_from_vars(vars, policy, thread_id.as_deref())
}

#[cfg(test)]
fn populate_env<I>(
    vars: I,
    policy: &ShellEnvironmentPolicy,
    thread_id: Option<ThreadId>,
) -> HashMap<String, String>
where
    I: IntoIterator<Item = (String, String)>,
{
    let thread_id = thread_id.map(|thread_id| thread_id.to_string());
    shell_environment::populate_env(vars, policy, thread_id.as_deref())
}

#[cfg(test)]
#[path = "exec_env_tests.rs"]
mod tests;
