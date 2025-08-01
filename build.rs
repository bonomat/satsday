use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=migrations");
    
    // Get git commit hash
    let output = Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .unwrap();
    let git_hash = String::from_utf8(output.stdout).unwrap();
    println!("cargo:rustc-env=GIT_HASH={}", git_hash.trim());
    
    // Get current date and time
    let now = chrono::Utc::now();
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", now.to_rfc3339());
}
