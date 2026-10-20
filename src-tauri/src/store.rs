use crate::model::{Entry, Settings, UsageStat};
use rusqlite::{params, Connection};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
};

const LOCATOR_BUDGET: usize = 8 * 1024 * 1024;

pub struct Store {
    conn: Connection,
    directory: PathBuf,
    settings: Settings,
    apps: Vec<Entry>,
}

impl Store {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, String> {
        let directory = path.as_ref().to_path_buf();
        fs::create_dir_all(&directory).map_err(|_| "Cannot create application data directory")?;
        let mut conn = Connection::open(directory.join("state.db")).map_err(db_error)?;
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(db_error)?;
        if version > 1 {
            return Err("Database requires a newer application version".into());
        }
        conn.pragma_update(None, "foreign_keys", true)
            .map_err(db_error)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(db_error)?;
        conn.pragma_update(None, "wal_autocheckpoint", 100)
            .map_err(db_error)?;
        conn.busy_timeout(std::time::Duration::from_secs(2))
            .map_err(db_error)?;
        if version == 0 {
            // 所有建表及版本升级在同一事务中，失败时保留旧数据库。
            let tx = conn.transaction().map_err(db_error)?;
            tx.execute_batch("CREATE TABLE apps(entity_id TEXT PRIMARY KEY, launch_key TEXT UNIQUE NOT NULL, data TEXT NOT NULL, manual INTEGER NOT NULL);
                CREATE TABLE aliases(entity_id TEXT PRIMARY KEY, data TEXT NOT NULL);
                CREATE TABLE pins(entity_id TEXT PRIMARY KEY, data TEXT NOT NULL, locator_bytes INTEGER NOT NULL, created_at INTEGER NOT NULL);
                CREATE TABLE usage_stats(entity_id TEXT PRIMARY KEY, data TEXT NOT NULL, locator_bytes INTEGER NOT NULL, success_count INTEGER NOT NULL, last_success_at INTEGER NOT NULL);
                CREATE INDEX usage_recency ON usage_stats(last_success_at);
                PRAGMA user_version=1;").map_err(db_error)?;
            tx.commit().map_err(db_error)?;
        }
        let primary = directory.join("config.json");
        let backup = directory.join("config.json.bak");
        let settings = if primary.exists() || backup.exists() {
            read_settings(&primary).or_else(|_| read_settings(&backup))?
        } else {
            Settings::default()
        };
        validate_settings(&settings)?;
        if read_settings(&primary).is_err() {
            atomic_json(&primary, &settings)?;
        }
        let apps = load_apps(&conn)?;
        Ok(Self {
            conn,
            directory,
            settings,
            apps,
        })
    }

    pub fn settings(&self) -> Settings {
        self.settings.clone()
    }

    pub fn preview_settings(
        &self,
        expected: u64,
        patch: serde_json::Value,
    ) -> Result<Settings, String> {
        if expected != self.settings.settings_version {
            return Err("Settings changed; reload and try again".into());
        }
        let patch = patch.as_object().ok_or("Invalid settings patch")?;
        let mut value =
            serde_json::to_value(&self.settings).map_err(|_| "Cannot serialize settings")?;
        let target = value.as_object_mut().ok_or("Invalid settings")?;
        for (key, new_value) in patch {
            if key == "settingsVersion" || !target.contains_key(key) {
                return Err("Unknown settings field".into());
            }
            target.insert(key.clone(), new_value.clone());
        }
        let mut settings: Settings =
            serde_json::from_value(value).map_err(|_| "Invalid settings value")?;
        settings.settings_version = expected
            .checked_add(1)
            .ok_or("Settings version limit reached")?;
        validate_settings(&settings)?;
        Ok(settings)
    }

    pub fn save_settings(
        &mut self,
        expected: u64,
        patch: serde_json::Value,
    ) -> Result<Settings, String> {
        let settings = self.preview_settings(expected, patch)?;
        atomic_json(&self.directory.join("config.json.bak"), &self.settings)?;
        atomic_json(&self.directory.join("config.json"), &settings)?;
        self.settings = settings;
        Ok(self.settings())
    }

