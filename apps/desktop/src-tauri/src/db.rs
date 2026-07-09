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
use crate::util::*;

fn db_err<E: ToString>(err: E) -> CodexxError {
    CodexxError::Database(err.to_string())
}


pub fn sql_select_column(cols: &HashSet<String>, name: &str, fallback: &str) -> String {
    if cols.contains(name) {
        format!("\"{}\"", name.replace('"', "\"\""))
    } else {
        fallback.to_string()
    }
}

pub fn db_path() -> Result<PathBuf> {
    Ok(app_home()?.join("codexx.db"))
}

pub fn open_db() -> Result<Connection> {
    let path = db_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
    }
    let conn = Connection::open(&path).map_err(|e| db_err(e))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS providers (
            id TEXT PRIMARY KEY,
            provider_name TEXT NOT NULL,
            base_url TEXT NOT NULL,
            model TEXT NOT NULL,
            api_key TEXT,
            wire_api TEXT NOT NULL DEFAULT 'responses',
            requires_openai_auth INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_providers_updated_at ON providers(updated_at DESC);
        CREATE TABLE IF NOT EXISTS prompts (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            filename TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_prompts_updated_at ON prompts(updated_at DESC);",
    )
    .map_err(|e| db_err(e))?;
    Ok(conn)
}

pub fn list_saved_providers_inner() -> Result<Vec<SavedProvider>> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, provider_name, base_url, model, api_key, wire_api, requires_openai_auth
             FROM providers
             ORDER BY created_at ASC, updated_at ASC",
        )
        .map_err(|e| db_err(e))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(SavedProvider {
                id: row.get(0)?,
                provider_name: row.get(1)?,
                base_url: row.get(2)?,
                model: row.get(3)?,
                api_key: row.get(4)?,
                wire_api: row.get(5)?,
                requires_openai_auth: row.get::<_, i64>(6)? != 0,
            })
        })
        .map_err(|e| db_err(e))?;

    let mut providers = Vec::new();
    for row in rows {
        providers.push(row.map_err(|e| db_err(e))?);
    }
    Ok(providers)
}

pub fn save_provider_inner(provider: SavedProvider) -> Result<SavedProvider> {
    let conn = open_db()?;
    let now = now_rfc3339();
    conn.execute(
        "INSERT INTO providers
            (id, provider_name, base_url, model, api_key, wire_api, requires_openai_auth, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
         ON CONFLICT(id) DO UPDATE SET
            provider_name = excluded.provider_name,
            base_url = excluded.base_url,
            model = excluded.model,
            api_key = excluded.api_key,
            wire_api = excluded.wire_api,
            requires_openai_auth = excluded.requires_openai_auth,
            updated_at = excluded.updated_at",
        params![
            provider.id,
            provider.provider_name,
            provider.base_url,
            provider.model,
            provider.api_key,
            provider.wire_api,
            if provider.requires_openai_auth { 1 } else { 0 },
            now,
        ],
    )
    .map_err(|e| db_err(e))?;

    // Re-read to return the normalized persisted row.
    let providers = list_saved_providers_inner()?;
    providers
        .into_iter()
        .find(|p| p.id == provider.id)
        .ok_or_else(|| db_err("provider saved but not found"))
}

pub fn delete_provider_inner(id: &str) -> Result<()> {
    let conn = open_db()?;
    conn.execute("DELETE FROM providers WHERE id = ?1", params![id])
        .map_err(|e| db_err(e))?;
    Ok(())
}

pub fn list_saved_prompts_inner() -> Result<Vec<SavedPrompt>> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare("SELECT id, title, filename, content FROM prompts ORDER BY updated_at DESC, created_at DESC")
        .map_err(|e| db_err(e))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(SavedPrompt {
                id: row.get(0)?,
                title: row.get(1)?,
                filename: row.get(2)?,
                content: row.get(3)?,
            })
        })
        .map_err(|e| db_err(e))?;
    let mut prompts = Vec::new();
    for row in rows {
        prompts.push(row.map_err(|e| db_err(e))?);
    }
    Ok(prompts)
}

