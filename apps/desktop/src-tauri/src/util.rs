#![allow(unused_imports)]

use chrono::Local;
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;
use toml_edit::{value, DocumentMut, Item, Table};

use crate::{INSTRUCTION_FILENAME, INSTRUCTION_RELATIVE, INSTRUCTION_CONTENT, INSTRUCTION_54_FILENAME, INSTRUCTION_54_RELATIVE, INSTRUCTION_54_CONTENT, CodexxError, Result, ProviderSummary, CodexState, BackupMeta, BackupEntry, ProviderInput, ProviderTomlInput, OfficialConfigInput, SavedProvider, SavedPrompt, ImportResult, OfficialAuthCandidate, ActionResult, AboutInfo, SessionPreview, SessionSyncStatus, SessionSyncResult, RolloutScan, SqliteScan};


pub fn io_err(path: &Path, source: std::io::Error) -> CodexxError {
    CodexxError::Io {
        path: path.display().to_string(),
        source,
    }
}

pub fn json_err(path: &Path, source: serde_json::Error) -> CodexxError {
    CodexxError::Json {
        path: path.display().to_string(),
        source,
    }
}

pub fn home_dir() -> Result<PathBuf> {
    dirs::home_dir().ok_or(CodexxError::NoHomeDir)
}

pub fn app_home() -> Result<PathBuf> {
    if let Ok(value) = std::env::var("CODEXX_HOME") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    Ok(home_dir()?.join(".codexx"))
}

pub fn now_rfc3339() -> String {
    Local::now().to_rfc3339()
}

pub fn normalize_prompt_filename(input: &str, fallback: &str) -> String {
    let raw = input.trim().trim_end_matches(".md");
    let base = if raw.is_empty() { fallback } else { raw };
    let mut out = String::new();
    let mut last_dash = false;
    for ch in base.to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let out = out.trim_matches('-');
    format!("{}.md", if out.is_empty() { "custom-prompt" } else { out })
}

pub fn sanitize_id(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.trim().to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        format!("provider-{}", Local::now().timestamp_millis())
    } else {
        out
    }
}

