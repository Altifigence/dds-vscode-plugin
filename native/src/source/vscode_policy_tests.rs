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