pub fn save_prompt_inner(prompt: SavedPrompt) -> Result<SavedPrompt> {
    let conn = open_db()?;
    let now = now_rfc3339();
    conn.execute(
        "INSERT INTO prompts (id, title, filename, content, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)
         ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            filename = excluded.filename,
            content = excluded.content,
            updated_at = excluded.updated_at",
        params![
            prompt.id,
            prompt.title,
            prompt.filename,
            prompt.content,
            now
        ],
    )
    .map_err(|e| db_err(e))?;
    list_saved_prompts_inner()?
        .into_iter()
        .find(|p| p.id == prompt.id)
        .ok_or_else(|| db_err("prompt saved but not found"))
}

pub fn get_saved_prompt_inner(id: &str) -> Result<SavedPrompt> {
    list_saved_prompts_inner()?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| CodexxError::Config(format!("提示词不存在: {id}")))
}

pub fn delete_prompt_inner(id: &str) -> Result<()> {
    let conn = open_db()?;
    conn.execute("DELETE FROM prompts WHERE id = ?1", params![id])
        .map_err(|e| db_err(e))?;
    Ok(())
}

pub fn import_ccswitch_codex_providers_inner(path: Option<String>) -> Result<ImportResult> {
    let db = path
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or(default_ccswitch_db_path()?);

    if !db.exists() {
        let candidates = ccswitch_db_candidates()?
            .into_iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n- ");
        return Err(CodexxError::Config(format!(
            "cc-switch 数据库不存在: {}\n已检查候选路径:\n- {}",
            db.display(),
            candidates
        )));
    }

    let conn = Connection::open_with_flags(
        &db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| {
        db_err(format!("打开 cc-switch 数据库失败 {}: {e}", db.display()))
    })?;

    let mut stmt = conn
        .prepare("SELECT id, name, settings_config FROM providers WHERE app_type = 'codex' ORDER BY sort_index ASC, created_at ASC")
        .map_err(|e| db_err(e))?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| db_err(e))?;

    let mut imported = 0usize;
    let mut skipped = 0usize;
    let mut warnings = Vec::new();

    for row in rows {
        let (id, name, settings_config) = row.map_err(|e| db_err(e))?;
        match extract_ccswitch_codex_provider(&id, &name, &settings_config) {
            Some(provider) => {
                save_provider_inner(provider)?;
                imported += 1;
            }
            None => {
                skipped += 1;
                warnings.push(format!(
                    "跳过 {name} ({id})：未找到可用 config/base_url，可能是官方登录或空模板"
                ));
            }
        }
    }

    Ok(ImportResult {
        imported,
        skipped,
        warnings,
        providers: list_saved_providers_inner()?,
    })
}

pub fn read_ccswitch_official_auth_inner(
    path: Option<String>,
) -> Result<Option<OfficialAuthCandidate>> {
    let db = path
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or(default_ccswitch_db_path()?);

    if !db.exists() {
        return Ok(None);
    }

    let conn = Connection::open_with_flags(
        &db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| {
        db_err(format!("打开 cc-switch 数据库失败 {}: {e}", db.display()))
    })?;

    let mut stmt = conn
        .prepare(
            "SELECT id, name, settings_config FROM providers
             WHERE app_type = 'codex' AND (id = 'codex-official' OR category = 'official')
             ORDER BY CASE WHEN id = 'codex-official' THEN 0 ELSE 1 END
             LIMIT 1",
        )
        .map_err(|e| db_err(e))?;

    let mut rows = stmt
        .query([])
        .map_err(|e| db_err(e))?;

    let Some(row) = rows
        .next()
        .map_err(|e| db_err(e))?
    else {
        return Ok(None);
    };

    let id: String = row
        .get(0)
        .map_err(|e| db_err(e))?;
    let name: String = row
        .get(1)
        .map_err(|e| db_err(e))?;
    let settings_config: String = row
        .get(2)
        .map_err(|e| db_err(e))?;
    let settings: Value = serde_json::from_str(&settings_config).map_err(|e| {
        db_err(format!("cc-switch official settings JSON 解析失败: {e}"))
    })?;

    let auth = settings
        .get("auth")
        .cloned()
        .filter(|value| value.is_object())
        .ok_or_else(|| {
            db_err("cc-switch official provider 缺少 auth object")
        })?;

    let model = settings
        .get("config")
        .and_then(Value::as_str)
        .and_then(|text| text.parse::<DocumentMut>().ok())
        .and_then(|doc| string_value(&doc, "model"));

    let auth_json = serde_json::to_string_pretty(&auth)
        .map_err(|e| db_err(format!("官方 auth JSON 格式化失败: {e}")))?;

    Ok(Some(OfficialAuthCandidate {
        auth_json,
        model,
        source: format!("cc-switch:{name}:{id}"),
    }))
}

