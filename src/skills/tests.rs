//! Unit tests for the skill linter rules (R01..R22).

#![cfg(test)]

use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use super::loader::load_skills;
use super::rules::{run_all, RuleContext};

const VALID_DESC: &str = "Manage Unity prefab assets with unity-cli. Use when the user asks to create a prefab from a scene object or open a prefab in edit mode. Do not use for general scene object editing; use `unity-gameobject-edit` instead.";

fn write_skill(dir: &Path, name: &str, frontmatter: &str, body: &str) -> PathBuf {
    let skill_dir = dir.join(name);
    fs::create_dir_all(skill_dir.join("references")).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\n{frontmatter}---\n\n{body}\n"),
    )
    .unwrap();
    fs::write(
        skill_dir.join("references/runtime-checklist.md"),
        "# Runtime\n\nMinimal placeholder.\n",
    )
    .unwrap();
    skill_dir
}

fn valid_frontmatter(name: &str) -> String {
    format!(
        "name: {name}\ndescription: {VALID_DESC}\nallowed-tools: Bash(unity-cli:*), Read, Grep, Glob\nmetadata:\n  author: tester\n  version: 0.3.0\n  category: prefabs\n  triggers:\n    - prefab\n  siblings:\n    - unity-gameobject-edit\n"
    )
}

fn sibling_frontmatter(name: &str, partner: &str) -> String {
    format!(
        "name: {name}\ndescription: Edit GameObjects with unity-cli. Use when the user asks to rename a gameobject. Do not use for prefab edit mode; use `{partner}`.\nallowed-tools: Bash(unity-cli:*), Read, Grep, Glob\nmetadata:\n  author: tester\n  version: 0.3.0\n  category: scenes\n  triggers:\n    - gameobject\n  siblings:\n    - {partner}\n"
    )
}

fn valid_body() -> String {
    [
        "# Test Skill",
        "",
        "## Use When",
        "- The user asks for prefabs.",
        "",
        "## Do Not Use When",
        "- Anything else.",
        "",
        "## Preferred Flow",
        "1. Step one.",
        "",
        "## Examples",
        "- An example.",
        "",
        "## References",
        "- [runtime-checklist.md](references/runtime-checklist.md)",
    ]
    .join("\n")
}

fn lint_dir(root: &Path) -> Vec<String> {
    let skills = load_skills(root).unwrap();
    let ctx = RuleContext {
        skills: &skills,
        repo_root: root,
    };
    let mut out = Vec::new();
    for skill in &skills {
        for v in run_all(skill, &ctx) {
            out.push(v.rule.clone());
        }
    }
    out
}

#[test]
fn good_skill_passes_all_rules() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "unity-prefab-workflow",
        &valid_frontmatter("unity-prefab-workflow"),
        &valid_body(),
    );
    write_skill(
        tmp.path(),
        "unity-gameobject-edit",
        &sibling_frontmatter("unity-gameobject-edit", "unity-prefab-workflow"),
        &valid_body(),
    );
    let violations = lint_dir(tmp.path());
    assert_eq!(violations, Vec::<String>::new(), "expected zero violations");
}

#[test]
fn r01_missing_description_fires() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "unity-broken",
        "name: unity-broken\nallowed-tools: Bash(unity-cli:*), Read, Grep, Glob\nmetadata:\n  author: tester\n  version: 0.3.0\n  category: scenes\n  triggers:\n    - scene\n",
        &valid_body(),
    );
    let violations = lint_dir(tmp.path());
    assert!(
        violations.iter().any(|r| r == "R01"),
        "expected R01, got {violations:?}"
    );
}

#[test]
fn r02_name_mismatch_fires() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "unity-mismatch",
        &valid_frontmatter("unity-other-name"),
        &valid_body(),
    );
    let violations = lint_dir(tmp.path());
    assert!(violations.iter().any(|r| r == "R02"));
}

#[test]
fn r03_disallowed_tool_fires() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "unity-tooltest",
        "name: unity-tooltest\ndescription: Manage Unity assets with unity-cli. Use when the user asks for assets. Do not use for code; use `unity-gameobject-edit`.\nallowed-tools: Bash(unity-cli:*), Read, WebFetch\nmetadata:\n  author: tester\n  version: 0.3.0\n  category: assets\n  triggers:\n    - asset\n  siblings:\n    - unity-gameobject-edit\n",
        &valid_body(),
    );
    let violations = lint_dir(tmp.path());
    assert!(violations.iter().any(|r| r == "R03"));
}

