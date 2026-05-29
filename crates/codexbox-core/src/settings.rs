use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BackendSettings {
    #[serde(rename = "codexAppPath", default)]
    pub codex_app_path: String,
    #[serde(rename = "codexExtraArgs", default)]
    pub codex_extra_args: Vec<String>,
    #[serde(rename = "enhancementsEnabled", default = "default_true")]
    pub enhancements_enabled: bool,
    #[serde(rename = "codexGoalsEnabled", default)]
    pub codex_goals_enabled: bool,
    #[serde(rename = "relayCommonConfigContents", default)]
    pub relay_common_config_contents: String,
    #[serde(rename = "relayContextConfigContents", default)]
    pub relay_context_config_contents: String,
}

impl Default for BackendSettings {
    fn default() -> Self {
        Self {
            codex_app_path: String::new(),
            codex_extra_args: Vec::new(),
            enhancements_enabled: true,
            codex_goals_enabled: false,
            relay_common_config_contents: String::new(),
            relay_context_config_contents: String::new(),
        }
    }
}

pub fn default_true() -> bool {
    true
}

pub fn normalize_codex_extra_args(args: &[String]) -> Vec<String> {
    args.iter()
        .map(|arg| arg.trim())
        .filter(|arg| !arg.is_empty())
        .map(ToString::to_string)
        .collect()
}

#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl Default for SettingsStore {
    fn default() -> Self {
        Self::new(crate::paths::default_settings_path())
    }
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> anyhow::Result<BackendSettings> {
        let contents = match fs::read_to_string(&self.path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(BackendSettings::default());
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read settings {}", self.path.display()));
            }
        };

        Ok(normalize_settings_config_sections(
            serde_json::from_str(&contents).unwrap_or_default(),
        ))
    }

    pub fn save(&self, settings: &BackendSettings) -> anyhow::Result<()> {
        let mut settings = normalize_settings_config_sections(settings.clone());
        settings.codex_extra_args = normalize_codex_extra_args(&settings.codex_extra_args);
        let bytes = serde_json::to_vec_pretty(&settings)?;
        atomic_write(&self.path, &bytes)
    }

    pub fn update(&self, payload: Value) -> anyhow::Result<BackendSettings> {
        let Value::Object(payload) = payload else {
            return self.load();
        };

        let mut raw = self.load_raw_object()?;
        merge_known_setting_fields(&mut raw, &payload);
        let settings = normalize_settings_config_sections(
            serde_json::from_value(Value::Object(raw.clone())).unwrap_or_default(),
        );
        raw.insert(
            "relayCommonConfigContents".to_string(),
            Value::String(settings.relay_common_config_contents.clone()),
        );
        raw.insert(
            "relayContextConfigContents".to_string(),
            Value::String(settings.relay_context_config_contents.clone()),
        );
        let bytes = serde_json::to_vec_pretty(&Value::Object(raw))?;
        atomic_write(&self.path, &bytes)?;
        Ok(settings)
    }

    fn load_raw_object(&self) -> anyhow::Result<Map<String, Value>> {
        let contents = match fs::read_to_string(&self.path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(settings_to_object(&BackendSettings::default()));
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read settings {}", self.path.display()));
            }
        };

        match serde_json::from_str::<Value>(&contents) {
            Ok(Value::Object(map)) => Ok(map),
            Ok(_) | Err(_) => Ok(settings_to_object(&BackendSettings::default())),
        }
    }
}

fn merge_known_setting_fields(target: &mut Map<String, Value>, source: &Map<String, Value>) {
    if let Some(value) = source.get("codexAppPath").and_then(Value::as_str) {
        target.insert("codexAppPath".to_string(), Value::String(value.to_string()));
    }
    if let Some(value) = source.get("codexExtraArgs").and_then(Value::as_array) {
        let args = value
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        target.insert(
            "codexExtraArgs".to_string(),
            Value::Array(
                normalize_codex_extra_args(&args)
                    .into_iter()
                    .map(Value::String)
                    .collect(),
            ),
        );
    }
    if let Some(value) = source.get("enhancementsEnabled").and_then(Value::as_bool) {
        target.insert("enhancementsEnabled".to_string(), Value::Bool(value));
    }
    if let Some(value) = source.get("codexGoalsEnabled").and_then(Value::as_bool) {
        target.insert("codexGoalsEnabled".to_string(), Value::Bool(value));
    }
    if let Some(value) = source
        .get("relayCommonConfigContents")
        .and_then(Value::as_str)
    {
        target.insert(
            "relayCommonConfigContents".to_string(),
            Value::String(value.to_string()),
        );
    }
    if let Some(value) = source
        .get("relayContextConfigContents")
        .and_then(Value::as_str)
    {
        target.insert(
            "relayContextConfigContents".to_string(),
            Value::String(value.to_string()),
        );
    }
}