pub fn sqlite_has_table(conn: &Connection, table: &str) -> Result<bool> {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
        [table],
        |_| Ok(()),
    )
    .map(|_| true)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(false),
        other => Err(db_err(other.to_string())),
    })
}

pub fn table_column_set(conn: &Connection, table: &str) -> Result<HashSet<String>> {
    let mut stmt = conn
        .prepare(&format!(
            "PRAGMA table_info(\"{}\")",
            table.replace('"', "\"\"")
        ))
        .map_err(|e| db_err(e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| db_err(e))?;
    let mut cols = HashSet::new();
    for row in rows {
        cols.insert(row.map_err(|e| db_err(e))?);
    }
    Ok(cols)
}

pub fn scan_sqlite(codex_dir: &Path, target_provider: &str) -> Result<SqliteScan> {
    let mut scan = SqliteScan::default();
    for path in sqlite_candidate_paths(codex_dir) {
        let conn = match Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(conn) => conn,
            Err(e) => {
                scan.warnings
                    .push(format!("无法读取 SQLite: {} ({e})", path.display()));
                continue;
            }
        };
        if !sqlite_has_table(&conn, "threads")? {
            continue;
        }
        let cols = table_column_set(&conn, "threads")?;
        if !cols.contains("model_provider") {
            scan.warnings.push(format!(
                "SQLite threads 缺少 model_provider 字段: {}",
                path.display()
            ));
            continue;
        }
        scan.sqlite_dbs += 1;
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM threads", [], |row| row.get(0))
            .map_err(|e| db_err(e))?;
        let mismatch: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM threads WHERE COALESCE(model_provider, '') <> ?1",
                [target_provider],
                |row| row.get(0),
            )
            .map_err(|e| db_err(e))?;
        scan.sqlite_threads += total.max(0) as usize;
        scan.mismatched_threads += mismatch.max(0) as usize;
    }
    Ok(scan)
}