#[test]
fn r03_legacy_bare_bash_fires() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "unity-bashtest",
        "name: unity-bashtest\ndescription: Manage Unity assets with unity-cli. Use when the user asks for assets. Do not use for code; use `unity-gameobject-edit`.\nallowed-tools: Bash, Read\nmetadata:\n  author: tester\n  version: 0.3.0\n  category: assets\n  triggers:\n    - asset\n  siblings:\n    - unity-gameobject-edit\n",
        &valid_body(),
    );
    let violations = lint_dir(tmp.path());
    assert!(violations.iter().any(|r| r == "R03"));
}

#[test]
fn r04_unknown_category_fires() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "unity-cattest",
        "name: unity-cattest\ndescription: Manage Unity prefabs with unity-cli. Use when the user asks for prefabs. Do not use for assets; use `unity-gameobject-edit`.\nallowed-tools: Bash(unity-cli:*), Read\nmetadata:\n  author: tester\n  version: 0.3.0\n  category: nonsense\n  triggers:\n    - prefab\n  siblings:\n    - unity-gameobject-edit\n",
        &valid_body(),
    );
    let violations = lint_dir(tmp.path());
    assert!(violations.iter().any(|r| r == "R04"));
}

#[test]
fn r05_uppercase_trigger_fires() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "unity-uppertest",
        "name: unity-uppertest\ndescription: Manage Unity prefabs with unity-cli. Use when the user asks for prefabs. Do not use for assets; use `unity-gameobject-edit`.\nallowed-tools: Bash(unity-cli:*), Read\nmetadata:\n  author: tester\n  version: 0.3.0\n  category: prefabs\n  triggers:\n    - Prefab\n  siblings:\n    - unity-gameobject-edit\n",
        &valid_body(),
    );
    let violations = lint_dir(tmp.path());
    assert!(violations.iter().any(|r| r == "R05"));
}

#[test]
fn r06_unknown_sibling_fires() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "unity-orphan",
        "name: unity-orphan\ndescription: Manage Unity prefabs with unity-cli. Use when the user asks for prefabs. Do not use for the imaginary `unity-ghost`.\nallowed-tools: Bash(unity-cli:*), Read\nmetadata:\n  author: tester\n  version: 0.3.0\n  category: prefabs\n  triggers:\n    - prefab\n  siblings:\n    - unity-ghost\n",
        &valid_body(),
    );
    let violations = lint_dir(tmp.path());
    assert!(violations.iter().any(|r| r == "R06"));
}

#[test]
fn r07_too_long_description_fires() {
    let tmp = TempDir::new().unwrap();
    let long = "Manage Unity prefabs with unity-cli. ".repeat(40);
    write_skill(
        tmp.path(),
        "unity-toolong",
        &format!("name: unity-toolong\ndescription: {long} Use when the user asks. Do not use for `unity-gameobject-edit`.\nallowed-tools: Bash(unity-cli:*), Read\nmetadata:\n  author: tester\n  version: 0.3.0\n  category: prefabs\n  triggers:\n    - prefab\n  siblings:\n    - unity-gameobject-edit\n"),
        &valid_body(),
    );
    let violations = lint_dir(tmp.path());
    assert!(violations.iter().any(|r| r == "R07"));
}

#[test]
fn r08_missing_front_trigger_fires() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "unity-noheadtrig",
        "name: unity-noheadtrig\ndescription: Drive Unity Editor automation with unity-cli. Use when the user asks to do something arbitrary in the editor without using the trigger word at all so the head test can fail. Do not use for `unity-gameobject-edit`.\nallowed-tools: Bash(unity-cli:*), Read\nmetadata:\n  author: tester\n  version: 0.3.0\n  category: editor\n  triggers:\n    - obscuretoken\n  siblings:\n    - unity-gameobject-edit\n",
        &valid_body(),
    );
    let violations = lint_dir(tmp.path());
    assert!(violations.iter().any(|r| r == "R08"));
}

