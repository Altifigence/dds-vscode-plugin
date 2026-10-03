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