pub fn list_session_previews(
    codex_dir: &Path,
    target_provider: &str,
    limit: usize,
) -> Result<(Vec<SessionPreview>, Vec<String>)> {
    let mut sessions = Vec::new();
    let mut warnings = Vec::new();
    let mut seen = HashSet::new();

    for path in sqlite_candidate_paths(codex_dir) {
        let conn = match Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(conn) => conn,
            Err(e) => {
                warnings.push(format!("无法读取会话数据库: {} ({e})", path.display()));
                continue;
            }
        };
        if !sqlite_has_table(&conn, "threads")? {
            continue;
        }
        let cols = table_column_set(&conn, "threads")?;
        if !cols.contains("id") {
            continue;
        }

        let title_col = sql_select_column(&cols, "title", "NULL");
        let first_message_col = sql_select_column(&cols, "first_user_message", "NULL");
        let preview_col = sql_select_column(&cols, "preview", "NULL");
        let provider_col = sql_select_column(&cols, "model_provider", "NULL");
        let model_col = sql_select_column(&cols, "model", "NULL");
        let cwd_col = sql_select_column(&cols, "cwd", "NULL");
        let rollout_col = sql_select_column(&cols, "rollout_path", "NULL");
        let updated_ms_col = sql_select_column(&cols, "updated_at_ms", "NULL");
        let updated_col = sql_select_column(&cols, "updated_at", "NULL");
        let archived_col = sql_select_column(&cols, "archived", "0");
        let has_user_event_col = sql_select_column(&cols, "has_user_event", "0");
        let order_col = if cols.contains("recency_at_ms") {
            "\"recency_at_ms\""
        } else if cols.contains("updated_at_ms") {
            "\"updated_at_ms\""
        } else if cols.contains("updated_at") {
            "\"updated_at\""
        } else {
            "\"id\""
        };

        let query = format!(
            "SELECT \"id\", {title_col}, {first_message_col}, {preview_col}, {provider_col}, {model_col}, {cwd_col}, {rollout_col}, {updated_ms_col}, {updated_col}, {archived_col}, {has_user_event_col} FROM threads ORDER BY {order_col} DESC LIMIT {}",
            limit.max(1)
        );
        let mut stmt = conn
            .prepare(&query)
            .map_err(|e| db_err(e))?;
        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let title: Option<String> = row.get(1)?;
                let first_message: Option<String> = row.get(2)?;
                let preview: Option<String> = row.get(3)?;
                let model_provider: Option<String> = row.get(4)?;
                let model: Option<String> = row.get(5)?;
                let cwd: Option<String> = row.get(6)?;
                let rollout_path: Option<String> = row.get(7)?;
                let updated_at_ms: Option<i64> = row.get(8)?;
                let updated_at: Option<i64> = row.get(9)?;
                let archived: i64 = row.get(10)?;
                let has_user_event: i64 = row.get(11)?;
                let clean_title = [title, first_message, preview]
                    .into_iter()
                    .flatten()
                    .map(|v| v.trim().to_string())
                    .find(|v| !v.is_empty())
                    .unwrap_or_else(|| format!("会话 {}", id.chars().take(8).collect::<String>()));
                let normalized_provider = model_provider
                    .as_ref()
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty());
                Ok(SessionPreview {
                    id,
                    title: clean_title,
                    model_provider: normalized_provider.clone(),
                    model: model.and_then(|v| {
                        let v = v.trim().to_string();
                        (!v.is_empty()).then_some(v)
                    }),
                    cwd: cwd.and_then(|v| {
                        let v = v.trim().to_string();
                        (!v.is_empty()).then_some(v)
                    }),
                    rollout_path: rollout_path.and_then(|v| {
                        let v = v.trim().to_string();
                        (!v.is_empty()).then_some(v)
                    }),
                    updated_at_ms: updated_at_ms.or_else(|| updated_at.map(|v| v * 1000)),
                    archived: archived != 0,
                    has_user_event: has_user_event != 0,
                    needs_sync: normalized_provider.as_deref() != Some(target_provider),
                })
            })
            .map_err(|e| db_err(e))?;

        for row in rows {
            let session = row.map_err(|e| db_err(e))?;
            if seen.insert(session.id.clone()) {
                sessions.push(session);
            }
        }
    }

    sessions.sort_by(|a, b| b.updated_at_ms.cmp(&a.updated_at_ms));
    sessions.truncate(limit);
    Ok((sessions, warnings))
}

pub fn apply_sqlite_provider_sync(
    codex_dir: &Path,
    target_provider: &str,
    thread_ids_with_user_events: &HashSet<String>,
    cwd_by_thread_id: &HashMap<String, String>,
) -> Result<usize> {
    let mut updated = 0usize;
    for path in sqlite_candidate_paths(codex_dir) {
        let mut conn = match Connection::open(&path) {
            Ok(conn) => conn,
            Err(e) => {
                return Err(db_err(format!(
                    "打开 SQLite 失败 {}: {e}",
                    path.display()
                )))
            }
        };
        if !sqlite_has_table(&conn, "threads")? {
            continue;
        }
        let cols = table_column_set(&conn, "threads")?;
        if !cols.contains("model_provider") {
            continue;
        }
        let tx = conn
            .transaction()
            .map_err(|e| db_err(e))?;
        updated += tx
            .execute(
                "UPDATE threads SET model_provider = ?1 WHERE COALESCE(model_provider, '') <> ?1",
                [target_provider],
            )
            .map_err(|e| db_err(e))?;
        if cols.contains("has_user_event") {
            for id in thread_ids_with_user_events {
                updated += tx
                    .execute("UPDATE threads SET has_user_event = 1 WHERE id = ?1 AND COALESCE(has_user_event, 0) <> 1", [id])
                    .map_err(|e| db_err(e))?;
            }
        }
        if cols.contains("cwd") {
            for (id, cwd) in cwd_by_thread_id {
                updated += tx
                    .execute(
                        "UPDATE threads SET cwd = ?1 WHERE id = ?2 AND COALESCE(cwd, '') <> ?1",
                        (cwd, id),
                    )
                    .map_err(|e| db_err(e))?;
            }
        }
        tx.commit()
            .map_err(|e| db_err(e))?;
    }
    Ok(updated)
}

