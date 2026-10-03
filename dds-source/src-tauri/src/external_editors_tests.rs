// SPDX-FileCopyrightText: 2026 Altifigence
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn settings_can_detect_editors_but_only_main_can_launch() {
    assert!(authorize_editor_window("main", false).is_ok());
    assert!(authorize_editor_window("main", true).is_ok());
    assert!(authorize_editor_window("settings", false).is_ok());
    assert!(authorize_editor_window("settings", true).is_err());
    for label in ["project-settings", "editor", "project-window", "unknown"] {
        assert!(authorize_editor_window(label, false).is_err());
        assert!(authorize_editor_window(label, true).is_err());
    }
}

#[cfg(windows)]
#[test]
fn windows_user_install_candidates_use_only_known_folder_catalog_paths() {
    let local_data = Path::new(r"C:\Users\Fixture User\AppData\Local");
    let code = editor_candidates(ExternalEditorId::VisualStudioCode, Some(local_data));
    assert_eq!(
        code[0],
        local_data.join(r"Programs\Microsoft VS Code\Code.exe")
    );
    let codium = editor_candidates(ExternalEditorId::Vscodium, Some(local_data));
    assert_eq!(
        codium[0],
        local_data.join(r"Programs\VSCodium\VSCodium.exe")
    );
    assert_eq!(
        editor_candidates(ExternalEditorId::EclipseLsp4e, Some(local_data)),
        editor_candidates(ExternalEditorId::EclipseLsp4e, None),
    );
    assert_eq!(
        editor_candidates(
            ExternalEditorId::VisualStudioCode,
            Some(Path::new("relative"))
        ),
        editor_candidates(ExternalEditorId::VisualStudioCode, None),
    );
}
