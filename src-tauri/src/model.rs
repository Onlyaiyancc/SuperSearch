use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub entity_id: String,
    pub kind: String,
    pub title: String,
    pub subtitle: String,
    pub locator: String,
    pub launch_kind: String,
    pub aliases: Vec<String>,
    pub launch_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultItem {
    pub result_id: String,
    pub entity_id: String,
    pub kind: String,
    pub title: String,
    pub subtitle: String,
    pub path: String,
    pub match_tier: u8,
    pub pinned: bool,
    pub available: bool,
    pub actions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub id: String,
    pub status: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchSnapshot {
    pub request_id: String,
    pub snapshot_revision: u64,
    pub scope: String,
    pub terms: Vec<String>,
    pub directory: Option<String>,
    pub folder_completions: Vec<FolderCompletion>,
    pub complete: bool,
    pub items: Vec<ResultItem>,
    pub providers: Vec<ProviderStatus>,
    pub truncated: bool,
    pub has_more: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FolderCompletion {
    pub title: String,
    pub path: String,
    pub query: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Settings {
    pub settings_version: u64,
    pub shortcut: String,
    pub autostart: bool,
    pub record_history: bool,
    pub include_desktop: bool,
    pub hide_hidden: bool,
    pub hide_system: bool,
    pub include_directories: Vec<String>,
    pub exclude_directories: Vec<String>,
    pub everything_path: String,
    pub auto_start_everything: bool,
    pub theme: String,
    pub window_placement: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            settings_version: 1,
            shortcut: "Alt+Space".into(),
            autostart: false,
            record_history: true,
            include_desktop: false,
            hide_hidden: true,
            hide_system: true,
            include_directories: vec![],
            exclude_directories: vec![],
            everything_path: String::new(),
            auto_start_everything: false,
            theme: "light".into(),
            window_placement: "remember".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageStat {
    pub success_count: u64,
    pub last_success_at: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_accept_cursor_placement_and_keep_missing_field_compatible() {
        let value: Settings = serde_json::from_str(r#"{"windowPlacement":"cursor"}"#).unwrap();
        assert_eq!(serde_json::to_value(value).unwrap()["windowPlacement"], "cursor");
        let old: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(serde_json::to_value(old).unwrap()["windowPlacement"], "remember");
    }
    #[test]
    fn settings_default_to_private_opt_in_startup() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings.shortcut, "Alt+Space");
        assert!(!settings.autostart);
        assert!(settings.record_history);
        assert!(settings.hide_hidden && settings.hide_system);
        assert_eq!(
            serde_json::to_value(settings).unwrap()["settingsVersion"],
            1
        );
    }
}
