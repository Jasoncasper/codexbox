use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use codexbox_core::install::SILENT_BINARY;
use codexbox_core::script_market::{self, MarketScript, ScriptMarketManifest};
use codexbox_core::settings::{BackendSettings, SettingsStore};
use codexbox_core::status::{LaunchStatus, StatusStore};
use codexbox_core::user_scripts::UserScriptManager;
use serde::Serialize;
use serde_json::{Value, json};

use crate::install::{self, InstallActionResult, InstallOptions};

#[derive(Debug, Clone, Serialize)]
pub struct CommandResult<T>
where
    T: Serialize,
{
    pub status: String,
    pub message: String,
    #[serde(flatten)]
    pub payload: T,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionPayload {
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PathState {
    pub status: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverviewPayload {
    pub codex_app: PathState,
    pub codex_version: Option<String>,
    pub silent_shortcut: PathState,
    pub management_shortcut: PathState,
    pub latest_launch: Option<LaunchStatus>,
    pub current_version: String,
    pub update_status: String,
    pub settings_path: String,
    pub logs_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingsPayload {
    pub settings: BackendSettings,
    pub settings_path: String,
    pub user_scripts: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextEntriesPayload {
    pub settings: BackendSettings,
    pub entries: codexbox_core::relay_config::CodexContextEntries,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveContextEntriesPayload {
    pub entries: codexbox_core::relay_config::CodexContextEntries,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayProfileTestPayload {
    pub http_status: u16,
    pub endpoint: String,
    pub response_preview: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextSettingsRequest {
    pub settings: BackendSettings,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextEntryRequest {
    pub settings: BackendSettings,
    pub kind: String,
    pub id: String,
    pub toml_body: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextDeleteRequest {
    pub settings: BackendSettings,
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequest {
    #[serde(default)]
    pub app_path: String,
    #[serde(default = "default_debug_port")]
    pub debug_port: u16,
    #[serde(default = "default_helper_port")]
    pub helper_port: u16,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRequest {
    #[serde(default = "default_log_lines")]
    pub lines: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogsPayload {
    pub path: String,
    pub text: String,
    pub lines: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticsPayload {
    pub report: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WatcherPayload {
    pub enabled: bool,
    pub disabled_flag: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdsPayload {
    pub version: u64,
    pub ads: Vec<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScriptMarketPayload {
    pub market: Value,
    pub user_scripts: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupPayload {
    pub show_update: bool,
}

#[tauri::command]
pub fn backend_version() -> CommandResult<VersionPayload> {
    ok(
        "后端版本已读取。",
        VersionPayload {
            version: codexbox_core::version::VERSION.to_string(),
        },
    )
}

#[tauri::command]
pub fn startup_options() -> CommandResult<StartupPayload> {
    ok(
        "启动参数已读取。",
        StartupPayload {
            show_update: startup_should_show_update(),
        },
    )
}

pub fn startup_should_show_update() -> bool {
    should_show_update(
        std::env::args(),
        std::env::var("CODEX_PLUS_SHOW_UPDATE").ok().as_deref(),
    )
}

fn should_show_update<I, S>(args: I, env_value: Option<&str>) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter().any(|arg| arg.as_ref() == "--show-update") || env_value == Some("1")
}

#[tauri::command]
pub async fn load_overview() -> CommandResult<OverviewPayload> {
    let payload = tauri::async_runtime::spawn_blocking(load_overview_payload).await;
    let Ok((codex_app_path, entrypoints, latest_launch)) = payload else {
        return failed(
            "概览后台任务失败。",
            OverviewPayload {
                codex_app: path_state(None),
                codex_version: None,
                silent_shortcut: path_state(None),
                management_shortcut: path_state(None),
                latest_launch: None,
                current_version: codexbox_core::version::VERSION.to_string(),
                update_status: "not_checked".to_string(),
                settings_path: codexbox_core::paths::default_settings_path()
                    .to_string_lossy()
                    .to_string(),
                logs_path: codexbox_core::paths::default_diagnostic_log_path()
                    .to_string_lossy()
                    .to_string(),
            },
        );
    };
    ok(
        "概览已加载。",
        OverviewPayload {
            codex_version: codex_app_path
                .as_deref()
                .and_then(codexbox_core::app_paths::codex_app_version),
            codex_app: path_state(codex_app_path),
            silent_shortcut: shortcut_state(entrypoints.silent_shortcut),
            management_shortcut: shortcut_state(entrypoints.management_shortcut),
            latest_launch,
            current_version: codexbox_core::version::VERSION.to_string(),
            update_status: "not_checked".to_string(),
            settings_path: codexbox_core::paths::default_settings_path()
                .to_string_lossy()
                .to_string(),
            logs_path: codexbox_core::paths::default_diagnostic_log_path()
                .to_string_lossy()
                .to_string(),
        },
    )
}

#[tauri::command]
pub fn launch_codex_plus(request: LaunchRequest) -> CommandResult<Value> {
    spawn_codex_plus_launch(request, "启动任务已在后台开始，可稍后查看概览状态。")
}

#[tauri::command]
pub fn restart_codex_plus(request: LaunchRequest) -> CommandResult<Value> {
    codexbox_core::watcher::stop_launcher_processes();
    codexbox_core::watcher::stop_codex_processes();
    spawn_codex_plus_launch(request, "Codex 已请求重启，启动任务正在后台运行。")
}

fn spawn_codex_plus_launch(request: LaunchRequest, accepted_message: &str) -> CommandResult<Value> {
    let debug_port = request.debug_port;
    let helper_port = request.helper_port;
    let _ = codexbox_core::diagnostic_log::append_diagnostic_log(
        "manager.launch_requested",
        json!({
            "debug_port": debug_port,
            "helper_port": helper_port,
            "app_path": request.app_path.trim()
        }),
    );
    match spawn_silent_launcher(&request) {
        Ok(()) => CommandResult {
            status: "accepted".to_string(),
            message: accepted_message.to_string(),
            payload: json!({
                "debugPort": debug_port,
                "helperPort": helper_port
            }),
        },
        Err(error) => failed(
            &format!("启动静默入口失败：{error}"),
            json!({
                "debugPort": debug_port,
                "helperPort": helper_port
            }),
        ),
    }
}

fn spawn_silent_launcher(request: &LaunchRequest) -> anyhow::Result<()> {
    let launcher = codexbox_core::install::companion_binary_path(SILENT_BINARY);
    let mut command = std::process::Command::new(&launcher);
    if !request.app_path.trim().is_empty() {
        command.arg("--app-path").arg(request.app_path.trim());
    }
    command
        .arg("--debug-port")
        .arg(request.debug_port.to_string())
        .arg("--helper-port")
        .arg(request.helper_port.to_string());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| anyhow::anyhow!("无法启动 {}：{error}", launcher.to_string_lossy()))
}

#[tauri::command]
pub fn load_settings() -> CommandResult<SettingsPayload> {
    settings_payload("设置已加载。", "设置读取失败")
}

#[tauri::command]
pub fn save_settings(mut settings: BackendSettings) -> CommandResult<SettingsPayload> {
    if let Some(path) =
        codexbox_core::app_paths::normalize_codex_app_path(Path::new(&settings.codex_app_path))
    {
        settings.codex_app_path = path.to_string_lossy().to_string();
    }
    match SettingsStore::default().save(&settings) {
        Ok(()) => settings_payload("设置已保存。", "设置保存后重新读取失败"),
        Err(error) => failed(
            &format!("保存设置失败：{error}"),
            SettingsPayload {
                settings,
                settings_path: codexbox_core::paths::default_settings_path()
                    .to_string_lossy()
                    .to_string(),
                user_scripts: user_script_inventory(),
            },
        ),
    }
}

#[tauri::command]
pub async fn load_ads() -> CommandResult<AdsPayload> {
    match codexbox_core::ads::fetch_ad_list().await {
        Ok(payload) => ok("推荐内容已加载。", ads_payload(payload)),
        Err(error) => failed(
            &format!("推荐内容加载失败：{error}"),
            AdsPayload {
                version: 1,
                ads: Vec::new(),
            },
        ),
    }
}

#[tauri::command]
pub async fn refresh_script_market() -> CommandResult<ScriptMarketPayload> {
    match script_market::fetch_market_manifest(script_market::DEFAULT_MARKET_INDEX_URL).await {
        Ok(manifest) => ok(
            "脚本市场已刷新。",
            script_market_payload_from_manifest(&manifest, "ok", "脚本市场已刷新。"),
        ),
        Err(error) => {
            let msg = format!("{error}");
            let status = if msg.contains("404") || msg.contains("error status") {
                "empty"
            } else {
                "failed"
            };
            ok(
                "暂无脚本市场",
                ScriptMarketPayload {
                    market: json!({
                        "status": status,
                        "message": "暂无可用脚本市场",
                        "indexUrl": script_market::DEFAULT_MARKET_INDEX_URL,
                        "updatedAt": "",
                        "scripts": []
                    }),
                    user_scripts: user_script_inventory(),
                },
            )
        }
    }
}

#[tauri::command]
pub async fn install_market_script(id: String) -> CommandResult<ScriptMarketPayload> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return failed(
            "脚本 id 不能为空。",
            failed_script_market_payload("脚本 id 不能为空。"),
        );
    }
    let manifest =
        match script_market::fetch_market_manifest(script_market::DEFAULT_MARKET_INDEX_URL).await {
            Ok(manifest) => manifest,
            Err(_error) => {
                return ok(
                    "暂无脚本市场",
                    ScriptMarketPayload {
                        market: json!({
                            "status": "empty",
                            "message": "暂无可用脚本市场",
                            "indexUrl": script_market::DEFAULT_MARKET_INDEX_URL,
                            "updatedAt": "",
                            "scripts": []
                        }),
                        user_scripts: user_script_inventory(),
                    },
                );
            }
        };
    let Some(script) = manifest.scripts.iter().find(|script| script.id == trimmed) else {
        return failed(
            "市场清单中未找到该脚本。",
            script_market_payload_from_manifest(&manifest, "failed", "市场清单中未找到该脚本。"),
        );
    };
    let manager = default_user_script_manager();
    match script_market::install_market_script(&manager, script).await {
        Ok(()) => ok(
            "脚本已安装。",
            script_market_payload_from_manifest(&manifest, "ok", "脚本已安装。"),
        ),
        Err(error) => failed(
            &format!("安装脚本失败：{error}"),
            script_market_payload_from_manifest(
                &manifest,
                "failed",
                &format!("安装脚本失败：{error}"),
            ),
        ),
    }
}

#[tauri::command]
pub fn set_user_script_enabled(key: String, enabled: bool) -> CommandResult<SettingsPayload> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return failed("脚本 key 不能为空。", fallback_settings_payload());
    }
    let manager = default_user_script_manager();
    match manager.set_script_enabled(trimmed, enabled) {
        Ok(_) => settings_payload(
            if enabled {
                "脚本已启用。"
            } else {
                "脚本已禁用。"
            },
            "脚本启停失败",
        ),
        Err(error) => failed(
            &format!("脚本启停失败：{error}"),
            fallback_settings_payload(),
        ),
    }
}

#[tauri::command]
pub fn delete_user_script(key: String) -> CommandResult<SettingsPayload> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return failed("脚本 key 不能为空。", fallback_settings_payload());
    }
    let manager = default_user_script_manager();
    match manager.delete_user_script(trimmed) {
        Ok(_) => settings_payload("脚本已删除。", "脚本删除失败"),
        Err(error) => failed(
            &format!("脚本删除失败：{error}"),
            fallback_settings_payload(),
        ),
    }
}

#[tauri::command]
pub fn open_external_url(url: String) -> CommandResult<Value> {
    let trimmed = url.trim();
    if !(trimmed.starts_with("https://") || trimmed.starts_with("http://")) {
        return failed("只允许打开 http 或 https 链接。", json!({}));
    }
    match open_url(trimmed) {
        Ok(()) => ok("已在系统浏览器打开链接。", json!({ "url": trimmed })),
        Err(error) => failed(&format!("打开链接失败：{error}"), json!({ "url": trimmed })),
    }
}

#[tauri::command]
pub async fn install_entrypoints() -> InstallActionResult {
    tauri::async_runtime::spawn_blocking(install::install_entrypoints)
        .await
        .unwrap_or_else(|error| install_background_failure("安装入口", error))
}

#[tauri::command]
pub async fn uninstall_entrypoints(options: InstallOptions) -> InstallActionResult {
    tauri::async_runtime::spawn_blocking(move || install::uninstall_entrypoints(options))
        .await
        .unwrap_or_else(|error| install_background_failure("卸载入口", error))
}

#[tauri::command]
pub async fn repair_shortcuts() -> InstallActionResult {
    tauri::async_runtime::spawn_blocking(install::repair_shortcuts)
        .await
        .unwrap_or_else(|error| install_background_failure("修复快捷方式", error))
}

#[tauri::command]
pub fn repair_backend() -> CommandResult<SettingsPayload> {
    settings_payload("后端已检查，配置有效。", "修复后重新读取设置失败")
}

#[tauri::command]
pub async fn check_update() -> CommandResult<Value> {
    match codexbox_core::update::check_for_update(codexbox_core::version::VERSION).await {
        Ok(update) => {
            let status = if update.update_available {
                "ok"
            } else {
                "not_checked"
            };
            CommandResult {
                status: status.to_string(),
                message: if update.update_available {
                    "发现可用更新。".to_string()
                } else {
                    "当前已是最新版本。".to_string()
                },
                payload: json!({
                    "currentVersion": update.current_version,
                    "latestVersion": update.latest_version,
                    "releaseSummary": update.release_summary,
                    "assetName": update.asset_name,
                    "assetUrl": update.asset_url,
                    "updateAvailable": update.update_available,
                    "progress": 0
                }),
            }
        }
        Err(error) => failed(
            &format!("检查更新失败：{error}"),
            json!({
                "currentVersion": codexbox_core::version::VERSION,
                "latestVersion": Value::Null,
                "releaseSummary": "",
                "assetName": Value::Null,
                "assetUrl": Value::Null,
                "updateAvailable": false,
                "progress": 0
            }),
        ),
    }
}

#[tauri::command]
pub async fn perform_update(
    release: Option<codexbox_core::update::Release>,
) -> CommandResult<Value> {
    let Some(release) = release else {
        return failed(
            "请先检查更新并选择可下载的 Release asset。",
            json!({
                "currentVersion": codexbox_core::version::VERSION,
                "progress": 0
            }),
        );
    };
    let download_dir = codexbox_core::paths::default_app_state_dir().join("updates");
    match codexbox_core::update::perform_update(&release, &download_dir).await {
        Ok(result) => ok(
            "安装包已下载并启动，请按安装向导完成更新。",
            json!({
                "currentVersion": codexbox_core::version::VERSION,
                "latestVersion": result.release.version,
                "releaseSummary": result.release.body,
                "installedPath": result.installer_path.to_string_lossy(),
                "launched": result.launched,
                "progress": 100
            }),
        ),
        Err(error) => failed(
            &format!("安装更新失败：{error}"),
            json!({
                "currentVersion": codexbox_core::version::VERSION,
                "latestVersion": release.version,
                "releaseSummary": release.body,
                "progress": 0
            }),
        ),
    }
}

#[tauri::command]
pub fn load_watcher_state() -> CommandResult<WatcherPayload> {
    ok("watcher 状态已加载。", watcher_payload())
}

#[tauri::command]
pub fn install_watcher() -> CommandResult<WatcherPayload> {
    let launcher_path =
        codexbox_core::install::companion_binary_path(codexbox_core::install::SILENT_BINARY);
    match codexbox_core::watcher::install_watcher(&launcher_path, default_debug_port()) {
        Ok(()) => ok("watcher 已安装。", watcher_payload()),
        Err(error) => failed(&format!("安装 watcher 失败：{error}"), watcher_payload()),
    }
}

#[tauri::command]
pub fn uninstall_watcher() -> CommandResult<WatcherPayload> {
    match codexbox_core::watcher::uninstall_watcher() {
        Ok(()) => ok("watcher 已移除。", watcher_payload()),
        Err(error) => failed(&format!("移除 watcher 失败：{error}"), watcher_payload()),
    }
}

#[tauri::command]
pub fn enable_watcher() -> CommandResult<WatcherPayload> {
    match codexbox_core::watcher::enable_watcher() {
        Ok(()) => ok("watcher 已启用。", watcher_payload()),
        Err(error) => failed(&format!("启用 watcher 失败：{error}"), watcher_payload()),
    }
}

#[tauri::command]
pub fn disable_watcher() -> CommandResult<WatcherPayload> {
    match codexbox_core::watcher::disable_watcher() {
        Ok(()) => ok("watcher 已禁用。", watcher_payload()),
        Err(error) => failed(&format!("禁用 watcher 失败：{error}"), watcher_payload()),
    }
}

#[tauri::command]
pub fn read_latest_logs(request: LogRequest) -> CommandResult<LogsPayload> {
    let path = codexbox_core::paths::default_diagnostic_log_path();
    match read_tail(&path, request.lines) {
        Ok(text) => ok(
            "日志已读取。",
            LogsPayload {
                path: path.to_string_lossy().to_string(),
                text,
                lines: request.lines,
            },
        ),
        Err(error) => failed(
            &format!("读取日志失败：{error}"),
            LogsPayload {
                path: path.to_string_lossy().to_string(),
                text: String::new(),
                lines: request.lines,
            },
        ),
    }
}

#[tauri::command]
pub fn copy_diagnostics() -> CommandResult<DiagnosticsPayload> {
    ok(
        "诊断报告已生成。",
        DiagnosticsPayload {
            report: diagnostics_report(),
        },
    )
}

#[tauri::command]
pub fn reset_settings() -> CommandResult<SettingsPayload> {
    let settings = BackendSettings::default();
    match SettingsStore::default().save(&settings) {
        Ok(()) => settings_payload("设置已重置为默认值。", "设置重置后重新读取失败"),
        Err(error) => failed(
            &format!("重置设置失败：{error}"),
            SettingsPayload {
                settings,
                settings_path: codexbox_core::paths::default_settings_path()
                    .to_string_lossy()
                    .to_string(),
                user_scripts: user_script_inventory(),
            },
        ),
    }
}

#[tauri::command]
pub fn write_diagnostic_event(event: String, detail: Value) -> CommandResult<Value> {
    let event = sanitize_manager_event(&event);
    match codexbox_core::diagnostic_log::append_diagnostic_log(&event, detail) {
        Ok(()) => ok("诊断日志已写入。", json!({})),
        Err(error) => failed(&format!("写入诊断日志失败：{error}"), json!({})),
    }
}

#[tauri::command]
pub fn list_context_entries(
    request: ContextSettingsRequest,
) -> CommandResult<ContextEntriesPayload> {
    match codexbox_core::relay_config::list_context_entries_from_common_config(
        &request.settings.relay_context_config_contents,
    ) {
        Ok(entries) => ok(
            "工具与插件列表已读取。",
            ContextEntriesPayload {
                settings: request.settings,
                entries,
            },
        ),
        Err(error) => failed(
            &format!("读取工具与插件列表失败：{error}"),
            ContextEntriesPayload {
                settings: request.settings,
                entries: empty_context_entries(),
            },
        ),
    }
}

#[tauri::command]
pub fn read_live_context_entries() -> CommandResult<LiveContextEntriesPayload> {
    let home = codexbox_core::relay_config::default_codex_home_dir();
    let config_path = home.join("config.toml");
    let config = read_optional_text_file(&config_path).unwrap_or_default();
    match codexbox_core::relay_config::list_context_entries_from_common_config(&config) {
        Ok(entries) => ok(
            "live 工具与插件已读取。",
            LiveContextEntriesPayload { entries },
        ),
        Err(error) => failed(
            &format!("读取 live 工具与插件失败：{error}"),
            LiveContextEntriesPayload {
                entries: empty_context_entries(),
            },
        ),
    }
}

#[tauri::command]
pub fn upsert_context_entry(request: ContextEntryRequest) -> CommandResult<ContextEntriesPayload> {
    let mut settings = request.settings;
    match codexbox_core::relay_config::upsert_context_entry_in_common_config(
        &settings.relay_context_config_contents,
        &request.kind,
        &request.id,
        &request.toml_body,
    ) {
        Ok(common) => {
            settings.relay_context_config_contents = common;
            list_context_entries(ContextSettingsRequest { settings })
        }
        Err(error) => failed(
            &format!("保存工具与插件失败：{error}"),
            ContextEntriesPayload {
                settings,
                entries: empty_context_entries(),
            },
        ),
    }
}

#[tauri::command]
pub fn sync_live_context_entries(
    request: ContextSettingsRequest,
) -> CommandResult<LiveContextEntriesPayload> {
    let home = codexbox_core::relay_config::default_codex_home_dir();
    let config_path = home.join("config.toml");
    let current_config = match read_optional_text_file(&config_path) {
        Ok(config) => config,
        Err(error) => {
            return failed(
                &format!("读取 live config.toml 失败：{error}"),
                LiveContextEntriesPayload {
                    entries: empty_context_entries(),
                },
            );
        }
    };
    let updated_config = match codexbox_core::relay_config::sync_live_config_context_entries(
        &current_config,
        &request.settings.relay_context_config_contents,
    ) {
        Ok(config) => config,
        Err(error) => {
            return failed(
                &format!("同步 live 工具与插件失败：{error}"),
                LiveContextEntriesPayload {
                    entries: empty_context_entries(),
                },
            );
        }
    };
    if let Some(parent) = config_path.parent() {
        if let Err(error) = std::fs::create_dir_all(parent) {
            return failed(
                &format!("创建 Codex 配置目录失败：{error}"),
                LiveContextEntriesPayload {
                    entries: empty_context_entries(),
                },
            );
        }
    }
    if let Err(error) = std::fs::write(&config_path, &updated_config) {
        return failed(
            &format!("写入 live config.toml 失败：{error}"),
            LiveContextEntriesPayload {
                entries: empty_context_entries(),
            },
        );
    }
    match codexbox_core::relay_config::list_context_entries_from_common_config(&updated_config) {
        Ok(entries) => ok(
            "live 工具与插件已同步。",
            LiveContextEntriesPayload { entries },
        ),
        Err(error) => failed(
            &format!("读取同步后的 live 工具与插件失败：{error}"),
            LiveContextEntriesPayload {
                entries: empty_context_entries(),
            },
        ),
    }
}

#[tauri::command]
pub fn delete_context_entry(request: ContextDeleteRequest) -> CommandResult<ContextEntriesPayload> {
    let mut settings = request.settings;
    match codexbox_core::relay_config::delete_context_entry_from_common_config(
        &settings.relay_context_config_contents,
        &request.kind,
        &request.id,
    ) {
        Ok(common) => {
            settings.relay_context_config_contents = common;
            list_context_entries(ContextSettingsRequest { settings })
        }
        Err(error) => failed(
            &format!("删除工具与插件失败：{error}"),
            ContextEntriesPayload {
                settings,
                entries: empty_context_entries(),
            },
        ),
    }
}

fn sanitize_manager_event(event: &str) -> String {
    let suffix = event
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let suffix = suffix.trim_matches(['.', '_', '-']).trim();
    if suffix.is_empty() {
        "manager.ui.event".to_string()
    } else if suffix.starts_with("manager.") {
        suffix.to_string()
    } else {
        format!("manager.ui.{suffix}")
    }
}

fn empty_context_entries() -> codexbox_core::relay_config::CodexContextEntries {
    codexbox_core::relay_config::CodexContextEntries {
        mcp_servers: Vec::new(),
        skills: Vec::new(),
        plugins: Vec::new(),
    }
}

fn read_optional_text_file(path: &std::path::Path) -> anyhow::Result<String> {
    match std::fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error.into()),
    }
}

fn ads_payload(payload: Value) -> AdsPayload {
    AdsPayload {
        version: payload.get("version").and_then(Value::as_u64).unwrap_or(1),
        ads: payload
            .get("ads")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
    }
}

fn open_url(url: &str) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        codexbox_core::windows_open_url(url)
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|error| anyhow::anyhow!("启动系统浏览器失败：{error}"))
    }
}

fn settings_payload(message: &str, failure_context: &str) -> CommandResult<SettingsPayload> {
    match settings_payload_value() {
        Ok(payload) => ok(message, payload),
        Err((error, payload)) => failed(&format!("{failure_context}：{error}"), payload),
    }
}

fn settings_payload_value() -> Result<SettingsPayload, (anyhow::Error, SettingsPayload)> {
    let store = SettingsStore::default();
    let settings_path = codexbox_core::paths::default_settings_path()
        .to_string_lossy()
        .to_string();
    match store.load() {
        Ok(settings) => Ok(SettingsPayload {
            settings: settings,
            settings_path,
            user_scripts: user_script_inventory(),
        }),
        Err(error) => Err((
            error,
            SettingsPayload {
                settings: BackendSettings::default(),
                settings_path,
                user_scripts: user_script_inventory(),
            },
        )),
    }
}

fn fallback_settings_payload() -> SettingsPayload {
    SettingsPayload {
        settings: SettingsStore::default().load().unwrap_or_default(),
        settings_path: codexbox_core::paths::default_settings_path()
            .to_string_lossy()
            .to_string(),
        user_scripts: user_script_inventory(),
    }
}

fn user_script_inventory() -> Value {
    default_user_script_manager()
        .inventory()
        .unwrap_or_else(|error| {
            json!({
                "enabled": true,
                "scripts": [],
                "error": error.to_string()
            })
        })
}

fn failed_script_market_payload(message: &str) -> ScriptMarketPayload {
    ScriptMarketPayload {
        market: json!({
            "status": "failed",
            "message": message,
            "indexUrl": script_market::DEFAULT_MARKET_INDEX_URL,
            "updatedAt": "",
            "scripts": []
        }),
        user_scripts: user_script_inventory(),
    }
}

fn script_market_payload_from_manifest(
    manifest: &ScriptMarketManifest,
    status: &str,
    message: &str,
) -> ScriptMarketPayload {
    let user_scripts = user_script_inventory();
    let installed = installed_market_versions(&user_scripts);
    let scripts = manifest
        .scripts
        .iter()
        .map(|script| market_script_payload(script, &installed))
        .collect::<Vec<_>>();
    ScriptMarketPayload {
        market: json!({
            "status": status,
            "message": message,
            "indexUrl": script_market::DEFAULT_MARKET_INDEX_URL,
            "updatedAt": manifest.updated_at.clone().unwrap_or_default(),
            "scripts": scripts
        }),
        user_scripts,
    }
}

fn installed_market_versions(user_scripts: &Value) -> BTreeMap<String, String> {
    user_scripts
        .get("scripts")
        .and_then(Value::as_array)
        .map(|scripts| {
            scripts
                .iter()
                .filter_map(|script| {
                    let id = script.get("market_id").and_then(Value::as_str)?;
                    if id.is_empty() {
                        return None;
                    }
                    let version = script
                        .get("version")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    Some((id.to_string(), version))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn market_script_payload(script: &MarketScript, installed: &BTreeMap<String, String>) -> Value {
    let installed_version = installed.get(&script.id).cloned().unwrap_or_default();
    let is_installed = !installed_version.is_empty();
    json!({
        "id": script.id,
        "name": script.name,
        "description": script.description,
        "version": script.version,
        "author": script.author,
        "tags": script.tags,
        "homepage": script.homepage,
        "script_url": script.script_url,
        "sha256": script.sha256,
        "installed": is_installed,
        "installedVersion": installed_version,
        "updateAvailable": is_installed && installed.get(&script.id).map(|version| version != &script.version).unwrap_or(false)
    })
}

fn default_user_script_manager() -> UserScriptManager {
    let config_dir = user_scripts_config_dir();
    UserScriptManager::new(
        builtin_user_scripts_dir(),
        config_dir.join("user_scripts"),
        config_dir.join("user_scripts.json"),
    )
}

fn user_scripts_config_dir() -> PathBuf {
    if cfg!(windows) {
        if let Some(roaming) = std::env::var_os("APPDATA") {
            return PathBuf::from(roaming).join("CodexBox");
        }
    }
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("CodexBox")
}

fn builtin_user_scripts_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .map(|path| path.join("user_scripts"))
        .unwrap_or_else(|| PathBuf::from("user_scripts"))
}

fn diagnostics_report() -> String {
    let (codex_app_path, entrypoints, latest_launch) = load_overview_payload();
    let overview = ok(
        "概览已加载。",
        OverviewPayload {
            codex_version: codex_app_path
                .as_deref()
                .and_then(codexbox_core::app_paths::codex_app_version),
            codex_app: path_state(codex_app_path),
            silent_shortcut: shortcut_state(entrypoints.silent_shortcut),
            management_shortcut: shortcut_state(entrypoints.management_shortcut),
            latest_launch,
            current_version: codexbox_core::version::VERSION.to_string(),
            update_status: "not_checked".to_string(),
            settings_path: codexbox_core::paths::default_settings_path()
                .to_string_lossy()
                .to_string(),
            logs_path: codexbox_core::paths::default_diagnostic_log_path()
                .to_string_lossy()
                .to_string(),
        },
    );
    let settings = SettingsStore::default().load().unwrap_or_default();
    let generated_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    serde_json::to_string_pretty(&json!({
        "generatedAtMs": generated_at_ms,
        "version": codexbox_core::version::VERSION,
        "overview": overview.payload,
        "settings": settings,
        "logs": {
            "diagnosticLogPath": codexbox_core::paths::default_diagnostic_log_path(),
            "latestStatusPath": codexbox_core::paths::default_latest_status_path()
        },
        "platform": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH
        }
    }))
    .unwrap_or_else(|error| format!("诊断报告序列化失败：{error}"))
}

fn load_overview_payload() -> (
    Option<PathBuf>,
    install::EntryPointState,
    Option<LaunchStatus>,
) {
    let settings = SettingsStore::default().load().unwrap_or_default();
    (
        codexbox_core::app_paths::resolve_codex_app_dir_with_saved(
            None,
            Some(settings.codex_app_path.as_str()),
        ),
        install::inspect_entrypoints(),
        StatusStore::default().load_latest().unwrap_or(None),
    )
}

fn install_background_failure(action: &str, error: impl std::fmt::Display) -> InstallActionResult {
    let state = install::inspect_entrypoints();
    InstallActionResult {
        status: "failed".to_string(),
        message: format!("{action}后台任务失败：{error}"),
        silent_shortcut: state.silent_shortcut,
        management_shortcut: state.management_shortcut,
    }
}

fn watcher_payload() -> WatcherPayload {
    let flag = codexbox_core::watcher::default_watcher_disabled_flag();
    WatcherPayload {
        enabled: !flag.exists(),
        disabled_flag: flag.to_string_lossy().to_string(),
    }
}

fn read_tail(path: &Path, max_lines: usize) -> std::io::Result<String> {
    let contents = fs::read_to_string(path)?;
    let mut lines = contents.lines().rev().take(max_lines).collect::<Vec<_>>();
    lines.reverse();
    Ok(lines.join("\n"))
}

fn path_state(path: Option<PathBuf>) -> PathState {
    match path {
        Some(path) => PathState {
            status: "found".to_string(),
            path: Some(path.to_string_lossy().to_string()),
        },
        None => PathState {
            status: "missing".to_string(),
            path: None,
        },
    }
}

fn shortcut_state(shortcut: install::ShortcutState) -> PathState {
    PathState {
        status: if shortcut.installed {
            "installed".to_string()
        } else {
            "missing".to_string()
        },
        path: shortcut.path,
    }
}

fn ok<T: Serialize>(message: &str, payload: T) -> CommandResult<T> {
    CommandResult {
        status: "ok".to_string(),
        message: message.to_string(),
        payload,
    }
}

fn failed<T: Serialize>(message: &str, payload: T) -> CommandResult<T> {
    CommandResult {
        status: "failed".to_string(),
        message: message.to_string(),
        payload,
    }
}

fn default_debug_port() -> u16 {
    9229
}

fn default_helper_port() -> u16 {
    57321
}

fn default_log_lines() -> usize {
    200
}


// ==================== 智能路由管理命令 ====================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingConfigPayload {
    pub config: codexbox_core::router::SmartRouterConfig,
    pub config_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteTestPayload {
    pub provider_id: String,
    pub provider_name: String,
    pub target_model: String,
    pub rule_name: String,
}

#[tauri::command]
pub fn load_routing_config() -> CommandResult<RoutingConfigPayload> {
    let config_path = codexbox_core::paths::default_app_state_dir().join("routing.toml");
    let config = if config_path.exists() {
        std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|c| toml::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        codexbox_core::router::SmartRouterConfig::default()
    };
    CommandResult {
        status: "ok".to_string(),
        message: "路由配置加载成功".to_string(),
        payload: RoutingConfigPayload {
            config,
            config_path: config_path.to_string_lossy().to_string(),
        },
    }
}

#[tauri::command]
pub fn save_routing_config(mut config: codexbox_core::router::SmartRouterConfig) -> CommandResult<RoutingConfigPayload> {
    let config_path = codexbox_core::paths::default_app_state_dir().join("routing.toml");
    // 保持原有 API key（前端可能发回的是脱敏值）
    if let Ok(old_raw) = std::fs::read_to_string(&config_path) {
        if let Ok(old_config) = toml::from_str::<codexbox_core::router::SmartRouterConfig>(&old_raw) {
            for provider in &mut config.providers {
                if let Some(old) = old_config.providers.iter().find(|p| p.id == provider.id) {
                    if provider.api_key == codexbox_core::router::api_key_masked_str(&old.api_key) {
                        provider.api_key = old.api_key.clone();
                    }
                }
            }
        }
    }
    if let Some(parent) = config_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let toml_content = match toml::to_string_pretty(&config) {
        Ok(c) => c,
        Err(e) => return CommandResult {
            status: "failed".to_string(),
            message: format!("序列化配置失败: {}", e),
            payload: RoutingConfigPayload { config, config_path: config_path.to_string_lossy().to_string() },
        },
    };
    match std::fs::write(&config_path, &toml_content) {
        Ok(_) => CommandResult {
            status: "ok".to_string(),
            message: "路由配置保存成功".to_string(),
            payload: RoutingConfigPayload { config, config_path: config_path.to_string_lossy().to_string() },
        },
        Err(e) => CommandResult {
            status: "failed".to_string(),
            message: format!("保存配置失败: {}", e),
            payload: RoutingConfigPayload { config, config_path: config_path.to_string_lossy().to_string() },
        },
    }
}

#[tauri::command]
pub async fn test_routing_decision(
    config: codexbox_core::router::SmartRouterConfig,
    model: String,
    has_image: bool,
) -> CommandResult<RouteTestPayload> {
    use codexbox_core::router::*;
    let router = RouterEngine::new(config);
    let request = RequestContext { model, has_image, ..Default::default() };
    match router.route(&request).await {
        Ok(decision) => CommandResult {
            status: "ok".to_string(),
            message: format!("路由匹配成功: 规则 '{}'", decision.rule_name),
            payload: RouteTestPayload {
                provider_id: decision.provider.id,
                provider_name: decision.provider.name,
                target_model: decision.target_model,
                rule_name: decision.rule_name,
            },
        },
        Err(e) => CommandResult {
            status: "failed".to_string(),
            message: format!("路由决策失败: {}", e),
            payload: RouteTestPayload { provider_id: String::new(), provider_name: String::new(), target_model: String::new(), rule_name: String::new() },
        },
    }
}

#[tauri::command]
pub fn upsert_provider(mut provider: codexbox_core::router::SmartProvider) -> CommandResult<RoutingConfigPayload> {
    let config_path = codexbox_core::paths::default_app_state_dir().join("routing.toml");
    let mut config: codexbox_core::router::SmartRouterConfig = if config_path.exists() {
        std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|c| toml::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        codexbox_core::router::SmartRouterConfig::default()
    };
    if let Some(existing) = config.providers.iter_mut().find(|p| p.id == provider.id) {
        // 保持原有 API key（前端可能发回的是脱敏值）
        let incoming_key = std::mem::replace(&mut provider.api_key, String::new());
        if incoming_key == codexbox_core::router::api_key_masked_str(&existing.api_key) {
            provider.api_key = existing.api_key.clone();
        } else {
            provider.api_key = incoming_key;
        }
        *existing = provider;
    } else {
        config.providers.push(provider);
    }
    if let Some(parent) = config_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let toml_content = match toml::to_string_pretty(&config) {
        Ok(c) => c,
        Err(e) => return CommandResult {
            status: "failed".to_string(),
            message: format!("序列化配置失败: {}", e),
            payload: RoutingConfigPayload { config, config_path: config_path.to_string_lossy().to_string() },
        },
    };
    match std::fs::write(&config_path, &toml_content) {
        Ok(_) => CommandResult {
            status: "ok".to_string(),
            message: "模型保存成功".to_string(),
            payload: RoutingConfigPayload { config, config_path: config_path.to_string_lossy().to_string() },
        },
        Err(e) => CommandResult {
            status: "failed".to_string(),
            message: format!("保存配置失败: {}", e),
            payload: RoutingConfigPayload { config, config_path: config_path.to_string_lossy().to_string() },
        },
    }
}

#[tauri::command]
pub fn delete_provider(provider_id: String) -> CommandResult<RoutingConfigPayload> {
    let config_path = codexbox_core::paths::default_app_state_dir().join("routing.toml");
    let mut config: codexbox_core::router::SmartRouterConfig = if config_path.exists() {
        std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|c| toml::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        codexbox_core::router::SmartRouterConfig::default()
    };
    config.providers.retain(|p| p.id != provider_id);
    if config.vision_fallback_model == provider_id {
        config.vision_fallback_model = String::new();
    }
    if let Some(parent) = config_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let toml_content = match toml::to_string_pretty(&config) {
        Ok(c) => c,
        Err(e) => return CommandResult {
            status: "failed".to_string(),
            message: format!("序列化配置失败: {}", e),
            payload: RoutingConfigPayload { config, config_path: config_path.to_string_lossy().to_string() },
        },
    };
    match std::fs::write(&config_path, &toml_content) {
        Ok(_) => CommandResult {
            status: "ok".to_string(),
            message: "模型已删除".to_string(),
            payload: RoutingConfigPayload { config, config_path: config_path.to_string_lossy().to_string() },
        },
        Err(e) => CommandResult {
            status: "failed".to_string(),
            message: format!("删除配置失败: {}", e),
            payload: RoutingConfigPayload { config, config_path: config_path.to_string_lossy().to_string() },
        },
    }
}

#[tauri::command]
pub async fn test_smart_provider(mut provider: codexbox_core::router::SmartProvider) -> CommandResult<RelayProfileTestPayload> {
    let base_url = provider.base_url.trim().to_string();
    // 如果前端传来的是脱敏 key，从磁盘配置恢复真实 key
    let config_path_ref = codexbox_core::paths::default_app_state_dir().join("routing.toml");
    if let Ok(raw) = std::fs::read_to_string(&config_path_ref) {
        if let Ok(stored) = toml::from_str::<codexbox_core::router::SmartRouterConfig>(&raw) {
            if let Some(existing) = stored.providers.iter().find(|p| p.id == provider.id) {
                if provider.api_key == codexbox_core::router::api_key_masked_str(&existing.api_key) {
                    provider.api_key = existing.api_key.clone();
                }
            }
        }
    }
    let api_key = provider.api_key.trim().to_string();
    if base_url.is_empty() {
        return CommandResult {
            status: "failed".to_string(),
            message: "Base URL 不能为空".to_string(),
            payload: RelayProfileTestPayload { http_status: 0, endpoint: String::new(), response_preview: String::new() },
        };
    }
    let test_url = codexbox_core::protocol_proxy::models_url_with(&base_url, provider.use_full_url);
    let client = reqwest::Client::new();
    match client.get(&test_url).bearer_auth(&api_key).timeout(std::time::Duration::from_secs(10)).send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            let preview = if body.len() > 500 { format!("{}...", &body[..500]) } else { body };
            CommandResult {
                status: if (200..400).contains(&status) { "ok" } else { "failed" }.to_string(),
                message: format!("HTTP {} - {}", status, if (200..400).contains(&status) { "连接成功" } else { "连接失败" }),
                payload: RelayProfileTestPayload { http_status: status, endpoint: test_url, response_preview: preview },
            }
        }
        Err(e) => CommandResult {
            status: "failed".to_string(),
            message: format!("连接失败: {}", e),
            payload: RelayProfileTestPayload { http_status: 0, endpoint: test_url, response_preview: String::new() },
        },
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_version_returns_structured_payload() {
        let result = backend_version();

        assert_eq!(result.status, "ok");
        assert!(!result.payload.version.is_empty());
    }

    #[test]
    fn startup_options_returns_structured_payload() {
        let result = startup_options();

        assert_eq!(result.status, "ok");
    }

    #[test]
    fn startup_options_honors_show_update_environment() {
        unsafe {
            std::env::set_var("CODEX_PLUS_SHOW_UPDATE", "1");
        }

        let result = startup_options();

        unsafe {
            std::env::remove_var("CODEX_PLUS_SHOW_UPDATE");
        }

        assert_eq!(result.status, "ok");
        assert!(result.payload.show_update);
    }

    #[test]
    fn startup_options_honors_show_update_argument() {
        assert!(should_show_update(
            ["codexbox-manager.exe", "--show-update"],
            None
        ));
    }

    #[test]
    fn overview_contains_expected_operational_fields() {
        let result = tauri::async_runtime::block_on(load_overview());

        assert_eq!(result.status, "ok");
        assert!(!result.payload.current_version.is_empty());
        assert!(
            result.payload.codex_version.is_none()
                || result
                    .payload
                    .codex_version
                    .as_deref()
                    .is_some_and(|version| !version.is_empty())
        );
        assert!(matches!(
            result.payload.codex_app.status.as_str(),
            "found" | "missing"
        ));
        assert!(matches!(
            result.payload.silent_shortcut.status.as_str(),
            "installed" | "missing"
        ));
    }

    #[test]
    fn update_install_requires_release_payload() {
        let result = tauri::async_runtime::block_on(perform_update(None));

        assert_eq!(result.status, "failed");
        assert!(result.message.contains("请先检查更新"));
    }

    #[test]
    fn watcher_state_returns_disabled_flag_path() {
        let result = load_watcher_state();

        assert_eq!(result.status, "ok");
        assert!(result.payload.disabled_flag.contains("watcher.disabled"));
    }

    #[test]
    fn missing_logs_return_failed_status() {
        let result = read_latest_logs(LogRequest { lines: 25 });

        if result.payload.text.is_empty() {
            assert_eq!(result.status, "failed");
        }
    }











    #[test]
    fn context_entry_commands_update_settings_payload() {
        let settings = BackendSettings::default();
        let upsert = upsert_context_entry(ContextEntryRequest {
            settings: settings.clone(),
            kind: "mcp".to_string(),
            id: "context7".to_string(),
            toml_body: "command = \"npx\"\n".to_string(),
        });

        assert_eq!(upsert.status, "ok");
        assert!(
            upsert
                .payload
                .settings
                .relay_context_config_contents
                .contains("[mcp_servers.context7]")
        );

        let listed = list_context_entries(ContextSettingsRequest {
            settings: upsert.payload.settings.clone(),
        });
        assert_eq!(listed.payload.entries.mcp_servers[0].id, "context7");

        let deleted = delete_context_entry(ContextDeleteRequest {
            settings: upsert.payload.settings,
            kind: "mcp".to_string(),
            id: "context7".to_string(),
        });
        assert_eq!(deleted.status, "ok");
        assert!(
            !deleted
                .payload
                .settings
                .relay_context_config_contents
                .contains("[mcp_servers.context7]")
        );
    }

    #[test]
    fn ads_payload_keeps_version_and_ad_items() {
        let payload = ads_payload(json!({
            "version": 1,
            "ads": [{"id": "ad-1", "type": "normal", "title": "Ad"}]
        }));

        assert_eq!(payload.version, 1);
        assert_eq!(payload.ads.len(), 1);
        assert_eq!(payload.ads[0]["id"], json!("ad-1"));
    }

    #[test]
    fn open_external_url_rejects_non_http_urls() {
        let result = open_external_url("file:///C:/Windows/win.ini".to_string());

        assert_eq!(result.status, "failed");
        assert!(result.message.contains("只允许打开 http 或 https 链接"));
    }
}
