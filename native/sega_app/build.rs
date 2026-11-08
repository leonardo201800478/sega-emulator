use std::process::Command;

fn main() {
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "unknown".to_owned());
    let git_sha = std::env::var("GITHUB_SHA").unwrap_or_else(|_| {
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
            .filter(|sha| !sha.is_empty())
            .unwrap_or_else(|| "unknown".to_owned())
    });

    println!("cargo:rustc-env=SEGA_BUILD_PROFILE={profile}");
    println!("cargo:rustc-env=SEGA_GIT_SHA={git_sha}");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
}
