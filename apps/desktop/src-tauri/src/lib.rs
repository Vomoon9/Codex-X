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

pub mod util;
pub mod db;
#[allow(unused_imports)] use util::*;
#[allow(unused_imports)] use db::*;

pub const INSTRUCTION_FILENAME: &str = "gpt5.5-unrestricted.md";
pub const INSTRUCTION_RELATIVE: &str = "./gpt5.5-unrestricted.md";
pub const INSTRUCTION_CONTENT: &str = include_str!("../../../../examples/gpt5.5-unrestricted.md");
pub const INSTRUCTION_54_FILENAME: &str = "gpt5.4-unrestricted.md";
pub const INSTRUCTION_54_RELATIVE: &str = "./gpt5.4-unrestricted.md";
pub const INSTRUCTION_54_CONTENT: &str = include_str!("../../../../examples/gpt5.4-unrestricted.md");

#[derive(Debug, Error)]
pub enum CodexxError {
    #[error("无法获取用户主目录")]
    NoHomeDir,
    #[error("IO error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("TOML parse error at {path}: {message}")]
    Toml { path: String, message: String },
    #[error("JSON error at {path}: {source}")]
    Json {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("配置错误: {0}")]
    Config(String),
    #[error("SQLite error: {0}")]
    Database(String),
}

pub type Result<T> = std::result::Result<T, CodexxError>;