pub fn session_sync_status_inner(
    config_dir: Option<String>,
    target_provider: Option<String>,
) -> Result<SessionSyncStatus> {
    let codex_dir = resolve_codex_dir(config_dir)?;
    let target = current_model_provider(&codex_dir, target_provider)?;
    let rollouts = scan_rollouts(&codex_dir, &target, false)?;
    let sqlite = scan_sqlite(&codex_dir, &target)?;
    let session_limit = sqlite.sqlite_threads.max(50).min(1000);
    let (sessions, session_warnings) = list_session_previews(&codex_dir, &target, session_limit)?;
    let mut warnings = rollouts.warnings;
    warnings.extend(sqlite.warnings);
    warnings.extend(session_warnings);
    Ok(SessionSyncStatus {
        codex_dir: codex_dir.display().to_string(),
        target_provider: target,
        rollout_files: rollouts.rollout_files,
        session_meta_count: rollouts.session_meta_count,
        mismatched_rollouts: rollouts.mismatched_rollouts,
        mismatched_session_meta: rollouts.mismatched_session_meta,
        sqlite_dbs: sqlite.sqlite_dbs,
        sqlite_threads: sqlite.sqlite_threads,
        mismatched_threads: sqlite.mismatched_threads,
        needs_sync: rollouts.mismatched_session_meta > 0 || sqlite.mismatched_threads > 0,
        backup_dir: None,
        warnings,
        sessions,
    })
}

pub fn sync_sessions_provider_inner(
    config_dir: Option<String>,
    target_provider: Option<String>,
) -> Result<SessionSyncResult> {
    let codex_dir = resolve_codex_dir(config_dir)?;
    fs::create_dir_all(&codex_dir).map_err(|e| io_err(&codex_dir, e))?;
    let target = current_model_provider(&codex_dir, target_provider)?;
    let lock_dir = codex_dir.join("tmp").join("provider-sync.lock");
    fs::create_dir_all(lock_dir.parent().unwrap_or(&codex_dir))
        .map_err(|e| io_err(lock_dir.parent().unwrap_or(&codex_dir), e))?;
    if lock_dir.exists() {
        return Err(CodexxError::Config(format!(
            "会话同步锁已存在: {}",
            lock_dir.display()
        )));
    }
    fs::create_dir_all(&lock_dir).map_err(|e| io_err(&lock_dir, e))?;

    let result = (|| -> Result<SessionSyncResult> {
        let rollouts = scan_rollouts(&codex_dir, &target, true)?;
        let sqlite_before = scan_sqlite(&codex_dir, &target)?;
        let changed_paths = rollouts
            .changed_files
            .iter()
            .map(|(p, _)| p.clone())
            .collect::<Vec<_>>();
        let backup_dir = create_provider_sync_backup(&codex_dir, &target, &changed_paths)?;

        let mut updated_rollouts = 0usize;
        for (path, text) in &rollouts.changed_files {
            write_text(path, text)?;
            updated_rollouts += 1;
        }
        let updated_threads = apply_sqlite_provider_sync(
            &codex_dir,
            &target,
            &rollouts.thread_ids_with_user_events,
            &rollouts.cwd_by_thread_id,
        )?;
        let mut status =
            session_sync_status_inner(Some(codex_dir.display().to_string()), Some(target.clone()))?;
        status.backup_dir = Some(backup_dir.display().to_string());
        if rollouts.changed_files.is_empty() && sqlite_before.mismatched_threads == 0 {
            status
                .warnings
                .push("没有发现需要修复的会话；已保留一次安全备份。".to_string());
        }
        Ok(SessionSyncResult {
            status,
            updated_rollouts,
            updated_threads,
            backup_dir: backup_dir.display().to_string(),
        })
    })();

    let _ = fs::remove_dir_all(&lock_dir);
    result
}