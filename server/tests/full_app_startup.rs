use std::process::Command;

#[test]
fn full_app_startup_smoke_initializes_core_resources_and_ticks_once() {
    let output = Command::new(env!("CARGO_BIN_EXE_bong-server"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("BONG_FULL_APP_STARTUP_SMOKE", "1")
        .env("BONG_SKIP_SKIN_PREFETCH", "1")
        .env("REDIS_URL", "redis://127.0.0.1:1")
        .output()
        .expect("startup smoke binary should launch");

    assert!(
        output.status.success(),
        "startup smoke should exit successfully; status={:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("full app startup smoke ok"),
        "startup smoke should print success marker; stdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}