fn settings_to_object(settings: &BackendSettings) -> Map<String, Value> {
    match serde_json::to_value(settings).unwrap_or_else(|_| Value::Object(Map::new())) {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

fn normalize_settings_config_sections(mut settings: BackendSettings) -> BackendSettings {
    let (common, extracted_context) =
        split_context_config_sections(&settings.relay_common_config_contents);
    let context = join_config_sections(&[
        settings.relay_context_config_contents.as_str(),
        extracted_context.as_str(),
    ]);
    settings.relay_common_config_contents = crate::relay_config::normalize_config_text(&common);
    settings.relay_context_config_contents = crate::relay_config::normalize_config_text(&context);
    settings
}

fn split_context_config_sections(config: &str) -> (String, String) {
    let mut common = Vec::new();
    let mut context = Vec::new();
    let mut in_context_table = false;

    for line in config.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_context_table = is_context_table_header(trimmed);
        }
        if in_context_table {
            context.push(line);
        } else {
            common.push(line);
        }
    }

    (
        normalize_text_config(common.join("\n")),
        normalize_text_config(context.join("\n")),
    )
}

fn is_context_table_header(header: &str) -> bool {
    header.starts_with("[mcp_servers.")
        || header.starts_with("[skills.")
        || header.starts_with("[plugins.")
}

fn join_config_sections(sections: &[&str]) -> String {
    let joined = sections
        .iter()
        .map(|section| section.trim())
        .filter(|section| !section.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    normalize_text_config(joined)
}

fn normalize_text_config(contents: String) -> String {
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}\n")
    }
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    let temp_path = temp_path_for(path);
    fs::write(&temp_path, bytes)
        .with_context(|| format!("failed to write temp file {}", temp_path.display()))?;
    fs::rename(&temp_path, path).with_context(|| {
        format!(
            "failed to replace {} with {}",
            path.display(),
            temp_path.display()
        )
    })?;
    Ok(())
}

