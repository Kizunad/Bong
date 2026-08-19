use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_test_directory(name: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "bong-full-app-startup-{name}-{}-{nonce}",
        std::process::id()
    ))
}

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

#[test]
fn production_readiness_is_published_by_poststartup() {
    let directory = unique_test_directory("readiness");
    fs::create_dir(&directory).expect("create readiness smoke directory");
    let ready_path = directory.join("server.ready");

    let output = Command::new(env!("CARGO_BIN_EXE_bong-server"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("BONG_FULL_APP_STARTUP_SMOKE", "1")
        .env("BONG_SKIP_SKIN_PREFETCH", "1")
        .env("BONG_SERVER_READY_PATH", &ready_path)
        .env("REDIS_URL", "redis://127.0.0.1:1")
        .output()
        .expect("readiness smoke binary should launch");

    assert!(
        output.status.success(),
        "readiness smoke should exit successfully; status={:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let readiness = fs::read_to_string(&ready_path)
        .expect("PostStartup must publish readiness before the smoke exits");
    assert!(
        readiness.starts_with("pid=")
            && readiness.ends_with('\n')
            && readiness.lines().count() == 1,
        "readiness must contain one exact PID line, got {readiness:?}"
    );
    let pid = readiness
        .strip_prefix("pid=")
        .and_then(|line| line.trim_end().parse::<u32>().ok())
        .expect("readiness PID must be decimal");
    assert!(pid > 0, "readiness PID must be positive");

    fs::remove_dir_all(&directory).expect("remove readiness smoke directory");
}