pub fn extract_ccswitch_codex_provider(
    id: &str,
    name: &str,
    settings_config: &str,
) -> Option<SavedProvider> {
    let settings: Value = serde_json::from_str(settings_config).ok()?;
    let auth = settings.get("auth");
    let api_key = auth
        .and_then(|v| v.get("OPENAI_API_KEY"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);

    let config_text = settings.get("config").and_then(Value::as_str).unwrap_or("");
    if config_text.trim().is_empty() {
        return None;
    }
    let doc = config_text.parse::<DocumentMut>().ok()?;
    let model = string_value(&doc, "model").unwrap_or_else(|| "gpt-5.5".to_string());
    let active_provider =
        string_value(&doc, "model_provider").unwrap_or_else(|| "custom".to_string());

    let provider_table = doc
        .get("model_providers")
        .and_then(|item| item.as_table())
        .and_then(|providers| providers.get(&active_provider))
        .and_then(|item| item.as_table());

    let base_url = provider_table
        .and_then(|table| table.get("base_url"))
        .and_then(|item| item.as_str())
        .or_else(|| doc.get("base_url").and_then(|item| item.as_str()))
        .map(str::trim)
        .filter(|s| !s.is_empty())?
        .trim_end_matches('/')
        .to_string();

    let provider_name = provider_table
        .and_then(|table| table.get("name"))
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(name)
        .to_string();

    let wire_api = provider_table
        .and_then(|table| table.get("wire_api"))
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("responses")
        .to_string();

    let requires_openai_auth = provider_table
        .and_then(|table| table.get("requires_openai_auth"))
        .and_then(|item| item.as_bool())
        .unwrap_or(true);

    Some(SavedProvider {
        id: sanitize_id(id),
        provider_name,
        base_url,
        model,
        api_key,
        wire_api,
        requires_openai_auth,
    })
}

pub fn push_existing_candidate(candidates: &mut Vec<PathBuf>, candidate: Option<PathBuf>) {
    let Some(path) = candidate else {
        return;
    };
    if !candidates.iter().any(|item| item == &path) {
        candidates.push(path);
    }
}

pub fn ccswitch_db_candidates() -> Result<Vec<PathBuf>> {
    let mut candidates = Vec::new();

    if let Ok(value) = std::env::var("CC_SWITCH_HOME") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            push_existing_candidate(
                &mut candidates,
                Some(PathBuf::from(trimmed).join("cc-switch.db")),
            );
        }
    }

    let home = home_dir()?;
    // cc-switch 当前主要使用这个位置，macOS/Windows/Linux 都适用。
    push_existing_candidate(
        &mut candidates,
        Some(home.join(".cc-switch").join("cc-switch.db")),
    );

    // 兼容 Tauri/AppData 风格位置，防止未来或不同发行版变更数据目录。
    if let Some(data_dir) = dirs::data_dir() {
        push_existing_candidate(
            &mut candidates,
            Some(data_dir.join("com.ccswitch.desktop").join("cc-switch.db")),
        );
        push_existing_candidate(
            &mut candidates,
            Some(data_dir.join("cc-switch").join("cc-switch.db")),
        );
        push_existing_candidate(
            &mut candidates,
            Some(data_dir.join("CC Switch").join("cc-switch.db")),
        );
    }
    if let Some(data_local_dir) = dirs::data_local_dir() {
        push_existing_candidate(
            &mut candidates,
            Some(
                data_local_dir
                    .join("com.ccswitch.desktop")
                    .join("cc-switch.db"),
            ),
        );
        push_existing_candidate(
            &mut candidates,
            Some(data_local_dir.join("cc-switch").join("cc-switch.db")),
        );
        push_existing_candidate(
            &mut candidates,
            Some(data_local_dir.join("CC Switch").join("cc-switch.db")),
        );
    }

    #[cfg(target_os = "macos")]
    {
        push_existing_candidate(
            &mut candidates,
            Some(
                home.join("Library")
                    .join("Application Support")
                    .join("com.ccswitch.desktop")
                    .join("cc-switch.db"),
            ),
        );
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            push_existing_candidate(
                &mut candidates,
                Some(
                    PathBuf::from(appdata)
                        .join("com.ccswitch.desktop")
                        .join("cc-switch.db"),
                ),
            );
        }
        if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
            push_existing_candidate(
                &mut candidates,
                Some(
                    PathBuf::from(localappdata)
                        .join("com.ccswitch.desktop")
                        .join("cc-switch.db"),
                ),
            );
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg_data_home) = std::env::var("XDG_DATA_HOME") {
            push_existing_candidate(
                &mut candidates,
                Some(
                    PathBuf::from(xdg_data_home)
                        .join("com.ccswitch.desktop")
                        .join("cc-switch.db"),
                ),
            );
        }
        push_existing_candidate(
            &mut candidates,
            Some(
                home.join(".local")
                    .join("share")
                    .join("com.ccswitch.desktop")
                    .join("cc-switch.db"),
            ),
        );
    }

    Ok(candidates)
}

pub fn default_ccswitch_db_path() -> Result<PathBuf> {
    let candidates = ccswitch_db_candidates()?;
    candidates
        .iter()
        .find(|path| path.exists())
        .cloned()
        .or_else(|| candidates.into_iter().next())
        .ok_or_else(|| CodexxError::Config("无法生成 cc-switch 数据库候选路径".to_string()))
}

pub fn default_codex_dir() -> Result<PathBuf> {
    if let Ok(value) = std::env::var("CODEX_HOME") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    Ok(home_dir()?.join(".codex"))
}

pub fn resolve_codex_dir(config_dir: Option<String>) -> Result<PathBuf> {
    match config_dir
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        Some(path) => Ok(PathBuf::from(path)),
        None => default_codex_dir(),
    }
}

pub fn config_path(codex_dir: &Path) -> PathBuf {
    codex_dir.join("config.toml")
}

pub fn auth_path(codex_dir: &Path) -> PathBuf {
    codex_dir.join("auth.json")
}