    pub fn apps(&self) -> Vec<Entry> {
        self.apps.clone()
    }

    pub fn app_count(&self) -> usize {
        self.apps.len()
    }

    pub fn replace_discovered_apps(&mut self, entries: Vec<Entry>) -> Result<(), String> {
        if entries.len() > 10000 {
            return Err("Application catalog limit reached".into());
        }
        for e in &entries {
            validate_entry(e)?;
            if e.kind != "application" {
                return Err("Invalid application entry".into());
            }
        }
        let tx = self.conn.transaction().map_err(db_error)?;
        tx.execute("DELETE FROM apps WHERE manual=0", [])
            .map_err(db_error)?;
        let mut seen = HashSet::new();
        for entry in entries {
            if !seen.insert(entry.launch_key.clone()) {
                continue;
            }
            let json = entry_json(&entry)?;
            // 手动添加的定位符保留；重复来源按 launchKey 合并。
            tx.execute(
                "INSERT OR IGNORE INTO apps(entity_id,launch_key,data,manual) VALUES(?1,?2,?3,0)",
                params![entry.entity_id, entry.launch_key, json],
            )
            .map_err(db_error)?;
        }
        let apps = load_apps(&tx)?;
        tx.commit().map_err(db_error)?;
        self.apps = apps;
        Ok(())
    }

    pub fn add_manual_app(&mut self, mut entry: Entry) -> Result<(), String> {
        validate_entry(&entry)?;
        if entry.kind != "application" {
            return Err("Invalid application entry".into());
        }
        let existing = self
            .apps
            .iter()
            .find(|app| app.launch_key == entry.launch_key);
        if self.apps.len() >= 10000 && existing.is_none() {
            return Err("Application catalog limit reached".into());
        }
        if let Some(app) = existing {
            entry.entity_id = app.entity_id.clone();
        }
        entry.aliases = clean_aliases(entry.aliases)?;
        let aliases =
            serde_json::to_string(&entry.aliases).map_err(|_| "Cannot serialize aliases")?;
        let tx = self.conn.transaction().map_err(db_error)?;
        // 相同启动目标保留实体 ID，用户名称和别名在同一事务中覆盖旧来源。
        tx.execute("INSERT INTO apps(entity_id,launch_key,data,manual) VALUES(?1,?2,?3,1) ON CONFLICT(launch_key) DO UPDATE SET data=excluded.data, manual=1", params![entry.entity_id, entry.launch_key, entry_json(&entry)?]).map_err(db_error)?;
        tx.execute("INSERT INTO aliases(entity_id,data) VALUES(?1,?2) ON CONFLICT(entity_id) DO UPDATE SET data=excluded.data", params![entry.entity_id, aliases]).map_err(db_error)?;
        let apps = load_apps(&tx)?;
        tx.commit().map_err(db_error)?;
        self.apps = apps;
        Ok(())
    }

    pub fn set_aliases(&mut self, id: &str, aliases: Vec<String>) -> Result<(), String> {
        if !self.apps.iter().any(|e| e.entity_id == id) {
            return Err("Application is unavailable".into());
        }
        let aliases = clean_aliases(aliases)?;
        let json = serde_json::to_string(&aliases).map_err(|_| "Cannot serialize aliases")?;
        let tx = self.conn.transaction().map_err(db_error)?;
        tx.execute("INSERT INTO aliases(entity_id,data) VALUES(?1,?2) ON CONFLICT(entity_id) DO UPDATE SET data=excluded.data", params![id, json]).map_err(db_error)?;
        let apps = load_apps(&tx)?;
        tx.commit().map_err(db_error)?;
        self.apps = apps;
        Ok(())
    }

