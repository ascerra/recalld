//! `recalld repair-legacy-vectors` against a synthetic released-1.0 store only.
use std::process::Command;

fn recalld(home: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_recalld"));
    command.env_clear().env("HOME", home).current_dir(home);
    command
}

#[test]
fn repair_subcommand_requires_explicit_config_and_plans_without_provider_calls() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("legacy");
    zip::ZipArchive::new(std::io::Cursor::new(include_bytes!(
        "../src/storage/fixtures/released-v1.zip"
    )))
    .unwrap()
    .extract(&source)
    .unwrap();
    let manifest = root.path().join("plan.json");

    let out = recalld(root.path())
        .args(["repair-legacy-vectors", "plan"])
        .arg(&source)
        .arg("--manifest")
        .arg(&manifest)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("requires an explicit --config"));
    assert!(!manifest.exists());

    // Nothing listens on port 9; plan must never contact the provider.
    let config = root.path().join("recalld.toml");
    std::fs::write(
        &config,
        "[embedding]\nprovider = \"ollama\"\nbase_url = \"http://127.0.0.1:9\"\n",
    )
    .unwrap();
    let out = recalld(root.path())
        .arg("--config")
        .arg(&config)
        .args(["repair-legacy-vectors", "plan"])
        .arg(&source)
        .arg("--manifest")
        .arg(&manifest)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["provider_calls"], 0);
    assert_eq!(result["repair_count"], 0);
    assert_eq!(result["drop_count"], 0);
    let written: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
    assert_eq!(written["version"], 1);
    assert_eq!(written["endpoint"], "http://127.0.0.1:9");
}
