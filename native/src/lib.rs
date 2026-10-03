// SPDX-FileCopyrightText: 2026 Altifigence
// SPDX-License-Identifier: Apache-2.0
//! DDS VS Code policy compiled from the source inventory, with public host ports.
//! Embedders own project authorization and durable device settings. Blocking
//! APIs belong on the host's worker thread; they are not renderer commands.
// Retain the inherited CLI probe's `&contents` expression byte-for-byte.
#![allow(clippy::needless_borrow)]

mod environment;
pub mod transport;
pub use transport::WslProject;

// Preserve the real probe's qualified process call without private engine deps.
#[cfg(windows)]
mod desktop_wsl_backend {
    pub(crate) mod transport {
        pub(crate) use crate::transport::run_process;
    }
}
use environment::sanitize_child_environment;
use serde_json::{json, Value};

include!("source/editor_policy.rs");
include!("source/vscode_policy.rs");
include!("source/validate_project.rs");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    OnPremises,
    Cloud,
}

/// The native host supplies its current immutable, authorized project.
/// Paths must come from host project selection, never from renderer arguments.
pub trait WorkspaceLease {
    fn environment(&self) -> Environment;
    fn workspace_root(&self) -> Result<&Path, String>;
    fn project_launch_root(&self) -> Result<&Path, String>;
    fn wsl_project(&self) -> Option<&WslProject> {
        None
    }
}

/// Device-wide installation authority. Implement atomic merge-patch writes,
/// preserving unrelated settings; JSON null removes an extension entry.
pub trait InstallationSettings {
    fn read_installation_settings(&self) -> Result<Value, String>;
    fn patch_installation_settings(&self, patch: Value) -> Result<(), String>;
}

impl IntegrationStatus {
    pub fn installed(&self) -> bool {
        self.installed
    }
    pub fn application_available(&self) -> bool {
        self.application_available
    }
    pub fn launch(&self) -> &str {
        self.launch
    }
}

/// Public equivalent of the DDS status command, with host ports substituted.
pub fn read_vscode_integration(
    window_label: &str,
    state: &impl WorkspaceLease,
    settings: &impl InstallationSettings,
    local_data: Option<&Path>,
) -> Result<IntegrationStatus, String> {
    authorize_editor_window(window_label, false)?;
    let executable = resolve_editor(ExternalEditorId::VisualStudioCode, local_data);
    status(state, settings, executable.as_deref())
}

fn status(
    state: &impl WorkspaceLease,
    settings: &impl InstallationSettings,
    executable: Option<&Path>,
) -> Result<IntegrationStatus, String> {
    let launch = if state.environment() == Environment::Cloud {
        "cloud-unsupported"
    } else if state.project_launch_root().is_err() {
        "project-unavailable"
    } else {
        #[cfg(windows)]
        if state.wsl_project().is_some() {
            return Ok(IntegrationStatus {
                installed: registered(&settings.read_installation_settings()?),
                installation_scope: "device",
                application_available: executable.is_some(),
                launch: match executable.map(wsl_extension_available) {
                    Some(Ok(true)) | None => "wsl",
                    Some(Ok(false)) => "wsl-extension-missing",
                    Some(Err(_)) => "wsl-extension-unavailable",
                },
            });
        }
        "native"
    };
    Ok(IntegrationStatus {
        installed: registered(&settings.read_installation_settings()?),
        installation_scope: "device",
        application_available: executable.is_some(),
        launch,
    })
}

/// Registration changes DDS's own setting; it downloads no application or VSIX.
pub fn set_vscode_integration(
    window_label: &str,
    state: &impl WorkspaceLease,
    settings: &impl InstallationSettings,
    local_data: Option<&Path>,
    installed: bool,
) -> Result<IntegrationStatus, String> {
    authorize_editor_window(window_label, true)?;
    settings.patch_installation_settings(json!({ "extensions": {
        ID: if installed { json!({ "version": VERSION, "installed": true }) } else { Value::Null }
    }}))?;
    let current = read_vscode_integration(window_label, state, settings, local_data)?;
    if current.installed != installed {
        return Err("VS Code integration changed during update. Refresh its status.".into());
    }
    Ok(current)
}

fn launch_args(
    state: &impl WorkspaceLease,
    settings: &impl InstallationSettings,
    executable: &Path,
) -> Result<Vec<std::ffi::OsString>, String> {
    if !registered(&settings.read_installation_settings()?) {
        return Err(
            "Install the DDS VS Code integration in Plugins before opening this project.".into(),
        );
    }
    if state.environment() != Environment::OnPremises {
        return Err("Opening Cloud projects in VS Code is not supported by this server.".into());
    }
    #[cfg(windows)]
    if let Some(project) = state.wsl_project() {
        // Hosts retain project ownership; reject malformed port implementations.
        WslProject::new(project.distribution.clone(), project.directory.clone())?;
        if !wsl_extension_available(executable)? {
            return Err(
                "Install Microsoft's WSL extension in VS Code before opening this project.".into(),
            );
        }
        return Ok(wsl_launch_args(&project.distribution, &project.directory));
    }
    let _ = executable;
    Ok(vec![
        "--new-window".into(),
        state.workspace_root()?.as_os_str().to_owned(),
    ])
}

/// Starts only the discovered VS Code installation, using the host's project.
/// Invoke this only after an explicit user's Open action on the main window.
pub fn open_vscode(
    window_label: &str,
    state: &impl WorkspaceLease,
    settings: &impl InstallationSettings,
    local_data: Option<&Path>,
) -> Result<bool, String> {
    authorize_editor_window(window_label, true)?;
    let executable = resolve_editor(ExternalEditorId::VisualStudioCode, local_data)
        .ok_or_else(|| "external editor is not installed or trusted".to_string())?;
    acquire_launch_slot(Instant::now())?;
    if state.environment() != Environment::OnPremises {
        return Err(
            "Opening Cloud projects in an external editor is not supported by this server.".into(),
        );
    }
    let args = launch_args(state, settings, &executable)?;
    let workspace = state.project_launch_root()?;
    let mut command = Command::new(executable);
    command
        .args(args)
        .current_dir(workspace)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    sanitize_child_environment(&mut command);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    command
        .spawn()
        .map_err(|_| "external editor could not be started".to_string())?;
    Ok(true)
}

/// Shared discovery from DDS; paths are intentionally absent from wire output.
pub fn list_external_editors(
    window_label: &str,
    local_data: Option<&Path>,
) -> Result<Vec<ExternalEditorStatus>, String> {
    authorize_editor_window(window_label, false)?;
    Ok(editor_statuses(local_data))
}

/// Fixed argument profiles retained from DDS's shared editor discovery module.
/// This describes arguments only and does not authorize or launch a process.
/// VS Code's registered integration uses `--new-window` through `open_vscode`.
pub fn shared_editor_argument_profile(
    id: &str,
    workspace: &Path,
) -> Result<Vec<std::ffi::OsString>, String> {
    Ok(ExternalEditorId::parse(id)?.launch_args(workspace))
}

// Compile the exact original boundary and policy tests without Tauri.
#[cfg(test)]
#[path = "../../dds-source/src-tauri/src/external_editors_tests.rs"]
mod boundary_tests;
#[cfg(test)]
mod source_integration_tests {
    use super::*;
    include!("source/vscode_policy_tests.rs");
}
#[cfg(test)]
mod port_tests;
