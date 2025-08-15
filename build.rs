use std::env;
use std::process::Command;

fn main() {
    // Get Git commit SHA - env var overrides .git
    let git_commit_sha = env::var("GIT_COMMIT_SHA")
        .unwrap_or_else(|_| get_git_commit_sha().unwrap_or_else(|| "unknown".to_string()));

    println!("cargo:rustc-env=GIT_COMMIT_SHA={git_commit_sha}");

    // Get Git dirty status - env var overrides .git status
    let git_dirty = env::var("GIT_DIRTY")
        .unwrap_or_else(|_| get_git_dirty_status().unwrap_or_else(|| "unknown".to_string()));

    println!("cargo:rustc-env=GIT_DIRTY={git_dirty}");

    // Get build time - SOURCE_DATE_EPOCH overrides current time
    let build_time = env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|epoch| epoch.parse::<i64>().ok())
        .and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0))
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    println!("cargo:rustc-env=BUILT_TIME_UTC={build_time}");

    // Tell cargo to rerun this script if .git/HEAD changes
    println!("cargo:rerun-if-changed=.git/HEAD");

    // Also watch the current branch ref file for new commits
    if let Ok(head_content) = std::fs::read_to_string(".git/HEAD")
        && let Some(branch_ref) = head_content.strip_prefix("ref: ").map(|s| s.trim())
    {
        println!("cargo:rerun-if-changed=.git/{branch_ref}");
    }
    // Also rerun if env vars change
    println!("cargo:rerun-if-env-changed=GIT_COMMIT_SHA");
    println!("cargo:rerun-if-env-changed=GIT_DIRTY");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
}

fn get_git_commit_sha() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn get_git_dirty_status() -> Option<String> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()?;

    if output.status.success() {
        let is_dirty = !output.stdout.is_empty();
        Some(if is_dirty { "true" } else { "false" }.to_string())
    } else {
        None
    }
}
