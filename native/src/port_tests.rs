// SPDX-FileCopyrightText: 2026 Altifigence
// SPDX-License-Identifier: Apache-2.0
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Lease {
    root: PathBuf,
    environment: Environment,
    available: bool,
    wsl: Option<WslProject>,
}
impl Lease {
    fn fixture() -> Self {
        Self {
            root: std::env::current_dir().unwrap(),
            environment: Environment::OnPremises,
            available: true,
            wsl: None,
        }
    }
}
impl WorkspaceLease for Lease {
    fn environment(&self) -> Environment {
        self.environment
    }
    fn workspace_root(&self) -> Result<&Path, String> {
        Ok(&self.root)
    }
    fn project_launch_root(&self) -> Result<&Path, String> {
        self.available
            .then_some(self.root.as_path())
            .ok_or_else(|| "fixture project unavailable".into())
    }
    fn wsl_project(&self) -> Option<&WslProject> {
        self.wsl.as_ref()
    }
}
struct Store {
    document: Mutex<Value>,
    writes: AtomicUsize,
    ignore_write: bool,
}
impl Store {
    fn fixture() -> Self {
        Self {
            document: Mutex::new(json!({ "unrelated": "preserved", "extensions": {} })),
            writes: AtomicUsize::new(0),
            ignore_write: false,
        }
    }
}
impl InstallationSettings for Store {
    fn read_installation_settings(&self) -> Result<Value, String> {
        Ok(self.document.lock().unwrap().clone())
    }
    fn patch_installation_settings(&self, patch: Value) -> Result<(), String> {
        self.writes.fetch_add(1, Ordering::Relaxed);
        if !self.ignore_write {
            let mut document = self.document.lock().unwrap();
            let extensions = document["extensions"].as_object_mut().unwrap();
            for (key, value) in patch["extensions"].as_object().unwrap() {
                if value.is_null() {
                    extensions.remove(key);
                } else {
                    extensions.insert(key.clone(), value.clone());
                }
            }
        }
        Ok(())
    }
}

#[test]
fn installation_ports_preserve_other_settings_remove_registration_and_check_readback() {
    let lease = Lease::fixture();
    let store = Store::fixture();
    assert!(!set_vscode_integration("main", &lease, &store, None, false)
        .unwrap()
        .installed());
    assert!(set_vscode_integration("main", &lease, &store, None, true)
        .unwrap()
        .installed());
    assert!(!set_vscode_integration("main", &lease, &store, None, false)
        .unwrap()
        .installed());
    let document = store.read_installation_settings().unwrap();
    assert_eq!(document["unrelated"], "preserved");
    assert!(document["extensions"].get(ID).is_none());
    assert_eq!(store.writes.load(Ordering::Relaxed), 3);
    let mut racing = Store::fixture();
    racing.ignore_write = true;
    assert!(set_vscode_integration("main", &lease, &racing, None, true)
        .err()
        .unwrap()
        .contains("changed during update"));
}

#[test]
fn native_window_roles_authorize_before_touching_store_or_launching() {
    let lease = Lease::fixture();
    let store = Store::fixture();
    assert!(read_vscode_integration("settings", &lease, &store, None).is_ok());
    assert!(set_vscode_integration("settings", &lease, &store, None, true).is_err());
    assert!(open_vscode("settings", &lease, &store, None).is_err());
    assert!(read_vscode_integration("renderer-project", &lease, &store, None).is_err());
    assert_eq!(store.writes.load(Ordering::Relaxed), 0);
}

#[test]
fn status_prioritizes_cloud_and_unavailable_projects_without_a_probe() {
    let store = Store::fixture();
    let mut lease = Lease::fixture();
    lease.environment = Environment::Cloud;
    assert_eq!(
        status(&lease, &store, None).unwrap().launch(),
        "cloud-unsupported"
    );
    lease.environment = Environment::OnPremises;
    lease.available = false;
    assert_eq!(
        status(&lease, &store, None).unwrap().launch(),
        "project-unavailable"
    );
    lease.available = true;
    let wire = serde_json::to_value(status(&lease, &store, None).unwrap()).unwrap();
    assert_eq!(
        wire,
        json!({ "installed": false, "installationScope": "device", "applicationAvailable": false, "launch": "native" })
    );
}

