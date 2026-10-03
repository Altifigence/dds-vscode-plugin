// Altifigence — reusable AI accelerator project.
// SPDX-FileCopyrightText: 2026 Altifigence
// SPDX-License-Identifier: Apache-2.0

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use serde::Serialize;
const LAUNCH_COOLDOWN: Duration = Duration::from_secs(1);
static LAST_LAUNCH: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExternalEditorId {
    VisualStudioCode,
    Vscodium,
    EclipseLsp4e,
}

impl ExternalEditorId {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "visual_studio_code" => Ok(Self::VisualStudioCode),
            "vscodium" => Ok(Self::Vscodium),
            "eclipse_lsp4e" => Ok(Self::EclipseLsp4e),
            _ => Err("external editor is not supported".into()),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::VisualStudioCode => "visual_studio_code",
            Self::Vscodium => "vscodium",
            Self::EclipseLsp4e => "eclipse_lsp4e",
        }
    }

    fn launch_args(self, workspace: &Path) -> Vec<std::ffi::OsString> {
        match self {
            Self::VisualStudioCode | Self::Vscodium => {
                vec!["--reuse-window".into(), workspace.as_os_str().to_owned()]
            }
            Self::EclipseLsp4e => {
                vec!["-data".into(), workspace.as_os_str().to_owned()]
            }
        }
    }
}

const EDITOR_IDS: [ExternalEditorId; 3] = [
    ExternalEditorId::VisualStudioCode,
    ExternalEditorId::Vscodium,
    ExternalEditorId::EclipseLsp4e,
];

#[derive(Debug, Serialize)]
pub struct ExternalEditorStatus {
    id: &'static str,
    available: bool,
}

#[cfg(target_os = "linux")]
fn candidates(editor: ExternalEditorId) -> &'static [&'static str] {
    match editor {
        ExternalEditorId::VisualStudioCode => &["/usr/bin/code", "/usr/local/bin/code"],
        ExternalEditorId::Vscodium => &["/usr/bin/codium", "/usr/local/bin/codium"],
        ExternalEditorId::EclipseLsp4e => &["/usr/bin/eclipse", "/usr/local/bin/eclipse"],
    }
}

#[cfg(target_os = "macos")]
fn candidates(editor: ExternalEditorId) -> &'static [&'static str] {
    match editor {
        ExternalEditorId::VisualStudioCode => {
            &["/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code"]
        }
        ExternalEditorId::Vscodium => {
            &["/Applications/VSCodium.app/Contents/Resources/app/bin/codium"]
        }
        ExternalEditorId::EclipseLsp4e => &["/Applications/Eclipse.app/Contents/MacOS/eclipse"],
    }
}

#[cfg(target_os = "windows")]
fn candidates(editor: ExternalEditorId) -> &'static [&'static str] {
    match editor {
        ExternalEditorId::VisualStudioCode => &[
            r"C:\Program Files\Microsoft VS Code\Code.exe",
            r"C:\Program Files (x86)\Microsoft VS Code\Code.exe",
        ],
        ExternalEditorId::Vscodium => &[
            r"C:\Program Files\VSCodium\VSCodium.exe",
            r"C:\Program Files (x86)\VSCodium\VSCodium.exe",
        ],
        ExternalEditorId::EclipseLsp4e => {
            &[r"C:\Program Files\Eclipse Adoptium\eclipse\eclipse.exe"]
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn candidates(_editor: ExternalEditorId) -> &'static [&'static str] {
    &[]
}

fn editor_candidates(editor: ExternalEditorId, local_data: Option<&Path>) -> Vec<PathBuf> {
    let paths = candidates(editor).iter().map(PathBuf::from);
    #[cfg(windows)]
    {
        let user_install =
            local_data
                .filter(|root| root.is_absolute())
                .and_then(|root| match editor {
                    ExternalEditorId::VisualStudioCode => {
                        Some(root.join(r"Programs\Microsoft VS Code\Code.exe"))
                    }
                    ExternalEditorId::Vscodium => {
                        Some(root.join(r"Programs\VSCodium\VSCodium.exe"))
                    }
                    ExternalEditorId::EclipseLsp4e => None,
                });
        user_install.into_iter().chain(paths).collect()
    }
    #[cfg(not(windows))]
    {
        let _ = local_data;
        paths.collect()
    }
}

fn resolve_editor(editor: ExternalEditorId, local_data: Option<&Path>) -> Option<PathBuf> {
    editor_candidates(editor, local_data)
        .into_iter()
        .find(|path| validate_editor_executable(path).is_ok())
}
fn authorize_editor_window(label: &str, launch: bool) -> Result<(), String> {
    match (label, launch) {
        ("main", _) | ("settings", false) => Ok(()),
        _ => Err("this window may not perform that external editor operation".into()),
    }
}

fn validate_editor_executable(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("external editor path is not absolute".into());
    }
    let metadata =
        fs::symlink_metadata(path).map_err(|_| "external editor is not installed".to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("external editor must be a regular file".into());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let mode = metadata.permissions().mode();
        if mode & 0o111 == 0 {
            return Err("external editor is not executable".into());
        }
        if mode & 0o022 != 0 {
            return Err("external editor is writable by another account".into());
        }
        let owner = metadata.uid();
        // SAFETY: geteuid has no preconditions and only reads process identity.
        let current = unsafe { libc::geteuid() };
        if owner != 0 && owner != current {
            return Err("external editor owner is not trusted".into());
        }
    }

    Ok(())
}

fn acquire_launch_slot(now: Instant) -> Result<(), String> {
    let mut guard = LAST_LAUNCH
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| "external editor launch state is unavailable".to_string())?;
    if guard.is_some_and(|previous| now.duration_since(previous) < LAUNCH_COOLDOWN) {
        return Err("external editor launch is temporarily rate limited".into());
    }
    *guard = Some(now);
    Ok(())
}
fn editor_statuses(local_data: Option<&Path>) -> Vec<ExternalEditorStatus> {
    EDITOR_IDS
        .into_iter()
        .map(|editor| ExternalEditorStatus {
            id: editor.as_str(),
            available: resolve_editor(editor, local_data).is_some(),
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_catalog_editor_ids() {
        for editor in EDITOR_IDS {
            assert_eq!(ExternalEditorId::parse(editor.as_str()), Ok(editor));
        }
        assert!(ExternalEditorId::parse("custom").is_err());
        assert!(ExternalEditorId::parse("/usr/bin/code").is_err());
    }

    #[test]
    fn launch_arguments_are_fixed_by_profile() {
        let root = Path::new("/workspace");
        assert_eq!(
            ExternalEditorId::VisualStudioCode.launch_args(root),
            vec![std::ffi::OsString::from("--reuse-window"), root.into()]
        );
        assert_eq!(
            ExternalEditorId::EclipseLsp4e.launch_args(root),
            vec![std::ffi::OsString::from("-data"), root.into()]
        );
    }

    #[test]
    fn status_never_contains_an_executable_path() {
        let serialized = serde_json::to_value(editor_statuses(None)).expect("serialize status");
        for entry in serialized.as_array().expect("status array") {
            let object = entry.as_object().expect("status object");
            assert_eq!(object.len(), 2);
            assert!(object.contains_key("id"));
            assert!(object.contains_key("available"));
        }
    }
}
