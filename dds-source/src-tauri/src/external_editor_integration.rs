//! Registration of the bundled DDS integration is independent of VS Code's
//! installation and of Microsoft's separately installed Remote WSL extension.
use super::*;
use crate::native_console::{Environment, WorkspaceLease};
use serde_json::{json, Value};

const ID: &str = "altifigence.vscode";
const VERSION: &str = "1.0.0";

fn registered(settings: &Value) -> bool {
    let entry = &settings["extensions"][ID];
    entry["installed"] == true && entry["version"] == VERSION
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationStatus {
    installed: bool,
    installation_scope: &'static str,
    application_available: bool,
    launch: &'static str,
}

fn status(state: &WorkspaceLease, local_data: Option<&Path>) -> Result<IntegrationStatus, String> {
    let executable = resolve_editor(ExternalEditorId::VisualStudioCode, local_data);
    let launch = if state.environment() == Environment::Cloud {
        "cloud-unsupported"
    } else if state.project_launch_root().is_err() {
        "project-unavailable"
    } else {
        #[cfg(windows)]
        if state.wsl().is_some() {
            return Ok(IntegrationStatus {
                installed: registered(&crate::native_workspace::read_installation_settings()?),
                installation_scope: "device",
                application_available: executable.is_some(),
                launch: match executable.as_deref().map(wsl_extension_available) {
                    Some(Ok(true)) | None => "wsl",
                    Some(Ok(false)) => "wsl-extension-missing",
                    Some(Err(_)) => "wsl-extension-unavailable",
                },
            });
        }
        "native"
    };
    Ok(IntegrationStatus {
        installed: registered(&crate::native_workspace::read_installation_settings()?),
        installation_scope: "device",
        application_available: executable.is_some(),
        launch,
    })
}

#[tauri::command]
pub async fn read_vscode_integration(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: WorkspaceLease,
) -> Result<IntegrationStatus, String> {
    authorize_editor_window(window.label(), false)?;
    let local_data = local_data_directory(&app);
    tauri::async_runtime::spawn_blocking(move || status(&state, local_data.as_deref()))
        .await
        .map_err(|_| "VS Code integration status is unavailable.".to_owned())?
}

#[tauri::command]
pub async fn set_vscode_integration(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: WorkspaceLease,
    installed: bool,
) -> Result<IntegrationStatus, String> {
    authorize_editor_window(window.label(), true)?;
    let local_data = local_data_directory(&app);
    tauri::async_runtime::spawn_blocking(move || {
        crate::native_workspace::patch_installation_settings(json!({ "extensions": {
            ID: if installed { json!({ "version": VERSION, "installed": true }) } else { Value::Null }
        }}))?;
        let current = status(&state, local_data.as_deref())?;
        if current.installed != installed {
            return Err("VS Code integration changed during update. Refresh its status.".into());
        }
        Ok(current)
    }).await.map_err(|_| "VS Code integration update could not complete.".to_owned())?
}

pub(super) fn launch_args(
    state: &WorkspaceLease,
    executable: &Path,
) -> Result<Vec<std::ffi::OsString>, String> {
    if !registered(&crate::native_workspace::read_installation_settings()?) {
        return Err(
            "Install the DDS VS Code integration in Plugins before opening this project.".into(),
        );
    }
    if state.environment() != Environment::OnPremises {
        return Err("Opening Cloud projects in VS Code is not supported by this server.".into());
    }
    #[cfg(windows)]
    if let Some(wsl) = state.wsl() {
        if !wsl_extension_available(executable)? {
            return Err(
                "Install Microsoft's WSL extension in VS Code before opening this project.".into(),
            );
        }
        let project = wsl.connection.project();
        return Ok(wsl_launch_args(&project.distribution, &project.directory));
    }
    let _ = executable;
    Ok(vec![
        "--new-window".into(),
        state.workspace_root()?.as_os_str().to_owned(),
    ])
}

#[cfg(any(windows, test))]
fn wsl_launch_args(distribution: &str, directory: &str) -> Vec<std::ffi::OsString> {
    // Native WslProject already validated both identities. The trailing slash
    // forces a folder even when its last component contains a dot. No shell,
    // default distribution, UNC Windows workspace or renderer path is used.
    vec![
        "--new-window".into(),
        "--remote".into(),
        format!("wsl+{distribution}").into(),
        format!("{}/", directory.trim_end_matches('/')).into(),
    ]
}

#[cfg(windows)]
fn wsl_extension_available(executable: &Path) -> Result<bool, String> {
    let root = executable
        .parent()
        .ok_or("VS Code installation is unavailable.")?;
    let wrapper = root.join("bin/code.cmd");
    let metadata = fs::symlink_metadata(&wrapper).map_err(|_| "VS Code CLI is unavailable.")?;
    use std::os::windows::fs::MetadataExt;
    if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 || metadata.len() > 16 * 1024
    {
        return Err("VS Code CLI is not a bounded regular file.".into());
    }
    use std::io::Read;
    let mut bytes = Vec::new();
    fs::File::open(&wrapper)
        .and_then(|file| file.take(16 * 1024 + 1).read_to_end(&mut bytes))
        .map_err(|_| "VS Code CLI could not be inspected.")?;
    if bytes.len() > 16 * 1024 {
        return Err("VS Code CLI exceeds its byte limit.".into());
    }
    let contents = std::str::from_utf8(&bytes).map_err(|_| "VS Code CLI is not valid text.")?;
    let relative = cli_script_path(&contents).ok_or("This VS Code CLI layout is not supported.")?;
    let script = root.join(relative);
    let script_metadata =
        fs::symlink_metadata(&script).map_err(|_| "VS Code CLI script is unavailable.")?;
    if !script_metadata.is_file() || script_metadata.file_attributes() & 0x400 != 0 {
        return Err("VS Code CLI script is not a regular file.".into());
    }
    let mut command = Command::new(executable);
    command
        .arg(script)
        .args(["--locate-extension", "ms-vscode-remote.remote-wsl"])
        .current_dir(root);
    sanitize_child_environment(&mut command);
    // This is the installed VS Code CLI's fixed bootstrap contract, inspected
    // above. We never execute the batch file or a renderer-supplied script.
    command
        .env("ELECTRON_RUN_AS_NODE", "1")
        .env_remove("VSCODE_DEV");
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0800_0000);
    let output = desktop_wsl_backend::transport::run_process(
        command,
        vec![],
        Duration::from_secs(10),
        16 * 1024,
    )
    .map_err(|_| {
        "The VS Code WSL extension could not be checked. Refresh and try again.".to_owned()
    })?;
    let output = std::str::from_utf8(&output)
        .map_err(|_| "VS Code CLI returned an invalid result.")?
        .trim();
    if output.is_empty() {
        return Ok(false);
    }
    if output.contains(['\r', '\n', '\0']) || !Path::new(output).is_absolute() {
        return Err("VS Code CLI returned an invalid extension location.".into());
    }
    Ok(true)
}

