use std::process::Command;

fn main() {
    slint_build::compile("ui/launcher.slint").expect("ui/launcher.slint");
    println!("cargo:rerun-if-changed=../../.github/CHANGELOG.md");
    println!("cargo:rerun-if-env-changed=NV_PLUS_VERSION");
    // HEAD's reflog changes on every commit and checkout, so the sha below stays current.
    for file in ["HEAD", "logs/HEAD"] {
        if let Some(path) = git(&["rev-parse", "--git-path", file]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }

    let version = std::env::var("NV_PLUS_VERSION")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| git(&["rev-parse", "--short", "HEAD"]))
        .unwrap_or_else(|| "dev".into());
    println!("cargo:rustc-env=NV_PLUS_VERSION={version}");
}

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (out.status.success() && !text.is_empty()).then_some(text)
}