pub fn read_to_string_if_exists(path: &Path) -> Result<String> {
    if !path.exists() {
        return Ok(String::new());
    }
    fs::read_to_string(path).map_err(|e| io_err(path, e))
}

pub fn parse_toml_document(path: &Path, text: &str) -> Result<DocumentMut> {
    if text.trim().is_empty() {
        return Ok(DocumentMut::new());
    }
    text.parse::<DocumentMut>().map_err(|e| CodexxError::Toml {
        path: path.display().to_string(),
        message: e.to_string(),
    })
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
    }
    let tmp = path.with_extension(format!(
        "tmp.{}.{}",
        std::process::id(),
        Local::now().format("%Y%m%d%H%M%S%3f")
    ));
    {
        let mut file = fs::File::create(&tmp).map_err(|e| io_err(&tmp, e))?;
        file.write_all(bytes).map_err(|e| io_err(&tmp, e))?;
        file.sync_all().map_err(|e| io_err(&tmp, e))?;
    }
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path).map_err(|e| io_err(path, e))?;
    }
    fs::rename(&tmp, path).map_err(|e| io_err(path, e))?;
    Ok(())
}

pub fn write_text(path: &Path, text: &str) -> Result<()> {
    atomic_write(path, text.as_bytes())
}

pub fn write_json(path: &Path, value: &Value) -> Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(|e| json_err(path, e))?;
    write_text(path, &(text + "\n"))
}

pub fn backup_root() -> Result<PathBuf> {
    Ok(app_home()?.join("backups"))
}

pub fn create_backup(codex_dir: &Path, action: &str) -> Result<Option<String>> {
    let cfg = config_path(codex_dir);
    let auth = auth_path(codex_dir);
    let had_config = cfg.exists();
    let had_auth = auth.exists();

    if !had_config && !had_auth {
        return Ok(None);
    }

    let id = format!("{}-{}", Local::now().format("%Y%m%d-%H%M%S"), action);
    let dir = backup_root()?.join(&id);
    fs::create_dir_all(&dir).map_err(|e| io_err(&dir, e))?;

    if had_config {
        fs::copy(&cfg, dir.join("config.toml")).map_err(|e| io_err(&cfg, e))?;
    }
    if had_auth {
        fs::copy(&auth, dir.join("auth.json")).map_err(|e| io_err(&auth, e))?;
    }

    let meta = BackupMeta {
        id: id.clone(),
        action: action.to_string(),
        created_at: Local::now().to_rfc3339(),
        codex_dir: codex_dir.display().to_string(),
        config_path: cfg.display().to_string(),
        auth_path: auth.display().to_string(),
        had_config,
        had_auth,
    };
    write_json(
        &dir.join("meta.json"),
        &serde_json::to_value(meta).expect("meta serialize"),
    )?;
    Ok(Some(id))
}

pub fn read_backup_entry(dir: &Path) -> Option<BackupEntry> {
    let meta_path = dir.join("meta.json");
    let text = fs::read_to_string(&meta_path).ok()?;
    let meta: BackupMeta = serde_json::from_str(&text).ok()?;
    Some(BackupEntry {
        id: meta.id,
        action: meta.action,
        created_at: meta.created_at,
        path: dir.display().to_string(),
        had_config: meta.had_config,
        had_auth: meta.had_auth,
    })
}

pub fn backups() -> Result<Vec<BackupEntry>> {
    let root = backup_root()?;
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(&root).map_err(|e| io_err(&root, e))? {
        let entry = entry.map_err(|e| io_err(&root, e))?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(backup) = read_backup_entry(&path) {
                entries.push(backup);
            }
        }
    }
    entries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(entries)
}

pub fn latest_backup() -> Result<Option<BackupEntry>> {
    Ok(backups()?.into_iter().next())
}

pub fn redacted_auth_preview(path: &Path) -> Result<Option<Value>> {
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path).map_err(|e| io_err(path, e))?;
    let mut value: Value = serde_json::from_str(&text).map_err(|e| json_err(path, e))?;
    if let Some(obj) = value.as_object_mut() {
        for (key, val) in obj.iter_mut() {
            let lower = key.to_ascii_lowercase();
            if lower.contains("key")
                || lower.contains("token")
                || lower.contains("secret")
                || lower.contains("password")
            {
                if val.as_str().is_some_and(|s| !s.trim().is_empty()) {
                    *val = Value::String("••••••••".to_string());
                }
            }
        }
    }
    Ok(Some(value))
}

