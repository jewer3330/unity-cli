#![allow(dead_code)]

pub mod cache;
pub mod fetcher;
pub mod search;
pub mod version;

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use walkdir::WalkDir;

pub fn maybe_execute_reference_tool(tool_name: &str, params: &Value) -> Option<Result<Value>> {
    match tool_name {
        "reference_fetch" => Some(execute_fetch(params)),
        "reference_status" => Some(execute_status(params)),
        "reference_search" => Some(execute_search(params)),
        "reference_grep" => Some(execute_grep(params)),
        "reference_view" => Some(execute_view(params)),
        "reference_clean" => Some(execute_clean(params)),
        _ => None,
    }
}

fn resolve_version(params: &Value) -> Result<String> {
    if let Some(v) = params.get("version").and_then(Value::as_str) {
        if !v.is_empty() {
            return Ok(v.to_string());
        }
    }
    let project_root = params
        .get("projectRoot")
        .and_then(Value::as_str)
        .map(Path::new)
        .unwrap_or_else(|| Path::new("."));
    Ok(version::detect_from_project(project_root)?.version)
}

fn resolve_version_and_branch(params: &Value) -> Result<(String, String)> {
    let explicit_version = params
        .get("version")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let explicit_branch = params
        .get("branch")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if let (Some(v), Some(b)) = (explicit_version.clone(), explicit_branch.clone()) {
        return Ok((v, b));
    }
    let project_root = params
        .get("projectRoot")
        .and_then(Value::as_str)
        .map(Path::new)
        .unwrap_or_else(|| Path::new("."));
    let detected = version::detect_from_project(project_root)?;
    Ok((
        explicit_version.unwrap_or(detected.version),
        explicit_branch.unwrap_or(detected.branch),
    ))
}

fn execute_fetch(params: &Value) -> Result<Value> {
    let (version, branch) = resolve_version_and_branch(params)?;
    let accept_license = params
        .get("acceptLicense")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let force = params
        .get("force")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let dest = cache::version_dir(&version)?;
    if dest.exists() && !force {
        return Ok(json!({
            "ok": true,
            "skipped": true,
            "reason": "destination already exists; pass force=true to refetch",
            "version": version,
            "branch": branch,
            "path": dest.display().to_string(),
        }));
    }
    if dest.exists() && force {
        std::fs::remove_dir_all(&dest)
            .with_context(|| format!("failed to remove {}", dest.display()))?;
    }
    fetcher::run_clone(
        fetcher::UNITY_CS_REFERENCE_URL,
        &branch,
        &dest,
        1,
        accept_license,
    )?;
    let meta = cache::CacheMeta {
        version: version.clone(),
        branch: branch.clone(),
        commit_sha: None,
        fetched_at: now_unix_seconds_string(),
        source_url: fetcher::UNITY_CS_REFERENCE_URL.to_string(),
    };
    cache::write_meta(&meta)?;
    Ok(json!({
        "ok": true,
        "version": version,
        "branch": branch,
        "path": dest.display().to_string(),
        "fetchedAt": meta.fetched_at,
    }))
}

fn execute_status(_params: &Value) -> Result<Value> {
    let versions = cache::list_versions()?;
    let mut entries = Vec::new();
    for v in versions {
        let dir = cache::version_dir(&v)?;
        let size_bytes = dir_size(&dir).unwrap_or(0);
        let meta = cache::read_meta(&v).ok();
        entries.push(json!({
            "version": v,
            "branch": meta.as_ref().map(|m| m.branch.clone()).unwrap_or_default(),
            "fetchedAt": meta.as_ref().map(|m| m.fetched_at.clone()).unwrap_or_default(),
            "sizeBytes": size_bytes,
            "path": dir.display().to_string(),
        }));
    }
    Ok(json!({
        "ok": true,
        "versions": entries,
    }))
}

