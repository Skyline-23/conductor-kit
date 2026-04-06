use std::fs;
use std::path::Path;

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn expected_skill_shims_exist() {
    for relative in [
        "skills/conductor/SKILL.md",
        "skills/team/SKILL.md",
        "skills/ralph/SKILL.md",
        "skills/plan/SKILL.md",
        "skills/implement/SKILL.md",
        "skills/review/SKILL.md",
        "skills/symphony/SKILL.md",
    ] {
        assert!(
            repo_root().join(relative).is_file(),
            "missing required skill shim: {relative}"
        );
    }
}

#[test]
fn team_and_ralph_skills_force_cli_routing() {
    let team = fs::read_to_string(repo_root().join("skills/team/SKILL.md"))
        .expect("failed to read team skill");
    let ralph = fs::read_to_string(repo_root().join("skills/ralph/SKILL.md"))
        .expect("failed to read ralph skill");

    assert!(team.contains("conductor team <count> <profile> [profile...]"));
    assert!(team.contains("conductor team --prompt \"<current task>\""));
    assert!(team.contains("--prompt \"<current task>\""));
    assert!(team.contains("do not call built-in sub-agent or delegation tools"));
    assert!(team.contains("do not inspect the repo, reason about layout, or explain the command before running it"));
    assert!(ralph.contains("conductor ralph"));
    assert!(ralph.contains("do not call built-in sub-agent or delegation tools"));
}

#[test]
fn team_and_ralph_commands_exist() {
    for relative in ["commands/team.md", "commands/ralph.md"] {
        assert!(
            repo_root().join(relative).is_file(),
            "missing required command doc: {relative}"
        );
    }
}