pub fn auth_has_material(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let text = fs::read_to_string(path).map_err(|e| io_err(path, e))?;
    let value: Value = serde_json::from_str(&text).map_err(|e| json_err(path, e))?;
    let Some(obj) = value.as_object() else {
        return Ok(false);
    };
    Ok(obj.iter().any(|(key, value)| {
        if key == "auth_mode" {
            return false;
        }
        match value {
            Value::Null => false,
            Value::String(s) => !s.trim().is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Object(o) => !o.is_empty(),
            _ => true,
        }
    }))
}

pub fn string_value(doc: &DocumentMut, key: &str) -> Option<String> {
    doc.get(key)
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

pub fn bool_from_item(item: Option<&Item>) -> Option<bool> {
    item.and_then(|i| i.as_bool())
}

pub fn extract_providers(doc: &DocumentMut, current: Option<&str>) -> Vec<ProviderSummary> {
    let Some(providers) = doc.get("model_providers").and_then(|i| i.as_table()) else {
        return Vec::new();
    };

    providers
        .iter()
        .filter_map(|(id, item)| {
            let table = item.as_table()?;
            Some(ProviderSummary {
                id: id.to_string(),
                name: table
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string),
                base_url: table
                    .get("base_url")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string),
                wire_api: table
                    .get("wire_api")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string),
                requires_openai_auth: bool_from_item(table.get("requires_openai_auth")),
                is_current: current.is_some_and(|c| c == id),
            })
        })
        .collect()
}

pub fn build_state(codex_dir: PathBuf) -> Result<CodexState> {
    let cfg = config_path(&codex_dir);
    let auth = auth_path(&codex_dir);
    let text = read_to_string_if_exists(&cfg)?;
    let doc = parse_toml_document(&cfg, &text)?;
    let model = string_value(&doc, "model");
    let model_provider = string_value(&doc, "model_provider");
    let instruction_file = string_value(&doc, "model_instructions_file");
    let instruction_enabled = instruction_file
        .as_deref()
        .is_some_and(is_managed_instruction_value);
    let providers = extract_providers(&doc, model_provider.as_deref());

    Ok(CodexState {
        codex_dir: codex_dir.display().to_string(),
        config_path: cfg.display().to_string(),
        auth_path: auth.display().to_string(),
        config_exists: cfg.exists(),
        auth_exists: auth.exists(),
        official_auth_available: auth_has_material(&auth)?,
        model,
        model_provider,
        instruction_file,
        instruction_enabled,
        providers,
        config_text: text,
        auth_preview: redacted_auth_preview(&auth)?,
        auth_text: read_to_string_if_exists(&auth)?,
        last_backup: latest_backup()?,
    })
}

pub fn current_model_provider(codex_dir: &Path, explicit: Option<String>) -> Result<String> {
    if let Some(provider) = explicit
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        return Ok(provider);
    }
    let cfg = config_path(codex_dir);
    let text = read_to_string_if_exists(&cfg)?;
    let doc = parse_toml_document(&cfg, &text)?;
    Ok(string_value(&doc, "model_provider").unwrap_or_else(|| "openai".to_string()))
}

pub fn is_rollout_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("rollout-") && name.ends_with(".jsonl"))
}

pub fn collect_rollout_paths(root: &Path, out: &mut Vec<PathBuf>, warnings: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(root) else {
        if root.exists() {
            warnings.push(format!("无法读取目录: {}", root.display()));
        }
        return;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if path.is_dir() {
            collect_rollout_paths(&path, out, warnings);
        } else if is_rollout_file(&path) {
            out.push(path);
        }
    }
}