fn execute_search(params: &Value) -> Result<Value> {
    let pattern = params
        .get("pattern")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("search requires `pattern`"))?;
    let version = resolve_version(params)?;
    let dir = cache::version_dir(&version)?;
    let file_glob = params.get("path").and_then(Value::as_str);
    let context = params.get("context").and_then(Value::as_u64).unwrap_or(0) as u32;
    let max_results = params
        .get("maxResults")
        .and_then(Value::as_u64)
        .map(|n| n as usize);
    let mut hits = search::run_grep(&dir, pattern, file_glob, context)?;
    if let Some(n) = max_results {
        hits.truncate(n);
    }
    Ok(json!({
        "ok": true,
        "version": version,
        "hits": hits,
    }))
}

fn execute_grep(params: &Value) -> Result<Value> {
    let pattern = params
        .get("pattern")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("grep requires `pattern`"))?;
    let version = resolve_version(params)?;
    let dir = cache::version_dir(&version)?;
    let file_glob = params.get("fileGlob").and_then(Value::as_str);
    let context = params.get("context").and_then(Value::as_u64).unwrap_or(0) as u32;
    let hits = search::run_grep(&dir, pattern, file_glob, context)?;
    Ok(json!({
        "ok": true,
        "version": version,
        "hits": hits,
    }))
}

fn execute_view(params: &Value) -> Result<Value> {
    let path = params
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("view requires `path`"))?;
    let version = resolve_version(params)?;
    let dir = cache::version_dir(&version)?;
    let start_line = params
        .get("startLine")
        .and_then(Value::as_u64)
        .map(|n| n as u32);
    let max_lines = params
        .get("maxLines")
        .and_then(Value::as_u64)
        .map(|n| n as u32);
    let out = search::run_view(&dir, path, start_line, max_lines)?;
    Ok(json!({
        "ok": true,
        "version": version,
        "view": out,
    }))
}

fn execute_clean(params: &Value) -> Result<Value> {
    let keep = params.get("keep").and_then(Value::as_u64).unwrap_or(1) as usize;
    let dry_run = params
        .get("dryRun")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let removed = cache::gc(keep, dry_run)?;
    Ok(json!({
        "ok": true,
        "keep": keep,
        "dryRun": dry_run,
        "removed": removed
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>(),
    }))
}

fn now_unix_seconds_string() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

fn dir_size(path: &Path) -> Result<u64> {
    let mut total = 0u64;
    for entry in WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            if let Ok(meta) = entry.metadata() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = env::var(key).ok();
            env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.previous {
                env::set_var(self.key, value);
            } else {
                env::remove_var(self.key);
            }
        }
    }

    fn unique_temp_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        env::temp_dir().join(format!("unity-cli-reference-mod-{label}-{nanos}"))
    }

    #[test]
    fn maybe_execute_returns_none_for_unknown_tool() {
        assert!(maybe_execute_reference_tool("nonexistent", &json!({})).is_none());
    }

    #[test]
    fn execute_clean_dry_run_on_empty_cache_returns_empty_removed() {
        let _guard = crate::test_env::env_lock()
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let root = unique_temp_path("clean");
        let _env = EnvVarGuard::set("UNITY_CLI_CACHE_ROOT", root.to_str().unwrap());
        let value =
            maybe_execute_reference_tool("reference_clean", &json!({"dryRun": true, "keep": 1}))
                .unwrap()
                .unwrap();
        assert_eq!(value["ok"], true);
        assert_eq!(value["dryRun"], true);
        assert!(value["removed"].is_array());
        assert_eq!(value["removed"].as_array().unwrap().len(), 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn execute_status_returns_versions_list() {
        let _guard = crate::test_env::env_lock()
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let root = unique_temp_path("status");
        let _env = EnvVarGuard::set("UNITY_CLI_CACHE_ROOT", root.to_str().unwrap());
        let base = root.join("UnityCsReference");
        std::fs::create_dir_all(base.join("2023.2.20f1")).unwrap();
        let value = maybe_execute_reference_tool("reference_status", &json!({}))
            .unwrap()
            .unwrap();
        assert_eq!(value["ok"], true);
        let versions = value["versions"].as_array().unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0]["version"], "2023.2.20f1");
        let _ = std::fs::remove_dir_all(&root);
    }
}
