use std::env;
use std::process::Command;

fn main() {
    emit_git_commit_sha();
    emit_git_dirty_status();
    emit_built_time_utc();

    // Tell cargo to rerun this script if .git/HEAD changes
    println!("cargo:rerun-if-changed=.git/HEAD");
    // Also watch the current branch ref file for new commits
    if let Ok(head_content) = std::fs::read_to_string(".git/HEAD")
        && let Some(branch_ref) = head_content.strip_prefix("ref: ").map(|s| s.trim())
    {
        println!("cargo:rerun-if-changed=.git/{branch_ref}");
    }
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

fn get_git_commit_sha_from_packaged() -> Option<String> {
    let mut packaged = std::path::PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    packaged.push(".packaged-commit");
    std::fs::read_to_string(&packaged)
        .ok()
        .and_then(|contents| contents.trim().to_string().into())
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

fn emit_git_commit_sha() {
    println!("cargo:rerun-if-env-changed=GIT_COMMIT_SHA");

    // Get Git commit SHA
    // - packaged commit SHA overrides env var
    // - env var overrides .git
    let git_commit_sha = get_git_commit_sha_from_packaged()
        .or_else(|| env::var("GIT_COMMIT_SHA").ok())
        .or_else(get_git_commit_sha)
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GIT_COMMIT_SHA={git_commit_sha}");
}

fn emit_git_dirty_status() {
    println!("cargo:rerun-if-env-changed=GIT_DIRTY");

    // Get Git dirty status - env var overrides .git
    let git_dirty = env::var("GIT_DIRTY").unwrap_or_else(|_| {
        // fallback to running git command if not set
        get_git_dirty_status().unwrap_or_else(|| "unknown".to_string())
    });

    println!("cargo:rustc-env=GIT_DIRTY={git_dirty}");
}

fn emit_built_time_utc() {
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");

    // Get build time - SOURCE_DATE_EPOCH overrides current time
    let build_time = env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|epoch| epoch.parse::<i64>().ok())
        .and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0))
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| {
            // fallback to current time if SOURCE_DATE_EPOCH is not set
            chrono::Utc::now().to_rfc3339()
        });

    println!("cargo:rustc-env=BUILT_TIME_UTC={build_time}");
}