pub fn split_line_ending(segment: &str) -> (&str, &str) {
    if let Some(line) = segment.strip_suffix("\r\n") {
        (line, "\r\n")
    } else if let Some(line) = segment.strip_suffix('\n') {
        (line, "\n")
    } else {
        (segment, "")
    }
}

pub fn normalize_workspace_path(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with(r"\\?\unc\") {
        return Some(format!(r"\\{}", trimmed[8..].replace('/', r"\")));
    }
    if trimmed.starts_with(r"\\?\") {
        return Some(trimmed[4..].replace('\\', "/"));
    }
    Some(trimmed.to_string())
}

pub fn scan_rollouts(codex_dir: &Path, target_provider: &str, rewrite: bool) -> Result<RolloutScan> {
    let mut paths = Vec::new();
    let mut scan = RolloutScan::default();
    for dir in ["sessions", "archived_sessions"] {
        collect_rollout_paths(&codex_dir.join(dir), &mut paths, &mut scan.warnings);
    }
    paths.sort();
    scan.rollout_files = paths.len();

    for path in paths {
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e)
                if matches!(e.kind(), std::io::ErrorKind::PermissionDenied)
                    || matches!(e.raw_os_error(), Some(32 | 33)) =>
            {
                scan.warnings
                    .push(format!("跳过被占用/无权限会话文件: {}", path.display()));
                continue;
            }
            Err(e) => return Err(io_err(&path, e)),
        };
        let mut next = String::with_capacity(text.len());
        let mut file_has_meta = false;
        let mut file_changed = false;
        let has_user_event = text.contains("\"user_message\"") || text.contains("\"user_input\"");
        let mut first_thread_id: Option<String> = None;
        let mut first_cwd: Option<String> = None;

        for segment in text.split_inclusive('\n') {
            let (line, ending) = split_line_ending(segment);
            let mut next_line = line.to_string();
            if !line.trim().is_empty() {
                if let Ok(mut record) = serde_json::from_str::<Value>(line) {
                    if record.get("type").and_then(Value::as_str) == Some("session_meta") {
                        file_has_meta = true;
                        scan.session_meta_count += 1;
                        if let Some(payload) =
                            record.get_mut("payload").and_then(Value::as_object_mut)
                        {
                            if first_thread_id.is_none() {
                                first_thread_id = payload
                                    .get("id")
                                    .and_then(Value::as_str)
                                    .map(ToString::to_string);
                            }
                            if first_cwd.is_none() {
                                first_cwd = payload
                                    .get("cwd")
                                    .and_then(Value::as_str)
                                    .and_then(normalize_workspace_path);
                            }
                            if payload.get("model_provider").and_then(Value::as_str)
                                != Some(target_provider)
                            {
                                scan.mismatched_session_meta += 1;
                                file_changed = true;
                                if rewrite {
                                    payload.insert(
                                        "model_provider".to_string(),
                                        json!(target_provider),
                                    );
                                    next_line = serde_json::to_string(&record)
                                        .map_err(|e| json_err(&path, e))?;
                                }
                            }
                        }
                    }
                }
            }
            next.push_str(&next_line);
            next.push_str(ending);
        }
        if file_has_meta {
            if has_user_event {
                if let Some(id) = &first_thread_id {
                    scan.thread_ids_with_user_events.insert(id.clone());
                }
            }
            if let (Some(id), Some(cwd)) = (&first_thread_id, &first_cwd) {
                scan.cwd_by_thread_id.insert(id.clone(), cwd.clone());
            }
        }
        if file_changed {
            scan.mismatched_rollouts += 1;
            if rewrite {
                scan.changed_files.push((path, next));
            }
        }
    }
    Ok(scan)
}

pub fn sqlite_candidate_paths(codex_dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let sqlite_dir = codex_dir.join("sqlite");
    if let Ok(entries) = fs::read_dir(&sqlite_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|v| v.to_str()).unwrap_or("");
            if matches!(ext, "db" | "sqlite" | "sqlite3") {
                paths.push(path);
            }
        }
    }
    paths.sort();
    let legacy = codex_dir.join("state_5.sqlite");
    if legacy.exists() && !paths.iter().any(|p| p == &legacy) {
        paths.push(legacy);
    }
    paths
}