impl serde::Serialize for CodexxError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSummary {
    id: String,
    name: Option<String>,
    base_url: Option<String>,
    wire_api: Option<String>,
    requires_openai_auth: Option<bool>,
    is_current: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexState {
    codex_dir: String,
    config_path: String,
    auth_path: String,
    config_exists: bool,
    auth_exists: bool,
    official_auth_available: bool,
    model: Option<String>,
    model_provider: Option<String>,
    instruction_file: Option<String>,
    instruction_enabled: bool,
    providers: Vec<ProviderSummary>,
    config_text: String,
    auth_preview: Option<Value>,
    auth_text: String,
    last_backup: Option<BackupEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupMeta {
    id: String,
    action: String,
    created_at: String,
    codex_dir: String,
    config_path: String,
    auth_path: String,
    had_config: bool,
    had_auth: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    id: String,
    action: String,
    created_at: String,
    path: String,
    had_config: bool,
    had_auth: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInput {
    config_dir: Option<String>,
    provider_name: String,
    base_url: String,
    model: String,
    api_key: Option<String>,
    wire_api: Option<String>,
    requires_openai_auth: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTomlInput {
    config_dir: Option<String>,
    config_text: String,
    api_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialConfigInput {
    config_dir: Option<String>,
    model: Option<String>,
    auth_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedProvider {
    id: String,
    provider_name: String,
    base_url: String,
    model: String,
    api_key: Option<String>,
    wire_api: String,
    requires_openai_auth: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedPrompt {
    id: String,
    title: String,
    filename: String,
    content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    imported: usize,
    skipped: usize,
    warnings: Vec<String>,
    providers: Vec<SavedProvider>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialAuthCandidate {
    auth_json: String,
    model: Option<String>,
    source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionResult {
    ok: bool,
    message: String,
    backup_id: Option<String>,
    state: CodexState,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutInfo {
    app_version: String,
    codex_version: Option<String>,
    codex_dir: String,
    project_url: String,
    github_repo: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPreview {
    id: String,
    title: String,
    model_provider: Option<String>,
    model: Option<String>,
    cwd: Option<String>,
    rollout_path: Option<String>,
    updated_at_ms: Option<i64>,
    archived: bool,
    has_user_event: bool,
    needs_sync: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSyncStatus {
    codex_dir: String,
    target_provider: String,
    rollout_files: usize,
    session_meta_count: usize,
    mismatched_rollouts: usize,
    mismatched_session_meta: usize,
    sqlite_dbs: usize,
    sqlite_threads: usize,
    mismatched_threads: usize,
    needs_sync: bool,
    backup_dir: Option<String>,
    warnings: Vec<String>,
    sessions: Vec<SessionPreview>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSyncResult {
    status: SessionSyncStatus,
    updated_rollouts: usize,
    updated_threads: usize,
    backup_dir: String,
}

#[derive(Debug, Default)]
pub struct RolloutScan {
    rollout_files: usize,
    session_meta_count: usize,
    mismatched_rollouts: usize,
    mismatched_session_meta: usize,
    changed_files: Vec<(PathBuf, String)>,
    thread_ids_with_user_events: HashSet<String>,
    cwd_by_thread_id: HashMap<String, String>,
    warnings: Vec<String>,
}

#[derive(Debug, Default)]
pub struct SqliteScan {
    sqlite_dbs: usize,
    sqlite_threads: usize,
    mismatched_threads: usize,
    warnings: Vec<String>,
}



#[tauri::command]
fn get_session_sync_status(
    config_dir: Option<String>,
    target_provider: Option<String>,
) -> Result<SessionSyncStatus> {
    session_sync_status_inner(config_dir, target_provider)
}

#[tauri::command]
fn sync_sessions_provider(
    config_dir: Option<String>,
    target_provider: Option<String>,
) -> Result<SessionSyncResult> {
    sync_sessions_provider_inner(config_dir, target_provider)
}

#[tauri::command]
fn read_ccswitch_official_auth(db_path: Option<String>) -> Result<Option<OfficialAuthCandidate>> {
    read_ccswitch_official_auth_inner(db_path)
}

#[tauri::command]
fn import_ccswitch_codex_providers(db_path: Option<String>) -> Result<ImportResult> {
    import_ccswitch_codex_providers_inner(db_path)
}

#[tauri::command]
fn get_about_info(config_dir: Option<String>) -> Result<AboutInfo> {
    let codex_dir = resolve_codex_dir(config_dir)?;
    Ok(AboutInfo {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        codex_version: detect_codex_version(),
        codex_dir: codex_dir.display().to_string(),
        project_url: "https://github.com/yynxxxxx/Codex-X".to_string(),
        github_repo: "yynxxxxx/Codex-X".to_string(),
    })
}

#[tauri::command]
fn list_saved_prompts() -> Result<Vec<SavedPrompt>> {
    list_saved_prompts_inner()
}

#[tauri::command]
fn save_prompt(prompt: SavedPrompt) -> Result<SavedPrompt> {
    let title = prompt.title.trim().to_string();
    if title.is_empty() {
        return Err(CodexxError::Config("提示词名称不能为空".to_string()));
    }
    let content = prompt.content.trim().to_string();
    if content.is_empty() {
        return Err(CodexxError::Config("提示词内容不能为空".to_string()));
    }
    let id = if prompt.id.trim().is_empty() {
        sanitize_id(&title)
    } else {
        sanitize_id(&prompt.id)
    };
    let filename = normalize_prompt_filename(&prompt.filename, &id);
    save_prompt_inner(SavedPrompt {
        id,
        title,
        filename,
        content,
    })
}

#[tauri::command]
fn delete_saved_prompt(id: String) -> Result<()> {
    delete_prompt_inner(id.trim())
}

#[tauri::command]
fn enable_saved_prompt(config_dir: Option<String>, id: String) -> Result<ActionResult> {
    let prompt = get_saved_prompt_inner(id.trim())?;
    let codex_dir = resolve_codex_dir(config_dir)?;
    fs::create_dir_all(&codex_dir).map_err(|e| io_err(&codex_dir, e))?;
    let cfg = config_path(&codex_dir);
    let backup_id = create_backup(&codex_dir, "enable-custom-prompt")?;

    let text = read_to_string_if_exists(&cfg)?;
    let mut doc = parse_toml_document(&cfg, &text)?;
    if doc.get("model").is_none() {
        doc["model"] = value("gpt-5.5");
    }
    doc["model_instructions_file"] = value(format!("./{}", prompt.filename));
    write_text(&codex_dir.join(&prompt.filename), &prompt.content)?;
    write_text(&cfg, &doc.to_string())?;

    let state = build_state(codex_dir)?;
    Ok(ActionResult {
        ok: true,
        message: format!("已启用 {}", prompt.title),
        backup_id,
        state,
    })
}

#[tauri::command]
fn list_saved_providers() -> Result<Vec<SavedProvider>> {
    list_saved_providers_inner()
}

#[tauri::command]
fn save_provider(provider: SavedProvider) -> Result<SavedProvider> {
    let normalized = SavedProvider {
        id: provider.id.trim().to_string(),
        provider_name: provider.provider_name.trim().to_string(),
        base_url: provider.base_url.trim().trim_end_matches('/').to_string(),
        model: provider.model.trim().to_string(),
        api_key: provider
            .api_key
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        wire_api: if provider.wire_api.trim().is_empty() {
            "responses".to_string()
        } else {
            provider.wire_api.trim().to_string()
        },
        requires_openai_auth: provider.requires_openai_auth,
    };
    if normalized.id.is_empty() {
        return Err(CodexxError::Config("provider id 不能为空".to_string()));
    }
    if normalized.provider_name.is_empty() {
        return Err(CodexxError::Config("供应商名称不能为空".to_string()));
    }
    if normalized.base_url.is_empty() {
        return Err(CodexxError::Config("base_url 不能为空".to_string()));
    }
    if normalized.model.is_empty() {
        return Err(CodexxError::Config("model 不能为空".to_string()));
    }
    save_provider_inner(normalized)
}

#[tauri::command]
fn delete_saved_provider(id: String) -> Result<()> {
    delete_provider_inner(id.trim())
}

#[tauri::command]
fn get_codex_state(config_dir: Option<String>) -> Result<CodexState> {
    let codex_dir = resolve_codex_dir(config_dir)?;
    build_state(codex_dir)
}

#[tauri::command]
fn switch_official_provider(config_dir: Option<String>) -> Result<ActionResult> {
    apply_official_config(
        config_dir,
        None,
        None,
        "switch-official",
        "已切换到 OpenAI Official",
    )
}

#[tauri::command]
fn save_official_config(input: OfficialConfigInput) -> Result<ActionResult> {
    apply_official_config(
        input.config_dir,
        input.model,
        input.auth_json,
        "save-official",
        "已保存 OpenAI Official 配置",
    )
}

#[tauri::command]
fn enable_instruction(config_dir: Option<String>) -> Result<ActionResult> {
    enable_instruction_inner(config_dir, "gpt5.5-unrestricted")
}

#[tauri::command]
fn enable_instruction_template(
    config_dir: Option<String>,
    template_id: String,
) -> Result<ActionResult> {
    enable_instruction_inner(config_dir, &template_id)
}

#[tauri::command]
fn disable_instruction(
    config_dir: Option<String>,
    delete_file: Option<bool>,
) -> Result<ActionResult> {
    let codex_dir = resolve_codex_dir(config_dir)?;
    let cfg = config_path(&codex_dir);
    let backup_id = create_backup(&codex_dir, "disable-instruct")?;

    let text = read_to_string_if_exists(&cfg)?;
    let mut doc = parse_toml_document(&cfg, &text)?;
    let current = string_value(&doc, "model_instructions_file");
    let removed = current.is_some();
    if removed {
        doc.as_table_mut().remove("model_instructions_file");
    }

    write_text(&cfg, &doc.to_string())?;
    if delete_file.unwrap_or(true) {
        for filename in [INSTRUCTION_FILENAME, INSTRUCTION_54_FILENAME] {
            let md = codex_dir.join(filename);
            if md.exists() {
                fs::remove_file(&md).map_err(|e| io_err(&md, e))?;
            }
        }
    }

    let state = build_state(codex_dir)?;
    Ok(ActionResult {
        ok: true,
        message: if removed {
            "已禁用指令提示词".to_string()
        } else {
            "当前未设置 model_instructions_file".to_string()
        },
        backup_id,
        state,
    })
}

#[tauri::command]
fn save_provider_toml_config(input: ProviderTomlInput) -> Result<ActionResult> {
    let codex_dir = resolve_codex_dir(input.config_dir.clone())?;
    fs::create_dir_all(&codex_dir).map_err(|e| io_err(&codex_dir, e))?;
    let cfg = config_path(&codex_dir);
    let auth = auth_path(&codex_dir);
    let backup_id = create_backup(&codex_dir, "save-provider-toml")?;

    let config_text = input.config_text.trim_end().to_string();
    let doc = parse_toml_document(&cfg, &config_text)?;
    if string_value(&doc, "model").is_none() {
        return Err(CodexxError::Config(
            "config.toml 必须包含 model".to_string(),
        ));
    }
    write_text(&cfg, &(config_text + "\n"))?;

    if let Some(api_key) = input
        .api_key
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        let mut auth_value = if auth.exists() {
            let text = fs::read_to_string(&auth).map_err(|e| io_err(&auth, e))?;
            serde_json::from_str::<Value>(&text).unwrap_or_else(|_| json!({}))
        } else {
            json!({})
        };
        if !auth_value.is_object() {
            auth_value = json!({});
        }
        auth_value["OPENAI_API_KEY"] = Value::String(api_key);
        write_json(&auth, &auth_value)?;
    }

    let state = build_state(codex_dir)?;
    Ok(ActionResult {
        ok: true,
        message: "已保存供应商 TOML 配置".to_string(),
        backup_id,
        state,
    })
}

#[tauri::command]
fn switch_provider(input: ProviderInput) -> Result<ActionResult> {
    let codex_dir = resolve_codex_dir(input.config_dir.clone())?;
    fs::create_dir_all(&codex_dir).map_err(|e| io_err(&codex_dir, e))?;
    let cfg = config_path(&codex_dir);
    let auth = auth_path(&codex_dir);
    let backup_id = create_backup(&codex_dir, "switch-provider")?;

    let provider_name = input.provider_name.trim();
    let base_url = input.base_url.trim().trim_end_matches('/');
    let model = input.model.trim();
    if provider_name.is_empty() {
        return Err(CodexxError::Config("供应商名称不能为空".to_string()));
    }
    if base_url.is_empty() {
        return Err(CodexxError::Config("base_url 不能为空".to_string()));
    }
    if model.is_empty() {
        return Err(CodexxError::Config("model 不能为空".to_string()));
    }

    let text = read_to_string_if_exists(&cfg)?;
    let mut doc = parse_toml_document(&cfg, &text)?;
    doc["model_provider"] = value("custom");
    doc["model"] = value(model);
    set_top_level_defaults(&mut doc);

    let root = doc.as_table_mut();
    let providers = ensure_table(root, "model_providers")?;
    let custom = ensure_table(providers, "custom")?;
    custom["name"] = value(provider_name);
    custom["base_url"] = value(base_url);
    custom["wire_api"] = value(input.wire_api.unwrap_or_else(|| "responses".to_string()));
    custom["requires_openai_auth"] = value(input.requires_openai_auth.unwrap_or(true));

    write_text(&cfg, &doc.to_string())?;

    if let Some(api_key) = input
        .api_key
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        let mut auth_value = if auth.exists() {
            let text = fs::read_to_string(&auth).map_err(|e| io_err(&auth, e))?;
            serde_json::from_str::<Value>(&text).unwrap_or_else(|_| json!({}))
        } else {
            json!({})
        };
        if !auth_value.is_object() {
            auth_value = json!({});
        }
        auth_value["OPENAI_API_KEY"] = Value::String(api_key);
        write_json(&auth, &auth_value)?;
    }

    let state = build_state(codex_dir)?;
    Ok(ActionResult {
        ok: true,
        message: format!("已切换到 {provider_name} / {model}"),
        backup_id,
        state,
    })
}

#[tauri::command]
fn list_backups() -> Result<Vec<BackupEntry>> {
    backups()
}

#[tauri::command]
fn restore_backup(config_dir: Option<String>, backup_id: String) -> Result<ActionResult> {
    let codex_dir = resolve_codex_dir(config_dir)?;
    let dir = backup_root()?.join(&backup_id);
    if !dir.exists() {
        return Err(CodexxError::Config(format!("备份不存在: {backup_id}")));
    }

    let restore_marker = create_backup(&codex_dir, "before-restore")?;
    let cfg = config_path(&codex_dir);
    let auth = auth_path(&codex_dir);
    fs::create_dir_all(&codex_dir).map_err(|e| io_err(&codex_dir, e))?;

    let backup_cfg = dir.join("config.toml");
    if backup_cfg.exists() {
        let bytes = fs::read(&backup_cfg).map_err(|e| io_err(&backup_cfg, e))?;
        atomic_write(&cfg, &bytes)?;
    } else if cfg.exists() {
        fs::remove_file(&cfg).map_err(|e| io_err(&cfg, e))?;
    }

    let backup_auth = dir.join("auth.json");
    if backup_auth.exists() {
        let bytes = fs::read(&backup_auth).map_err(|e| io_err(&backup_auth, e))?;
        atomic_write(&auth, &bytes)?;
    } else if auth.exists() {
        fs::remove_file(&auth).map_err(|e| io_err(&auth, e))?;
    }

    let state = build_state(codex_dir)?;
    Ok(ActionResult {
        ok: true,
        message: format!("已恢复备份 {backup_id}"),
        backup_id: restore_marker,
        state,
    })
}

#[tauri::command]
fn open_url(url: String) -> std::result::Result<(), String> {
    let trimmed = url.trim().to_string();
    if trimmed.is_empty() {
        return Err("URL 为空".to_string());
    }

    // Do not wait for the browser process. On Windows, waiting for `cmd /C start` can
    // visibly freeze the WebView for a few seconds before the default browser appears.
    std::thread::spawn(move || {
        #[cfg(target_os = "macos")]
        {
            let _ = Command::new("open").arg(&trimmed).spawn();
        }

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            let _ = Command::new("cmd")
                .creation_flags(CREATE_NO_WINDOW)
                .args(["/C", "start", ""])
                .arg(&trimmed)
                .spawn();
        }

        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        {
            let _ = Command::new("xdg-open").arg(&trimmed).spawn();
        }
    });

    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_about_info,
            get_session_sync_status,
            sync_sessions_provider,
            read_ccswitch_official_auth,
            import_ccswitch_codex_providers,
            list_saved_prompts,
            save_prompt,
            delete_saved_prompt,
            enable_saved_prompt,
            list_saved_providers,
            save_provider,
            delete_saved_provider,
            get_codex_state,
            switch_official_provider,
            save_official_config,
            enable_instruction,
            enable_instruction_template,
            disable_instruction,
            switch_provider,
            save_provider_toml_config,
            list_backups,
            restore_backup,
            open_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Codex-X");
}