#[test]
fn r09_first_person_fires() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "unity-person",
        "name: unity-person\ndescription: I can help you manage Unity prefabs with unity-cli. Use when the user asks for prefabs. Do not use for `unity-gameobject-edit`.\nallowed-tools: Bash(unity-cli:*), Read\nmetadata:\n  author: tester\n  version: 0.3.0\n  category: prefabs\n  triggers:\n    - prefab\n  siblings:\n    - unity-gameobject-edit\n",
        &valid_body(),
    );
    let violations = lint_dir(tmp.path());
    assert!(violations.iter().any(|r| r == "R09"));
}

#[test]
fn r10_missing_use_when_fires() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "unity-nousewhen",
        "name: unity-nousewhen\ndescription: Manage Unity prefabs with unity-cli. Do not use for `unity-gameobject-edit`.\nallowed-tools: Bash(unity-cli:*), Read\nmetadata:\n  author: tester\n  version: 0.3.0\n  category: prefabs\n  triggers:\n    - prefab\n  siblings:\n    - unity-gameobject-edit\n",
        &valid_body(),
    );
    let violations = lint_dir(tmp.path());
    assert!(violations.iter().any(|r| r == "R10"));
}

#[test]
fn r11_missing_sibling_mention_fires() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "unity-nomention",
        "name: unity-nomention\ndescription: Manage Unity prefabs with unity-cli. Use when the user asks for prefabs. Do not use otherwise.\nallowed-tools: Bash(unity-cli:*), Read\nmetadata:\n  author: tester\n  version: 0.3.0\n  category: prefabs\n  triggers:\n    - prefab\n  siblings:\n    - unity-gameobject-edit\n",
        &valid_body(),
    );
    write_skill(
        tmp.path(),
        "unity-gameobject-edit",
        &sibling_frontmatter("unity-gameobject-edit", "unity-nomention"),
        &valid_body(),
    );
    let violations = lint_dir(tmp.path());
    assert!(violations.iter().any(|r| r == "R11"));
}

#[test]
fn r12_unidirectional_sibling_fires() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "unity-one",
        "name: unity-one\ndescription: Manage Unity prefabs with unity-cli. Use when the user asks for prefabs. Do not use for `unity-two`.\nallowed-tools: Bash(unity-cli:*), Read\nmetadata:\n  author: tester\n  version: 0.3.0\n  category: prefabs\n  triggers:\n    - prefab\n  siblings:\n    - unity-two\n",
        &valid_body(),
    );
    write_skill(
        tmp.path(),
        "unity-two",
        "name: unity-two\ndescription: Manage Unity assets with unity-cli. Use when the user asks for assets. Do not use for `unity-three`.\nallowed-tools: Bash(unity-cli:*), Read\nmetadata:\n  author: tester\n  version: 0.3.0\n  category: assets\n  triggers:\n    - asset\n  siblings:\n    - unity-three\n",
        &valid_body(),
    );
    write_skill(
        tmp.path(),
        "unity-three",
        "name: unity-three\ndescription: Manage Unity assets with unity-cli. Use when the user asks for assets. Do not use for `unity-two`.\nallowed-tools: Bash(unity-cli:*), Read\nmetadata:\n  author: tester\n  version: 0.3.0\n  category: assets\n  triggers:\n    - asset-mgmt\n  siblings:\n    - unity-two\n",
        &valid_body(),
    );
    let violations = lint_dir(tmp.path());
    assert!(violations.iter().any(|r| r == "R12"));
}

#[test]
fn r13_too_long_body_fires() {
    let tmp = TempDir::new().unwrap();
    let body_lines: Vec<String> = (0..600).map(|i| format!("line {i}")).collect();
    let body = format!(
        "# Title\n\n## Use When\n- a\n## Do Not Use When\n- b\n## Preferred Flow\n1. c\n## Examples\n- d\n## References\n- [r](references/runtime-checklist.md)\n\n{}\n",
        body_lines.join("\n")
    );
    write_skill(
        tmp.path(),
        "unity-bigbody",
        &valid_frontmatter("unity-bigbody"),
        &body,
    );
    write_skill(
        tmp.path(),
        "unity-gameobject-edit",
        &sibling_frontmatter("unity-gameobject-edit", "unity-bigbody"),
        &valid_body(),
    );
    let violations = lint_dir(tmp.path());
    assert!(violations.iter().any(|r| r == "R13"));
}

