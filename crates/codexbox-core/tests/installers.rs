use codexbox_core::install::{
    InstallOptions, SILENT_BINARY, app_bundle_names, build_macos_app_bundle,
    build_windows_entrypoint_plan, companion_binary_path_from_exe, default_install_root_strategy,
    shortcut_names,
};

#[test]
fn windows_entrypoint_plan_contains_silent_and_manager_entrypoints() {
    let options = InstallOptions {
        install_root: Some("C:/Users/A/Desktop".into()),
        launcher_path: Some("C:/Tools/codexbox.exe".into()),
        manager_path: Some("C:/Tools/codexbox-manager.exe".into()),
        remove_owned_data: false,
    };

    let plan = build_windows_entrypoint_plan(&options);

    assert!(plan.silent_shortcut.ends_with("CodexBox.lnk"));
    assert!(plan.manager_shortcut.ends_with("CodexBox 管理工具.lnk"));
    assert_eq!(plan.launcher_path, "C:/Tools/codexbox.exe");
    assert_eq!(plan.manager_path, "C:/Tools/codexbox-manager.exe");
    assert_eq!(plan.silent_icon_path, "C:/Tools/codexbox.exe");
    assert_eq!(
        plan.manager_icon_path,
        "C:/Tools/codexbox-manager.exe"
    );
    assert_eq!(plan.uninstall_key, "CodexBox");
    assert_eq!(plan.legacy_uninstall_key, "CodexBox");
}

#[test]
fn windows_entrypoint_plan_can_request_owned_data_removal_without_shell_script() {
    let options = InstallOptions {
        install_root: Some("C:/Users/A/Desktop".into()),
        launcher_path: None,
        manager_path: None,
        remove_owned_data: true,
    };

    let plan = build_windows_entrypoint_plan(&options);

    assert!(plan.silent_shortcut.ends_with("CodexBox.lnk"));
    assert!(plan.manager_shortcut.ends_with("CodexBox 管理工具.lnk"));
    assert!(plan.remove_owned_data);
}

#[test]
fn macos_bundle_metadata_contains_silent_and_manager_apps() {
    let options = InstallOptions {
        install_root: Some("/Applications".into()),
        launcher_path: Some("/opt/CodexBox/codex-plus-plus".into()),
        manager_path: Some("/opt/CodexBox/codexbox-manager".into()),
        remove_owned_data: false,
    };

    let silent = build_macos_app_bundle(&options, false);
    let manager = build_macos_app_bundle(&options, true);

    assert!(silent.app_path.ends_with("CodexBox.app"));
    assert!(manager.app_path.ends_with("CodexBox 管理工具.app"));
    assert!(silent.info_plist.contains("<string>CodexBox</string>"));
    assert!(
        manager
            .info_plist
            .contains("<string>CodexBox 管理工具</string>")
    );
    assert!(silent.launch_script.contains("codexbox"));
    assert!(manager.launch_script.contains("codexbox-manager"));
}

#[test]
fn installer_exports_expected_two_entrypoint_names() {
    assert_eq!(shortcut_names(), ("CodexBox.lnk", "CodexBox 管理工具.lnk"));
    assert_eq!(app_bundle_names(), ("CodexBox.app", "CodexBox 管理工具.app"));
}

#[test]
fn companion_binary_path_resolves_macos_silent_app_next_to_manager_app() {
    let manager_exe = std::path::Path::new(
        "/Applications/CodexBox 管理工具.app/Contents/MacOS/CodexPlusPlusManager",
    );

    let companion = companion_binary_path_from_exe(manager_exe, SILENT_BINARY);

    assert_eq!(
        companion,
        std::path::PathBuf::from("/Applications/CodexBox.app/Contents/MacOS/CodexPlusPlus")
    );
    assert_ne!(
        companion,
        std::path::PathBuf::from(
            "/Applications/CodexBox 管理工具.app/Contents/MacOS/codex-plus-plus"
        )
    );
}

#[test]
fn windows_default_install_root_uses_known_folder_before_userprofile_desktop() {
    let strategy = default_install_root_strategy();

    if cfg!(windows) {
        assert_eq!(strategy, "windows-known-folder");
    } else if cfg!(target_os = "macos") {
        assert_eq!(strategy, "macos-applications");
    } else {
        assert_eq!(strategy, "user-dirs-desktop");
    }
}
