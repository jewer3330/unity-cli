use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use super::model::{Frontmatter, Metadata, Skill};

/// Load every skill directory under `root` whose name starts with `unity-`.
///
/// `root` should normally point to `.claude-plugin/plugins/unity-cli/skills/`.
/// If `root` itself contains a `SKILL.md`, it is treated as a single skill.
pub fn load_skills(root: &Path) -> Result<Vec<Skill>> {
    let mut skills = Vec::new();
    if root.join("SKILL.md").exists() {
        skills.push(load_skill(root)?);
        return Ok(skills);
    }
    if !root.exists() {
        return Err(anyhow!("skills root not found: {}", root.display()));
    }
    let entries = fs::read_dir(root).with_context(|| format!("read_dir {}", root.display()))?;
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if !name.starts_with("unity-") {
            continue;
        }
        if !path.join("SKILL.md").exists() {
            continue;
        }
        paths.push(path);
    }
    paths.sort();
    for path in paths {
        skills.push(load_skill(&path)?);
    }
    Ok(skills)
}

pub fn load_skill(dir: &Path) -> Result<Skill> {
    let skill_md_path = dir.join("SKILL.md");
    let raw = fs::read_to_string(&skill_md_path)
        .with_context(|| format!("read {}", skill_md_path.display()))?;
    let (raw_yaml, body) = split_frontmatter(&raw);
    let frontmatter = parse_frontmatter(raw_yaml)
        .with_context(|| format!("parse frontmatter for {}", skill_md_path.display()))?;
    let name = frontmatter
        .name
        .clone()
        .unwrap_or_else(|| dir_name(dir).to_string());
    let references = collect_references(dir)?;
    Ok(Skill {
        name,
        dir: dir.to_path_buf(),
        skill_md_path,
        frontmatter,
        body: body.to_string(),
        references,
    })
}

fn dir_name(path: &Path) -> &str {
    path.file_name().and_then(|s| s.to_str()).unwrap_or("")
}

fn split_frontmatter(raw: &str) -> (&str, &str) {
    // Match a leading `---\n...\n---\n` block. Be tolerant of trailing CR.
    let bytes = raw.as_bytes();
    if bytes.len() < 4 || &bytes[..3] != b"---" {
        return ("", raw);
    }
    // Find first newline after opening ---.
    let after_open = match raw.find('\n') {
        Some(idx) => idx + 1,
        None => return ("", raw),
    };
    let rest = &raw[after_open..];
    if let Some(pos) = rest.find("\n---") {
        let yaml = &rest[..pos];
        // Skip past `\n---` and optional `\r\n` or `\n`.
        let mut body_start = pos + 4;
        if rest[body_start..].starts_with('\r') {
            body_start += 1;
        }
        if rest[body_start..].starts_with('\n') {
            body_start += 1;
        }
        let body = &rest[body_start..];
        (yaml, body)
    } else {
        ("", raw)
    }
}

fn parse_frontmatter(raw_yaml: &str) -> Result<Frontmatter> {
    let mut fm = Frontmatter {
        raw_yaml: raw_yaml.to_string(),
        ..Default::default()
    };
    if raw_yaml.trim().is_empty() {
        return Ok(fm);
    }
    let value: serde_yml::Value =
        serde_yml::from_str(raw_yaml).with_context(|| "frontmatter is not valid YAML")?;
    let mapping = value
        .as_mapping()
        .ok_or_else(|| anyhow!("frontmatter is not a YAML mapping"))?;

    for (key, val) in mapping {
        let key_str = match key.as_str() {
            Some(s) => s,
            None => continue,
        };
        match key_str {
            "name" => fm.name = val.as_str().map(|s| s.to_string()),
            "description" => fm.description = val.as_str().map(|s| s.to_string()),
            "allowed-tools" => fm.allowed_tools = val.as_str().map(|s| s.to_string()),
            "user-invocable" => fm.user_invocable = val.as_bool(),
            "metadata" => {
                if let Some(meta) = val.as_mapping() {
                    fm.metadata = parse_metadata(meta);
                }
            }
            _ => {}
        }
    }
    Ok(fm)
}

fn parse_metadata(meta: &serde_yml::Mapping) -> Metadata {
    let mut out = Metadata::default();
    for (key, val) in meta {
        let key_str = match key.as_str() {
            Some(s) => s,
            None => continue,
        };
        match key_str {
            "author" => out.author = val.as_str().map(|s| s.to_string()),
            "version" => {
                out.version = val
                    .as_str()
                    .map(|s| s.to_string())
                    .or_else(|| val.as_f64().map(|n| n.to_string()))
                    .or_else(|| val.as_i64().map(|n| n.to_string()));
            }
            "category" => out.category = val.as_str().map(|s| s.to_string()),
            "triggers" => out.triggers = string_array(val),
            "siblings" => out.siblings = string_array(val),
            _ => {}
        }
    }
    out
}

fn string_array(value: &serde_yml::Value) -> Vec<String> {
    value
        .as_sequence()
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn collect_references(dir: &Path) -> Result<Vec<PathBuf>> {
    let refs_dir = dir.join("references");
    if !refs_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&refs_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_frontmatter_handles_basic_skill() {
        let raw = "---\nname: foo\ndescription: bar\n---\n# Body\nText\n";
        let (yaml, body) = split_frontmatter(raw);
        assert_eq!(yaml, "name: foo\ndescription: bar");
        assert_eq!(body, "# Body\nText\n");
    }

    #[test]
    fn parse_frontmatter_extracts_metadata() {
        let raw = "name: foo\ndescription: hello\nallowed-tools: Bash(unity-cli:*), Read\nmetadata:\n  author: a\n  version: 0.3.0\n  category: scenes\n  triggers:\n    - scene\n    - hierarchy\n  siblings:\n    - unity-scene-create\n";
        let fm = parse_frontmatter(raw).unwrap();
        assert_eq!(fm.name.as_deref(), Some("foo"));
        assert_eq!(fm.allowed_tools.as_deref(), Some("Bash(unity-cli:*), Read"));
        assert_eq!(fm.metadata.author.as_deref(), Some("a"));
        assert_eq!(fm.metadata.version.as_deref(), Some("0.3.0"));
        assert_eq!(fm.metadata.category.as_deref(), Some("scenes"));
        assert_eq!(fm.metadata.triggers, vec!["scene", "hierarchy"]);
        assert_eq!(fm.metadata.siblings, vec!["unity-scene-create"]);
    }
}
