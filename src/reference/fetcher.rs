use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Context, Result};

pub const UNITY_CS_REFERENCE_URL: &str =
    "https://github.com/Unity-Technologies/UnityCsReference.git";
const LICENSE_ENV_VAR: &str = "UNITY_CLI_ACCEPT_LICENSE";
const GITHUB_TOKEN_ENV_VARS: &[&str] = &["GITHUB_TOKEN", "GH_TOKEN"];

pub fn build_clone_args(url: &str, branch: &str, dest: &Path, depth: u32) -> Vec<String> {
    vec![
        "--depth".to_string(),
        depth.to_string(),
        "--single-branch".to_string(),
        "--branch".to_string(),
        branch.to_string(),
        url.to_string(),
        dest.display().to_string(),
    ]
}

pub fn require_license_accepted(flag: bool) -> Result<()> {
    if flag {
        return Ok(());
    }
    if let Ok(value) = std::env::var(LICENSE_ENV_VAR) {
        if !value.trim().is_empty() && value != "0" {
            return Ok(());
        }
    }
    Err(anyhow!(
        "UnityCsReference is distributed under the Unity Companion License. Pass --accept-license or set {}=1 to confirm consent before fetching.",
        LICENSE_ENV_VAR
    ))
}

pub fn ensure_git_available() -> Result<()> {
    Command::new("git")
        .arg("--version")
        .output()
        .context("git binary not found in PATH; install git or use a future zip fallback")?;
    Ok(())
}

fn github_token() -> Option<String> {
    for key in GITHUB_TOKEN_ENV_VARS {
        if let Ok(v) = std::env::var(key) {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
    }
    None
}

pub fn run_clone(
    url: &str,
    branch: &str,
    dest: &Path,
    depth: u32,
    accept_license: bool,
) -> Result<()> {
    require_license_accepted(accept_license)?;
    ensure_git_available()?;
    let mut cmd = Command::new("git");
    if let Some(token) = github_token() {
        cmd.arg("-c")
            .arg(format!("http.extraHeader=Authorization: token {token}"));
    }
    cmd.arg("clone");
    for arg in build_clone_args(url, branch, dest, depth) {
        cmd.arg(arg);
    }
    let status = cmd
        .status()
        .with_context(|| format!("failed to spawn git clone for {url}"))?;
    if !status.success() {
        return Err(anyhow!("git clone exited with status {status}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::path::PathBuf;
    use std::sync::Mutex;

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
        fn unset(key: &'static str) -> Self {
            let previous = env::var(key).ok();
            env::remove_var(key);
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

    fn env_lock() -> &'static Mutex<()> {
        crate::test_env::env_lock()
    }

    #[test]
    fn build_clone_args_emits_shallow_single_branch() {
        let dest = PathBuf::from("/tmp/unity-cs-reference/2023.2.20f1");
        let args = build_clone_args(UNITY_CS_REFERENCE_URL, "2023.2/staging", &dest, 1);
        assert_eq!(
            args,
            vec![
                "--depth".to_string(),
                "1".to_string(),
                "--single-branch".to_string(),
                "--branch".to_string(),
                "2023.2/staging".to_string(),
                UNITY_CS_REFERENCE_URL.to_string(),
                dest.display().to_string(),
            ]
        );
    }

    #[test]
    fn license_required_when_flag_false_and_env_unset() {
        let _guard = env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _env = EnvVarGuard::unset("UNITY_CLI_ACCEPT_LICENSE");
        let err = require_license_accepted(false).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("Unity Companion License"));
        assert!(msg.contains("--accept-license"));
    }

    #[test]
    fn license_ok_when_flag_true() {
        let _guard = env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _env = EnvVarGuard::unset("UNITY_CLI_ACCEPT_LICENSE");
        require_license_accepted(true).expect("license OK when flag set");
    }

    #[test]
    fn license_ok_when_env_set() {
        let _guard = env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _env = EnvVarGuard::set("UNITY_CLI_ACCEPT_LICENSE", "1");
        require_license_accepted(false).expect("license OK when env set");
    }
}