#[test]
fn native_arguments_use_only_current_host_project_and_require_registration() {
    let lease = Lease::fixture();
    let store = Store::fixture();
    assert!(launch_args(&lease, &store, Path::new("unused")).is_err());
    store
        .patch_installation_settings(
            json!({ "extensions": { ID: { "installed": true, "version": VERSION } } }),
        )
        .unwrap();
    assert_eq!(
        launch_args(&lease, &store, Path::new("unused")).unwrap(),
        vec![
            std::ffi::OsString::from("--new-window"),
            lease.root.as_os_str().to_owned()
        ]
    );
    let mut cloud = lease;
    cloud.environment = Environment::Cloud;
    assert!(launch_args(&cloud, &store, Path::new("unused"))
        .unwrap_err()
        .contains("Cloud projects"));
}

#[test]
fn exact_wsl_identity_validation_rejects_aliases_and_shell_like_distribution_names() {
    assert!(WslProject::new(
        "DDS-Acceptance".into(),
        "/home/dds/My project.with.dot".into()
    )
    .is_ok());
    for distribution in ["", "-default", "Ubuntu;echo", "Ubuntu\nother", "has space"] {
        assert!(WslProject::new(distribution.into(), "/home/dds/project".into()).is_err());
    }
    for directory in [
        "/",
        "relative",
        "/home/../other",
        "/home/./project",
        "/home//project",
        "/home/project/",
        "/home\\project",
        "/home/project\n",
    ] {
        assert!(WslProject::new("DDS-Fixture".into(), directory.into()).is_err());
    }
    assert_eq!(
        WslProject::from_unc(r"\\wsl.localhost\DDS-Fixture\home\dds\project")
            .unwrap()
            .unwrap(),
        WslProject::new("DDS-Fixture".into(), "/home/dds/project".into()).unwrap()
    );
    assert!(WslProject::from_unc(r"\\unrelated-server\share\project")
        .unwrap()
        .is_none());
}

#[test]
fn exact_environment_filter_removes_sensitive_overrides_and_keeps_runtime_variables() {
    let mut command = Command::new("unused");
    command
        .env("NODE_OPTIONS", "--require=injected")
        .env("OPENAI_FIXTURE_KEY", "test-only")
        .env("ALTIF_FIXTURE_TOKEN", "test-only")
        .env("PATH", "fixture-path");
    sanitize_child_environment(&mut command);
    for key in ["NODE_OPTIONS", "OPENAI_FIXTURE_KEY", "ALTIF_FIXTURE_TOKEN"] {
        assert!(command
            .get_envs()
            .any(|(name, value)| name == key && value.is_none()));
    }
    assert!(
        command
            .get_envs()
            .any(|(name, value)| name == "PATH"
                && value == Some(std::ffi::OsStr::new("fixture-path")))
    );
    let mut child = Command::new(fixture_process_path());
    child
        .arg("environment")
        .env("OPENAI_FIXTURE_KEY", "fixture-only");
    sanitize_child_environment(&mut child);
    assert_eq!(
        transport::run_process(child, vec![], Duration::from_secs(2), 1024).unwrap(),
        b"removed\n"
    );
}

fn fixture_process_path() -> PathBuf {
    let test_executable = std::env::current_exe().unwrap();
    let filename = if cfg!(windows) {
        "fixture-process.exe"
    } else {
        "fixture-process"
    };
    let helper = test_executable
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(filename);
    assert!(
        helper.is_file(),
        "run cargo test --all-targets to build fixture-process"
    );
    helper
}