#[test]
fn r14_missing_heading_fires() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "unity-noheading",
        &valid_frontmatter("unity-noheading"),
        "# Title\n\n## Use When\n- a\n## Examples\n- e\n## References\n- [r](references/runtime-checklist.md)\n",
    );
    write_skill(
        tmp.path(),
        "unity-gameobject-edit",
        &sibling_frontmatter("unity-gameobject-edit", "unity-noheading"),
        &valid_body(),
    );
    let violations = lint_dir(tmp.path());
    assert!(violations.iter().any(|r| r == "R14"));
}

#[test]
fn r15_references_without_link_fires() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "unity-emptyrefs",
        &valid_frontmatter("unity-emptyrefs"),
        "# Title\n\n## Use When\n- a\n## Do Not Use When\n- b\n## Preferred Flow\n1. c\n## Examples\n- d\n## References\n\nNo links here.\n",
    );
    write_skill(
        tmp.path(),
        "unity-gameobject-edit",
        &sibling_frontmatter("unity-gameobject-edit", "unity-emptyrefs"),
        &valid_body(),
    );
    let violations = lint_dir(tmp.path());
    assert!(violations.iter().any(|r| r == "R15"));
}

#[test]
fn r16_time_sensitive_phrase_fires() {
    let tmp = TempDir::new().unwrap();
    write_skill(
        tmp.path(),
        "unity-timed",
        &valid_frontmatter("unity-timed"),
        "# Title\n\n## Use When\n- as of yesterday this works\n## Do Not Use When\n- b\n## Preferred Flow\n1. c\n## Examples\n- d\n## References\n- [r](references/runtime-checklist.md)\n",
    );
    write_skill(
        tmp.path(),
        "unity-gameobject-edit",
        &sibling_frontmatter("unity-gameobject-edit", "unity-timed"),
        &valid_body(),
    );
    let violations = lint_dir(tmp.path());
    assert!(violations.iter().any(|r| r == "R16"));
}

#[test]
fn r17_missing_runtime_checklist_fires() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("unity-noruntime");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        format!(
            "---\n{}---\n\n{}\n",
            valid_frontmatter("unity-noruntime"),
            valid_body()
        ),
    )
    .unwrap();
    write_skill(
        tmp.path(),
        "unity-gameobject-edit",
        &sibling_frontmatter("unity-gameobject-edit", "unity-noruntime"),
        &valid_body(),
    );
    let violations = lint_dir(tmp.path());
    assert!(violations.iter().any(|r| r == "R17"));
}

#[test]
fn r22_collision_without_cross_listing_fires() {
    let tmp = TempDir::new().unwrap();
    // Two skills share trigger `prefab` but are not siblings of each other.
    write_skill(
        tmp.path(),
        "unity-x-one",
        "name: unity-x-one\ndescription: Manage Unity prefabs with unity-cli. Use when the user asks for prefab. Do not use for `unity-x-other`.\nallowed-tools: Bash(unity-cli:*), Read\nmetadata:\n  author: tester\n  version: 0.3.0\n  category: prefabs\n  triggers:\n    - prefab\n  siblings:\n    - unity-x-other\n",
        &valid_body(),
    );
    write_skill(
        tmp.path(),
        "unity-x-other",
        "name: unity-x-other\ndescription: Manage Unity prefabs with unity-cli. Use when the user asks for prefab. Do not use for `unity-x-one`.\nallowed-tools: Bash(unity-cli:*), Read\nmetadata:\n  author: tester\n  version: 0.3.0\n  category: prefabs\n  triggers:\n    - prefab\n  siblings:\n    - unity-x-one\n",
        &valid_body(),
    );
    // Third skill also uses `prefab` but does not cross-list.
    write_skill(
        tmp.path(),
        "unity-x-third",
        "name: unity-x-third\ndescription: Manage Unity prefabs with unity-cli. Use when the user asks for prefab. Do not use for `unity-x-one`.\nallowed-tools: Bash(unity-cli:*), Read\nmetadata:\n  author: tester\n  version: 0.3.0\n  category: prefabs\n  triggers:\n    - prefab\n  siblings:\n    - unity-x-one\n",
        &valid_body(),
    );
    let violations = lint_dir(tmp.path());
    assert!(
        violations.iter().any(|r| r == "R22"),
        "expected R22, got {violations:?}"
    );
}
