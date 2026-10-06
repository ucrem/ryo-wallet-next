use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .ok()?;
    output.status.success().then_some(())?;
    Some(String::from_utf8(output.stdout).ok()?.trim().to_owned())
}

fn main() {
    let frontend_dev = tauri_build::is_dev();
    println!("cargo:rerun-if-env-changed=PROFILE");
    if std::env::var("PROFILE").as_deref() == Ok("release") && frontend_dev {
        panic!(
            "Standalone release requires embedded frontend assets. Run `pnpm exec tauri build --ci --no-bundle`; plain `cargo build --release` would require the Vite development server."
        );
    }
    println!(
        "cargo:rustc-env=RYO_FRONTEND_MODE={}",
        if frontend_dev {
            "development_server"
        } else {
            "embedded"
        }
    );
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let root = Path::new(&manifest).parent().unwrap();
    let revision = git(root, &["rev-parse", "--verify", "HEAD"]).filter(|value| {
        matches!(value.len(), 40 | 64) && value.bytes().all(|b| b.is_ascii_hexdigit())
    });
    println!(
        "cargo:rustc-env=RYO_BUILD_REVISION={}",
        revision.as_deref().unwrap_or("unknown")
    );
    let modified = Command::new("git")
        .current_dir(root)
        .args(["diff", "--quiet", "HEAD", "--"])
        .status()
        .ok()
        .and_then(|status| match status.code() {
            Some(0) => Some("false"),
            Some(1) => Some("true"),
            _ => None,
        });
    println!(
        "cargo:rustc-env=RYO_BUILD_SOURCE_MODIFIED={}",
        modified.unwrap_or("unknown")
    );
    println!(
        "cargo:rustc-env=RYO_BUILD_TARGET={}",
        std::env::var("TARGET").unwrap()
    );
    // Watch real Git metadata, including linked worktrees and detached checkouts.
    let mut metadata = vec![
        "HEAD".to_owned(),
        "index".to_owned(),
        "packed-refs".to_owned(),
    ];
    if let Some(reference) = git(root, &["symbolic-ref", "--quiet", "HEAD"]) {
        metadata.push(reference);
    }
    for entry in metadata {
        if let Some(path) = git(root, &["rev-parse", "--git-path", &entry]) {
            let path = root.join(path);
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    tauri_build::build()
}