    pub fn pinned_entries(&self) -> Result<Vec<Entry>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT data FROM pins ORDER BY created_at,entity_id")
            .map_err(db_error)?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_error)?;
        rows.map(|row| self.current_presentation(parse_entry(&row.map_err(db_error)?)?))
            .collect()
    }

    pub fn usage_entries(&self) -> Result<Vec<(Entry, UsageStat)>, String> {
        let mut stmt = self.conn.prepare("SELECT data,success_count,last_success_at FROM usage_stats ORDER BY last_success_at DESC,entity_id").map_err(db_error)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, u64>(1)?,
                    r.get::<_, u64>(2)?,
                ))
            })
            .map_err(db_error)?;
        rows.map(|row| {
            let (json, success_count, last_success_at) = row.map_err(db_error)?;
            Ok((
                self.current_presentation(parse_entry(&json)?)?,
                UsageStat {
                    success_count,
                    last_success_at,
                },
            ))
        })
        .collect()
    }

    fn current_presentation(&self, mut entry: Entry) -> Result<Entry, String> {
        if let Some(app) = self.apps.iter().find(|e| e.entity_id == entry.entity_id) {
            // 只更新展示与搜索信息，固定项原始启动定位符保持不变。
            entry.title = app.title.clone();
            entry.subtitle = app.subtitle.clone();
            entry.aliases = app.aliases.clone();
        }
        Ok(entry)
    }

    pub fn usage(&self) -> Result<HashMap<String, UsageStat>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT entity_id,success_count,last_success_at FROM usage_stats")
            .map_err(db_error)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    UsageStat {
                        success_count: r.get(1)?,
                        last_success_at: r.get(2)?,
                    },
                ))
            })
            .map_err(db_error)?;
        rows.map(|r| r.map_err(db_error)).collect()
    }

    // 调用方只在 Shell 明确受理 Open 后调用；失败、复制和定位不记入历史。
    pub fn record_open(&mut self, entry: &Entry) -> Result<(), String> {
        if !self.settings.record_history {
            return Ok(());
        }
        validate_entry(entry)?;
        let tx = self.conn.transaction().map_err(db_error)?;
        tx.execute("INSERT INTO usage_stats(entity_id,data,locator_bytes,success_count,last_success_at) VALUES(?1,?2,?3,1,?4)
            ON CONFLICT(entity_id) DO UPDATE SET data=excluded.data, locator_bytes=excluded.locator_bytes, success_count=min(success_count+1,2147483647),last_success_at=excluded.last_success_at",
            params![entry.entity_id, entry_json(entry)?, entry.locator.len() as i64, unix_time() as i64]).map_err(db_error)?;
        loop {
            let (count, bytes): (i64, i64) = tx
                .query_row(
                    "SELECT count(*),coalesce(sum(locator_bytes),0) FROM usage_stats",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .map_err(db_error)?;
            if count <= 5000 && bytes <= LOCATOR_BUDGET as i64 {
                break;
            }
            let removed = tx.execute("DELETE FROM usage_stats WHERE entity_id=(SELECT entity_id FROM usage_stats WHERE entity_id NOT IN (SELECT entity_id FROM pins) ORDER BY last_success_at, rowid LIMIT 1)", []).map_err(db_error)?;
            if removed == 0 {
                return Err("Usage history storage limit reached".into());
            }
        }
        tx.commit().map_err(db_error)
    }

    pub fn pin(&mut self, entry: &Entry) -> Result<(), String> {
        validate_entry(entry)?;
        let tx = self.conn.transaction().map_err(db_error)?;
        let existing: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pins WHERE entity_id=?1)",
                [&entry.entity_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if existing {
            return Ok(());
        }
        let (count, bytes): (i64, i64) = tx
            .query_row(
                "SELECT count(*),coalesce(sum(locator_bytes),0) FROM pins",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(db_error)?;
        if count >= 100 || bytes as usize + entry.locator.len() > LOCATOR_BUDGET {
            return Err("Pinned items limit reached; remove an item first".into());
        }
        tx.execute(
            "INSERT INTO pins(entity_id,data,locator_bytes,created_at) VALUES(?1,?2,?3,?4)",
            params![
                entry.entity_id,
                entry_json(entry)?,
                entry.locator.len() as i64,
                unix_time() as i64
            ],
        )
        .map_err(db_error)?;
        tx.commit().map_err(db_error)
    }

    pub fn unpin(&mut self, id: &str) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM pins WHERE entity_id=?1", [id])
            .map_err(db_error)?;
        Ok(())
    }
    pub fn remove_pin(&mut self, id: &str) -> Result<(), String> {
        self.unpin(id)
    }
    pub fn clear_usage(&mut self) -> Result<usize, String> {
        self.conn
            .execute("DELETE FROM usage_stats", [])
            .map_err(db_error)
    }
}