#[cfg(any(windows, test))]
fn cli_script_path(wrapper: &str) -> Option<PathBuf> {
    // Both stable layouts are emitted by the official Windows installer. New
    // layouts fail closed; arbitrary batch commands and path escapes are not
    // interpreted. A versioned install uses a hexadecimal update directory.
    for line in wrapper.lines().map(str::trim) {
        let Some(tail) = line.strip_prefix(r#""%~dp0..\Code.exe" "%~dp0..\"#) else {
            continue;
        };
        let path = tail.strip_suffix(r#"" %*"#)?;
        if path == r"resources\app\out\cli.js" {
            return Some(PathBuf::from(path));
        }
        let version = path.strip_suffix(r"\resources\app\out\cli.js")?;
        if (8..=40).contains(&version.len()) && version.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Some(PathBuf::from(path));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    #[ignore = "requires the user's installed VS Code and Microsoft WSL extension; read-only probe"]
    fn actual_installed_vscode_wsl_extension_is_detected_without_a_shell() {
        let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap();
        let executable = resolve_editor(ExternalEditorId::VisualStudioCode, Some(&local)).unwrap();
        assert!(wsl_extension_available(&executable).unwrap());
    }
    #[test]
    fn registration_requires_the_bundled_version_and_explicit_install() {
        assert!(!registered(
            &json!({ "developerTools": { "externalEditor": "visual_studio_code" } })
        ));
        assert!(registered(
            &json!({ "extensions": { ID: { "version": VERSION, "installed": true } } })
        ));
        assert!(!registered(&json!({ "extensions": { ID: null } })));
        assert!(!registered(
            &json!({ "extensions": { ID: { "version": "other", "installed": true } } })
        ));
    }
    #[test]
    fn remote_launch_preserves_exact_distribution_folder_and_other_windows() {
        assert_eq!(
            wsl_launch_args("DDS-Acceptance", "/home/dds/My project.with.dot"),
            vec![
                "--new-window",
                "--remote",
                "wsl+DDS-Acceptance",
                "/home/dds/My project.with.dot/"
            ]
            .into_iter()
            .map(std::ffi::OsString::from)
            .collect::<Vec<_>>()
        );
    }
    #[test]
    fn windows_cli_layout_is_closed_and_never_interprets_batch_text() {
        assert_eq!(cli_script_path("@echo off\r\n\"%~dp0..\\Code.exe\" \"%~dp0..\\7debcd0e2a\\resources\\app\\out\\cli.js\" %*"),
            Some(PathBuf::from(r"7debcd0e2a\resources\app\out\cli.js")));
        assert_eq!(
            cli_script_path(r#""%~dp0..\Code.exe" "%~dp0..\resources\app\out\cli.js" %*"#),
            Some(PathBuf::from(r"resources\app\out\cli.js"))
        );
        for invalid in [
            r#""%~dp0..\Code.exe" "%~dp0..\..\resources\app\out\cli.js" %*"#,
            r#""%~dp0..\Code.exe" "%~dp0..\12345678\resources\app\out\cli.js" %* & arbitrary"#,
            r#""%~dp0..\Other.exe" "%~dp0..\resources\app\out\cli.js" %*"#,
        ] {
            assert!(cli_script_path(invalid).is_none());
        }
    }
}