pub fn provider_sync_backup_root(codex_dir: &Path) -> PathBuf {
    codex_dir.join("backups_state").join("provider-sync")
}

pub fn copy_file_to_backup(codex_dir: &Path, backup_dir: &Path, source: &Path) -> Result<()> {
    if !source.exists() {
        return Ok(());
    }
    let relative = source.strip_prefix(codex_dir).unwrap_or(source);
    let target = backup_dir.join(relative);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
    }
    fs::copy(source, &target).map_err(|e| io_err(source, e))?;
    Ok(())
}

pub fn prune_provider_sync_backups(codex_dir: &Path) -> Result<()> {
    let root = provider_sync_backup_root(codex_dir);
    if !root.exists() {
        return Ok(());
    }
    let mut dirs = Vec::new();
    for entry in fs::read_dir(&root).map_err(|e| io_err(&root, e))? {
        let entry = entry.map_err(|e| io_err(&root, e))?;
        let path = entry.path();
        if path.is_dir() && path.join("metadata.json").exists() {
            dirs.push(path);
        }
    }
    dirs.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    for path in dirs.into_iter().skip(5) {
        let _ = fs::remove_dir_all(path);
    }
    Ok(())
}

pub fn create_provider_sync_backup(
    codex_dir: &Path,
    target_provider: &str,
    changed_rollouts: &[PathBuf],
) -> Result<PathBuf> {
    let root = provider_sync_backup_root(codex_dir);
    fs::create_dir_all(&root).map_err(|e| io_err(&root, e))?;
    let mut backup_dir = root.join(Local::now().format("%Y%m%d%H%M%S").to_string());
    let mut suffix = 0;
    while backup_dir.exists() {
        suffix += 1;
        backup_dir = root.join(format!("{}-{suffix}", Local::now().format("%Y%m%d%H%M%S")));
    }
    fs::create_dir_all(&backup_dir).map_err(|e| io_err(&backup_dir, e))?;

    for name in [
        "config.toml",
        ".codex-global-state.json",
        ".codex-global-state.json.bak",
    ] {
        copy_file_to_backup(codex_dir, &backup_dir, &codex_dir.join(name))?;
    }
    for path in sqlite_candidate_paths(codex_dir) {
        for candidate in [
            path.clone(),
            PathBuf::from(format!("{}-wal", path.display())),
            PathBuf::from(format!("{}-shm", path.display())),
        ] {
            copy_file_to_backup(codex_dir, &backup_dir, &candidate)?;
        }
    }
    for path in changed_rollouts {
        copy_file_to_backup(codex_dir, &backup_dir, path)?;
    }
    write_json(
        &backup_dir.join("metadata.json"),
        &json!({
            "version": 1,
            "namespace": "provider-sync",
            "managedBy": "Codex-X session manager",
            "codexHome": codex_dir.display().to_string(),
            "targetProvider": target_provider,
            "createdAt": Local::now().to_rfc3339(),
            "changedRolloutFiles": changed_rollouts.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
        }),
    )?;
    prune_provider_sync_backups(codex_dir)?;
    Ok(backup_dir)
}

pub fn instruction_template(template_id: &str) -> Result<(&'static str, &'static str, &'static str)> {
    match template_id.trim() {
        "gpt5.4-unrestricted" => Ok((
            INSTRUCTION_54_FILENAME,
            INSTRUCTION_54_RELATIVE,
            INSTRUCTION_54_CONTENT,
        )),
        "gpt5.5-unrestricted" | "" => Ok((
            INSTRUCTION_FILENAME,
            INSTRUCTION_RELATIVE,
            INSTRUCTION_CONTENT,
        )),
        other => Err(CodexxError::Config(format!("未知指令提示词模板: {other}"))),
    }
}

pub fn is_managed_instruction_value(value: &str) -> bool {
    [INSTRUCTION_FILENAME, INSTRUCTION_54_FILENAME]
        .iter()
        .any(|filename| {
            value == format!("./{filename}")
                || value == *filename
                || value.ends_with(&format!("/{filename}"))
                || value.ends_with(&format!("\\{filename}"))
        })
}