fn db_error(_: rusqlite::Error) -> String {
    "Local data storage failed".into()
}
fn unix_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn entry_json(entry: &Entry) -> Result<String, String> {
    serde_json::to_string(entry).map_err(|_| "Cannot serialize entry".into())
}
fn parse_entry(json: &str) -> Result<Entry, String> {
    let entry: Entry = serde_json::from_str(json).map_err(|_| "Invalid stored entry")?;
    validate_entry(&entry)?;
    Ok(entry)
}

fn validate_entry(entry: &Entry) -> Result<(), String> {
    if !["application", "file", "folder"].contains(&entry.kind.as_str())
        || entry.entity_id.is_empty()
        || entry.entity_id.len() > 512
        || entry.title.is_empty()
        || entry.title.chars().count() > 512
        || entry.subtitle.chars().count() > 4096
        || entry.locator.is_empty()
        || entry.locator.encode_utf16().count() > 32767
        || entry.locator.contains('\0')
        || entry.launch_key.is_empty()
        || entry.launch_key.len() > 131072
        || !["path", "shortcut", "shell", "aumid", "packaged"].contains(&entry.launch_kind.as_str())
    {
        return Err("Invalid entry".into());
    }
    clean_aliases(entry.aliases.clone())?;
    Ok(())
}

fn clean_aliases(aliases: Vec<String>) -> Result<Vec<String>, String> {
    if aliases.len() > 16 {
        return Err("Too many application aliases".into());
    }
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for alias in aliases {
        let alias = alias.trim();
        if alias.is_empty() || alias.chars().count() > 128 || alias.chars().any(char::is_control) {
            return Err("Invalid application alias".into());
        }
        if seen.insert(crate::ranking::normalize(alias)) {
            result.push(alias.into());
        }
    }
    Ok(result)
}

fn load_apps(conn: &Connection) -> Result<Vec<Entry>, String> {
    let mut stmt = conn.prepare("SELECT apps.data,aliases.data FROM apps LEFT JOIN aliases USING(entity_id) ORDER BY apps.entity_id").map_err(db_error)?;
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
        })
        .map_err(db_error)?;
    rows.map(|r| {
        let (json, aliases) = r.map_err(db_error)?;
        let mut entry = parse_entry(&json)?;
        if let Some(json) = aliases {
            entry.aliases =
                clean_aliases(serde_json::from_str(&json).map_err(|_| "Invalid stored aliases")?)?;
        }
        Ok(entry)
    })
    .collect()
}

fn validate_settings(settings: &Settings) -> Result<(), String> {
    if serde_json::to_vec_pretty(settings)
        .map_err(|_| "Cannot serialize settings")?
        .len()
        > 256 * 1024
    {
        return Err("Settings file is too large".into());
    }
    if settings.settings_version == 0
        || !["system", "light", "dark"].contains(&settings.theme.as_str())
        || !["remember", "cursor"].contains(&settings.window_placement.as_str())
        || settings.shortcut.len() > 80
    {
        return Err("Invalid settings".into());
    }
    let tokens: Vec<_> = settings.shortcut.split('+').collect();
    let modifiers = [
        "Alt", "Control", "Ctrl", "Shift", "Super", "Meta", "Command",
    ];
    if tokens.len() < 2
        || tokens.iter().any(|p| p.is_empty())
        || tokens[..tokens.len() - 1]
            .iter()
            .any(|p| !modifiers.contains(p))
        || !tokens
            .last()
            .unwrap()
            .chars()
            .all(|c| c.is_ascii_alphanumeric())
    {
        return Err("Invalid shortcut".into());
    }
    for dirs in [&settings.include_directories, &settings.exclude_directories] {
        if dirs.len() > 32 {
            return Err("Too many directory filters".into());
        }
        for path in dirs {
            crate::query::local_directory(path)?;
        }
    }
    if !settings.everything_path.is_empty() {
        crate::query::local_directory(&settings.everything_path)?;
        if !settings
            .everything_path
            .to_ascii_lowercase()
            .ends_with(".exe")
        {
            return Err("Select an Everything executable".into());
        }
    }
    Ok(())
}