#[test]
fn executable_validation_rejects_relative_missing_and_non_file_paths() {
    let fixture = tempfile::tempdir().unwrap();
    assert!(validate_editor_executable(Path::new("relative-code")).is_err());
    assert!(validate_editor_executable(&fixture.path().join("missing-code")).is_err());
    assert!(validate_editor_executable(fixture.path()).is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let executable = fixture.path().join("code-fixture");
        fs::write(&executable, b"fixture bytes").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(validate_editor_executable(&executable).is_ok());
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(validate_editor_executable(&executable).is_err());
        let alias = fixture.path().join("code-alias");
        std::os::unix::fs::symlink(&executable, &alias).unwrap();
        assert!(validate_editor_executable(&alias).is_err());
    }
}

#[test]
fn process_launch_cooldown_uses_the_exact_dds_policy() {
    let now = Instant::now();
    assert!(acquire_launch_slot(now).is_ok());
    assert!(acquire_launch_slot(now + Duration::from_millis(999)).is_err());
    assert!(acquire_launch_slot(now + Duration::from_secs(1)).is_ok());
}

#[cfg(windows)]
#[test]
fn actual_wsl_probe_runs_only_a_bounded_fixture_with_the_official_cli_layout() {
    let fixture = tempfile::tempdir().unwrap();
    let executable = fixture.path().join("Code.exe");
    fs::copy(fixture_process_path(), &executable).unwrap();
    fs::create_dir(fixture.path().join("bin")).unwrap();
    fs::create_dir_all(fixture.path().join("resources/app/out")).unwrap();
    let wrapper = fixture.path().join("bin/code.cmd");
    fs::write(
        &wrapper,
        r#""%~dp0..\Code.exe" "%~dp0..\resources\app\out\cli.js" %*"#,
    )
    .unwrap();
    fs::write(
        fixture.path().join("resources/app/out/cli.js"),
        b"fixture only; not interpreted",
    )
    .unwrap();
    assert!(wsl_extension_available(&executable).unwrap());
    let store = Store::fixture();
    store
        .patch_installation_settings(
            json!({ "extensions": { ID: { "installed": true, "version": VERSION } } }),
        )
        .unwrap();
    let mut lease = Lease::fixture();
    lease.wsl =
        Some(WslProject::new("DDS-Fixture".into(), "/home/dds/project.with.dot".into()).unwrap());
    assert_eq!(
        launch_args(&lease, &store, &executable).unwrap(),
        [
            "--new-window",
            "--remote",
            "wsl+DDS-Fixture",
            "/home/dds/project.with.dot/"
        ]
        .map(std::ffi::OsString::from)
    );
    assert_eq!(
        status(&lease, &store, Some(&executable)).unwrap().launch(),
        "wsl"
    );
    let extension_mode = fixture.path().join("fixture-extension-mode");
    fs::write(&extension_mode, b"missing").unwrap();
    assert!(!wsl_extension_available(&executable).unwrap());
    assert_eq!(
        status(&lease, &store, Some(&executable)).unwrap().launch(),
        "wsl-extension-missing"
    );
    assert!(launch_args(&lease, &store, &executable)
        .unwrap_err()
        .contains("Install Microsoft's WSL extension"));
    fs::write(&extension_mode, b"invalid").unwrap();
    assert!(wsl_extension_available(&executable)
        .unwrap_err()
        .contains("invalid extension location"));
    fs::write(&extension_mode, b"oversize").unwrap();
    assert!(wsl_extension_available(&executable).is_err());
    fs::write(&wrapper, b"arbitrary batch commands").unwrap();
    assert!(wsl_extension_available(&executable).is_err());
    assert_eq!(
        status(&lease, &store, Some(&executable)).unwrap().launch(),
        "wsl-extension-unavailable"
    );
    fs::write(&wrapper, vec![b'x'; 16 * 1024 + 1]).unwrap();
    assert!(wsl_extension_available(&executable).is_err());
}