pub fn set_top_level_defaults(doc: &mut DocumentMut) {
    if doc.get("model_reasoning_effort").is_none() {
        doc["model_reasoning_effort"] = value("high");
    }
    if doc.get("disable_response_storage").is_none() {
        doc["disable_response_storage"] = value(true);
    }
}

pub fn ensure_table<'a>(parent: &'a mut Table, key: &str) -> Result<&'a mut Table> {
    if !parent.contains_key(key) {
        parent[key] = Item::Table(Table::new());
    }
    parent
        .get_mut(key)
        .and_then(|item| item.as_table_mut())
        .ok_or_else(|| CodexxError::Config(format!("{key} 不是 TOML table")))
}

pub fn command_version(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(target_os = "macos")]
pub fn macos_codex_app_version() -> Option<String> {
    let output = Command::new("/usr/libexec/PlistBuddy")
        .args([
            "-c",
            "Print :CFBundleShortVersionString",
            "/Applications/Codex.app/Contents/Info.plist",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(not(target_os = "macos"))]
pub fn macos_codex_app_version() -> Option<String> {
    None
}

pub fn detect_codex_version() -> Option<String> {
    command_version("codex", &["--version"])
        .or_else(|| command_version("codex", &["-V"]))
        .or_else(macos_codex_app_version)
}

pub fn apply_official_config(
    config_dir: Option<String>,
    model: Option<String>,
    auth_json: Option<String>,
    action: &str,
    message: &str,
) -> Result<ActionResult> {
    let codex_dir = resolve_codex_dir(config_dir)?;
    fs::create_dir_all(&codex_dir).map_err(|e| io_err(&codex_dir, e))?;
    let cfg = config_path(&codex_dir);
    let auth = auth_path(&codex_dir);
    let backup_id = create_backup(&codex_dir, action)?;

    let text = read_to_string_if_exists(&cfg)?;
    let mut doc = parse_toml_document(&cfg, &text)?;

    // 官方模式不应指向自定义路由。
    doc.as_table_mut().remove("model_provider");
    let mut remove_model_providers = false;
    if let Some(providers) = doc
        .as_table_mut()
        .get_mut("model_providers")
        .and_then(|item| item.as_table_mut())
    {
        providers.remove("custom");
        remove_model_providers = providers.is_empty();
    }
    if remove_model_providers {
        doc.as_table_mut().remove("model_providers");
    }

    if let Some(model) = model
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        doc["model"] = value(model);
    }

    write_text(&cfg, &doc.to_string())?;

    if let Some(auth_json) = auth_json
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        let parsed: Value = serde_json::from_str(&auth_json).map_err(|e| json_err(&auth, e))?;
        if !parsed.is_object() {
            return Err(CodexxError::Config(
                "auth.json 必须是 JSON object".to_string(),
            ));
        }
        write_json(&auth, &parsed)?;
    }

    let state = build_state(codex_dir)?;
    Ok(ActionResult {
        ok: true,
        message: message.to_string(),
        backup_id,
        state,
    })
}

pub fn enable_instruction_inner(config_dir: Option<String>, template_id: &str) -> Result<ActionResult> {
    let (filename, relative, content) = instruction_template(template_id)?;
    let codex_dir = resolve_codex_dir(config_dir)?;
    fs::create_dir_all(&codex_dir).map_err(|e| io_err(&codex_dir, e))?;
    let cfg = config_path(&codex_dir);
    let backup_id = create_backup(&codex_dir, "enable-instruct")?;

    let text = read_to_string_if_exists(&cfg)?;
    let mut doc = parse_toml_document(&cfg, &text)?;
    if doc.get("model").is_none() {
        doc["model"] = value("gpt-5.5");
    }
    doc["model_instructions_file"] = value(relative);

    write_text(&codex_dir.join(filename), content)?;
    write_text(&cfg, &doc.to_string())?;

    let state = build_state(codex_dir)?;
    Ok(ActionResult {
        ok: true,
        message: format!("已启用 {filename}"),
        backup_id,
        state,
    })
}