fn temp_path_for(path: &Path) -> PathBuf {
    let mut temp_path = path.to_path_buf();
    let extension = path.extension().and_then(|value| value.to_str());
    temp_path.set_extension(match extension {
        Some(extension) => format!("{extension}.tmp"),
        None => "tmp".to_string(),
    });
    temp_path
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "codexbox-core-settings-test-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn settings_default_matches_expected_behavior() {
        let settings = BackendSettings::default();
        assert!(settings.enhancements_enabled);
        assert!(!settings.codex_goals_enabled);
        assert!(settings.codex_app_path.is_empty());
        assert!(settings.codex_extra_args.is_empty());
        assert!(settings.relay_common_config_contents.is_empty());
        assert!(settings.relay_context_config_contents.is_empty());
    }

    #[test]
    fn settings_deserialize_uses_existing_json_keys() {
        let settings: BackendSettings = serde_json::from_str(
            r#"{"codexAppPath":"C:\\Portable\\Codex\\app","codexGoalsEnabled":true}"#,
        )
        .unwrap();
        assert_eq!(settings.codex_app_path, r"C:\Portable\Codex\app");
        assert!(settings.codex_goals_enabled);
        assert!(settings.codex_extra_args.is_empty());
    }

    #[test]
    fn settings_deserialize_reads_codex_extra_args() {
        let settings: BackendSettings = serde_json::from_str(
            r#"{"codexExtraArgs":["--force_high_performance_gpu"," --ignored-trimmed-by-ui "]}"#,
        )
        .unwrap();

        assert_eq!(
            settings.codex_extra_args,
            vec![
                "--force_high_performance_gpu".to_string(),
                " --ignored-trimmed-by-ui ".to_string(),
            ]
        );
    }

    #[test]
    fn settings_deserialize_ignores_removed_relay_fields() {
        let settings: BackendSettings = serde_json::from_str(
            r#"{
                "relayProfiles":[{"id":"old","name":"旧供应商"}],
                "activeRelayId":"old",
                "launchMode":"relay",
                "relayBaseUrl":"https://example.test",
                "relayApiKey":"sk-old",
                "providerSyncEnabled":true,
                "cliWrapperEnabled":true,
                "codexAppPath":"/Applications/Codex.app"
            }"#,
        )
        .unwrap();
        assert_eq!(settings.codex_app_path, "/Applications/Codex.app");
        assert!(settings.enhancements_enabled);
    }

    #[test]
    fn settings_store_load_missing_file_returns_default() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));

        assert_eq!(store.load().unwrap(), BackendSettings::default());
    }

    #[test]
    fn settings_store_load_bad_json_returns_default() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        std::fs::write(&path, "{bad json").unwrap();
        let store = SettingsStore::new(path);

        assert_eq!(store.load().unwrap(), BackendSettings::default());
    }

    #[test]
    fn settings_store_save_load_roundtrip_uses_custom_path() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("nested").join("settings.json"));
        let settings = BackendSettings {
            codex_extra_args: vec!["--force_high_performance_gpu".to_string()],
            ..BackendSettings::default()
        };

        store.save(&settings).unwrap();

        assert_eq!(store.load().unwrap(), settings);
    }

    #[test]
    fn settings_store_update_only_mutates_present_known_fields() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));
        let initial = BackendSettings::default();
        store.save(&initial).unwrap();

        let updated = store
            .update(json!({
            "codexAppPath": "C:\\Portable\\Codex\\Codex.exe",
            "enhancementsEnabled": false,
            "codexGoalsEnabled": true,
            "codexExtraArgs": ["--force_high_performance_gpu", "", "  ", " --enable-gpu "],
            "unknownKey": "ignored"
            }))
            .unwrap();

        assert_eq!(updated.codex_app_path, r"C:\Portable\Codex\Codex.exe");
        assert!(!updated.enhancements_enabled);
        assert!(updated.codex_goals_enabled);
        assert_eq!(
            updated.codex_extra_args,
            vec![
                "--force_high_performance_gpu".to_string(),
                "--enable-gpu".to_string(),
            ]
        );
        assert_eq!(store.load().unwrap(), updated);
    }

    #[test]
    fn settings_store_update_preserves_existing_unknown_fields() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        let store = SettingsStore::new(path.clone());
        std::fs::write(
            &path,
            r#"{"codexGoalsEnabled":false,"customField":{"nested":true}}"#,
        )
        .unwrap();

        let updated = store
            .update(json!({
                "codexGoalsEnabled": true
            }))
            .unwrap();
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        assert!(updated.codex_goals_enabled);
        assert_eq!(saved["codexGoalsEnabled"], json!(true));
        assert_eq!(saved["codexExtraArgs"], Value::Null);
        assert_eq!(saved["customField"], json!({"nested": true}));
    }

    #[test]
    fn settings_store_update_persists_codex_extra_args_and_preserves_unknown_fields() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        let store = SettingsStore::new(path.clone());
        std::fs::write(
            &path,
            r#"{"codexGoalsEnabled":false,"customField":{"nested":true}}"#,
        )
        .unwrap();

        let updated = store
            .update(json!({
                "codexExtraArgs": ["--force_high_performance_gpu", "--enable-features=UseOzonePlatform"]
            }))
            .unwrap();
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        assert_eq!(
            updated.codex_extra_args,
            vec![
                "--force_high_performance_gpu".to_string(),
                "--enable-features=UseOzonePlatform".to_string(),
            ]
        );
        assert_eq!(
            saved["codexExtraArgs"],
            json!([
                "--force_high_performance_gpu",
                "--enable-features=UseOzonePlatform"
            ])
        );
        assert_eq!(saved["customField"], json!({"nested": true}));
    }

    #[test]
    fn settings_store_update_with_non_object_payload_does_not_write_file() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        let store = SettingsStore::new(path.clone());
        let original = r#"{"codexGoalsEnabled":false,"customField":"keep me"}"#;
        std::fs::write(&path, original).unwrap();

        let updated = store.update(json!(null)).unwrap();

        assert!(!updated.codex_goals_enabled);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }

    #[test]
    fn settings_store_update_moves_context_tables_out_of_common_config() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));

        let updated = store
            .update(json!({
                "relayCommonConfigContents": "[mcp_servers.context7]\ncommand = \"npx\"\n"
            }))
            .unwrap();

        assert!(updated.relay_common_config_contents.is_empty());
        assert_eq!(
            updated.relay_context_config_contents,
            "[mcp_servers.context7]\ncommand = \"npx\"\n"
        );
        assert_eq!(store.load().unwrap(), updated);
    }

    #[test]
    fn settings_store_update_extracts_context_config_from_common_config() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));

        let updated = store
            .update(json!({
                "relayCommonConfigContents": "model_reasoning_effort = \"high\"\n\n[mcp_servers.context7]\ncommand = \"npx\"\n\n[plugins.\"superpowers@openai-curated\"]\nenabled = true\n"
            }))
            .unwrap();

        assert_eq!(
            updated.relay_common_config_contents,
            "model_reasoning_effort = \"high\"\n"
        );
        assert!(
            updated
                .relay_context_config_contents
                .contains("[mcp_servers.context7]")
        );
        assert!(
            updated
                .relay_context_config_contents
                .contains("[plugins.\"superpowers@openai-curated\"]")
        );
        assert_eq!(store.load().unwrap(), updated);
    }
}