fn read_settings(path: &Path) -> Result<Settings, String> {
    let metadata = fs::metadata(path).map_err(|_| "Cannot read settings")?;
    if metadata.len() > 256 * 1024 {
        return Err("Settings file is too large".into());
    }
    let settings: Settings =
        serde_json::from_slice(&fs::read(path).map_err(|_| "Cannot read settings")?)
            .map_err(|_| "Settings are damaged; restore a backup")?;
    validate_settings(&settings)?;
    Ok(settings)
}

fn atomic_json(path: &Path, settings: &Settings) -> Result<(), String> {
    let temp = path.with_extension("tmp");
    let json = serde_json::to_vec_pretty(settings).map_err(|_| "Cannot serialize settings")?;
    let mut file = fs::File::create(&temp).map_err(|_| "Cannot write settings")?;
    file.write_all(&json)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Cannot write settings")?;
    drop(file);
    atomic_replace(&temp, path).map_err(|_| "Cannot save settings".into())
}

#[cfg(windows)]
fn atomic_replace(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    // 缓冲区以 NUL 结尾且在调用期间有效；原子替换并等待落盘。
    if unsafe { MoveFileExW(source.as_ptr(), target.as_ptr(), 0x1 | 0x8) } == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn atomic_replace(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(source, target)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window_placement_is_validated_and_persists_across_reopen() {
        let f = Fixture::new();
        let mut store = Store::new(&f.0).unwrap();
        let version = store.settings().settings_version;
        assert!(store.save_settings(version, serde_json::json!({"windowPlacement":"active-app"})).is_err());
        let saved = store.save_settings(version, serde_json::json!({"windowPlacement":"cursor"})).unwrap();
        assert_eq!(saved.window_placement, "cursor");
        drop(store);
        assert_eq!(Store::new(&f.0).unwrap().settings().window_placement, "cursor");
    }
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "supersearch-domain-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn entry(n: usize) -> Entry {
        Entry {
            entity_id: format!("e{n}"),
            kind: "application".into(),
            title: format!("App {n}"),
            subtitle: String::new(),
            locator: format!(r"C:\Apps\{n}.exe"),
            launch_kind: "path".into(),
            aliases: vec![],
            launch_key: format!("key{n}"),
        }
    }
    #[test]
    fn settings_version_whitelist_backup_and_recovery() {
        let f = Fixture::new();
        let mut s = Store::new(&f.0).unwrap();
        let version = s.settings().settings_version;
        assert!(s
            .save_settings(version + 1, serde_json::json!({"theme":"dark"}))
            .is_err());
        assert!(s
            .save_settings(version, serde_json::json!({"bogus":true}))
            .is_err());
        assert!(s
            .save_settings(
                version,
                serde_json::json!({"includeDirectories":["relative"]})
            )
            .is_err());
        let changed = s
            .save_settings(
                version,
                serde_json::json!({"theme":"dark","recordHistory":false}),
            )
            .unwrap();
        assert_eq!(changed.settings_version, version + 1);
        assert!(f.0.join("config.json.bak").exists());
        drop(s);
        fs::write(f.0.join("config.json"), "corrupt").unwrap();
        let recovered = Store::new(&f.0).unwrap();
        assert_eq!(recovered.settings().theme, "light");
    }
    #[test]
    fn app_refresh_is_atomic_and_preserves_manual_apps_and_aliases() {
        let f = Fixture::new();
        let mut s = Store::new(&f.0).unwrap();
        s.replace_discovered_apps(vec![entry(0)]).unwrap();
        s.add_manual_app(entry(1)).unwrap();
        s.set_aliases("e0", vec!["中文别名".into()]).unwrap();
        let mut bad = entry(2);
        bad.locator = String::new();
        assert!(s.replace_discovered_apps(vec![bad]).is_err());
        assert_eq!(s.apps().len(), 2);
        s.replace_discovered_apps(vec![entry(0)]).unwrap();
        assert_eq!(
            s.apps()
                .iter()
                .find(|e| e.entity_id == "e0")
                .unwrap()
                .aliases,
            ["中文别名"]
        );
        drop(s);
        assert_eq!(Store::new(&f.0).unwrap().apps().len(), 2);
    }
    #[test]
    fn history_can_be_disabled_cleared_and_pins_are_separate() {
        let f = Fixture::new();
        let mut s = Store::new(&f.0).unwrap();
        let e = entry(0);
        s.pin(&e).unwrap();
        s.record_open(&e).unwrap();
        s.record_open(&e).unwrap();
        assert_eq!(s.usage().unwrap()["e0"].success_count, 2);
        s.save_settings(
            s.settings().settings_version,
            serde_json::json!({"recordHistory":false}),
        )
        .unwrap();
        s.record_open(&entry(1)).unwrap();
        assert_eq!(s.usage().unwrap().len(), 1);
        assert_eq!(s.clear_usage().unwrap(), 1);
        assert_eq!(s.pinned_entries().unwrap().len(), 1);
        s.remove_pin("e0").unwrap();
        assert!(s.pinned_entries().unwrap().is_empty());
    }
    #[test]
    fn enforces_pin_and_usage_limits_without_evicting_pins() {
        let f = Fixture::new();
        let mut s = Store::new(&f.0).unwrap();
        for n in 0..100 {
            s.pin(&entry(n)).unwrap();
        }
        assert!(s.pin(&entry(100)).is_err());
        s.record_open(&entry(0)).unwrap();
        for n in 100..5101 {
            s.record_open(&entry(n)).unwrap();
        }
        assert_eq!(s.usage().unwrap().len(), 5000);
        assert!(s.usage().unwrap().contains_key("e0"));
        let mut huge = entry(9999);
        huge.locator = "x".repeat(8 * 1024 * 1024 + 1);
        assert!(s.pin(&huge).is_err());
    }
    #[test]
    fn rejects_higher_schema_without_overwriting_data() {
        let f = Fixture::new();
        let c = Connection::open(f.0.join("state.db")).unwrap();
        c.pragma_update(None, "user_version", 99).unwrap();
        drop(c);
        assert!(Store::new(&f.0).is_err());
        let c = Connection::open(f.0.join("state.db")).unwrap();
        let version: i64 = c
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, 99);
    }
    #[test]
    fn retains_packaged_shell_descriptor_and_current_aliases_on_pins() {
        let f = Fixture::new();
        let mut s = Store::new(&f.0).unwrap();
        let mut app = entry(0);
        app.launch_kind = "shell".into();
        app.locator = "shell:AppsFolder\\Package!App".into();
        s.replace_discovered_apps(vec![app.clone()]).unwrap();
        s.pin(&app).unwrap();
        s.set_aliases("e0", vec!["packaged".into()]).unwrap();
        assert_eq!(s.pinned_entries().unwrap()[0].locator, app.locator);
        assert_eq!(s.pinned_entries().unwrap()[0].aliases, ["packaged"]);
        s.record_open(&app).unwrap();
        assert_eq!(s.usage_entries().unwrap()[0].0.locator, app.locator);
        s.unpin("e0").unwrap();
        assert!(s.pinned_entries().unwrap().is_empty());
    }
    #[test]
    fn enforces_logical_locator_budget_before_row_limit() {
        let f = Fixture::new();
        let mut s = Store::new(&f.0).unwrap();
        for n in 0..100 {
            let mut e = entry(n);
            e.locator = "字".repeat(32700);
            s.record_open(&e).unwrap();
        }
        assert!(s.usage().unwrap().len() < 100);
        let total: usize = s
            .usage_entries()
            .unwrap()
            .iter()
            .map(|(e, _)| e.locator.len())
            .sum();
        assert!(total <= LOCATOR_BUDGET);
        let mut rejected = false;
        for n in 0..100 {
            let mut e = entry(n);
            e.locator = "字".repeat(32700);
            if s.pin(&e).is_err() {
                rejected = true;
                break;
            }
        }
        assert!(rejected);
        assert!(s.pinned_entries().unwrap().len() < 100);
    }
    #[test]
    fn rejects_settings_larger_than_recovery_reader_limit() {
        let f = Fixture::new();
        let mut s = Store::new(&f.0).unwrap();
        let dirs: Vec<String> = (0..32)
            .map(|n| format!("C:\\{n}{}", "a".repeat(9000)))
            .collect();
        let before = s.settings();
        assert!(s
            .save_settings(
                before.settings_version,
                serde_json::json!({"includeDirectories":dirs})
            )
            .is_err());
        assert_eq!(s.settings(), before);
    }
    #[test]
    fn manual_override_preserves_identity_and_new_name_aliases_across_refresh() {
        let f = Fixture::new();
        let mut s = Store::new(&f.0).unwrap();
        let discovered = entry(0);
        s.replace_discovered_apps(vec![discovered.clone()]).unwrap();
        s.set_aliases("e0", vec!["old alias".into()]).unwrap();
        s.pin(&discovered).unwrap();
        s.record_open(&discovered).unwrap();
        let mut manual = discovered.clone();
        manual.entity_id = "new-id".into();
        manual.title = "My custom editor".into();
        manual.aliases = vec!["editor".into(), " 编辑器 ".into()];
        manual.subtitle = "Manual application".into();
        s.add_manual_app(manual).unwrap();
        let current = s.apps();
        assert_eq!(current.len(), 1);
        assert_eq!(current[0].entity_id, "e0");
        assert_eq!(current[0].title, "My custom editor");
        assert_eq!(current[0].aliases, ["editor", "编辑器"]);
        assert_eq!(s.pinned_entries().unwrap()[0].entity_id, "e0");
        assert_eq!(s.pinned_entries().unwrap()[0].title, "My custom editor");
        assert_eq!(s.usage_entries().unwrap()[0].0.title, "My custom editor");
        assert_eq!(s.usage().unwrap()["e0"].success_count, 1);
        s.replace_discovered_apps(vec![discovered]).unwrap();
        assert_eq!(s.apps()[0].title, "My custom editor");
        assert_eq!(s.apps()[0].aliases, ["editor", "编辑器"]);
        drop(s);
        let reopened = Store::new(&f.0).unwrap();
        assert_eq!(reopened.apps()[0].title, "My custom editor");
        assert_eq!(reopened.apps()[0].entity_id, "e0");
    }
    #[test]
    fn preview_validates_without_writes_and_full_settings_rollback_is_monotonic() {
        let f = Fixture::new();
        let mut s = Store::new(&f.0).unwrap();
        let old = s.settings();
        let contents = fs::read(f.0.join("config.json")).unwrap();
        let patch = serde_json::json!({"shortcut":"Control+Space","autostart":true,"theme":"dark","recordHistory":false,"includeDesktop":true,"includeDirectories":["C:\\Work"]});
        let preview = s
            .preview_settings(old.settings_version, patch.clone())
            .unwrap();
        assert_eq!(preview.settings_version, old.settings_version + 1);
        assert_eq!(preview.theme, "dark");
        assert_eq!(s.settings(), old);
        assert_eq!(fs::read(f.0.join("config.json")).unwrap(), contents);
        assert!(!f.0.join("config.json.bak").exists());
        assert!(s
            .preview_settings(old.settings_version + 1, patch.clone())
            .is_err());
        assert!(s
            .preview_settings(
                old.settings_version,
                serde_json::json!({"shortcut":"Control+Space","theme":"invalid"})
            )
            .is_err());
        let saved = s.save_settings(old.settings_version, patch).unwrap();
        assert_eq!(saved, preview);
        let mut rollback = serde_json::to_value(&old).unwrap();
        rollback.as_object_mut().unwrap().remove("settingsVersion");
        let restored = s.save_settings(saved.settings_version, rollback).unwrap();
        let mut expected = old;
        expected.settings_version = saved.settings_version + 1;
        assert_eq!(restored, expected);
        drop(s);
        assert_eq!(Store::new(&f.0).unwrap().settings(), expected);
    }
}
