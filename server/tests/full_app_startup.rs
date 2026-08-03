use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const SOURCE_ROOT: &str = env!("CARGO_MANIFEST_DIR");
const SOURCE_ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");

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

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap_or_else(|error| {
        panic!(
            "create copied asset directory {} failed: {error}",
            destination.display()
        )
    });
    for entry in fs::read_dir(source).unwrap_or_else(|error| {
        panic!(
            "read source asset directory {} failed: {error}",
            source.display()
        )
    }) {
        let entry = entry.expect("source asset directory entry must be readable");
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry
            .file_type()
            .expect("source asset file type must be readable")
            .is_dir()
        {
            copy_tree(&source_path, &destination_path);
        } else {
            fs::copy(&source_path, &destination_path).unwrap_or_else(|error| {
                panic!(
                    "copy asset {} to {} failed: {error}",
                    source_path.display(),
                    destination_path.display()
                )
            });
        }
    }
}

fn copied_assets_root(name: &str) -> PathBuf {
    let root = unique_test_directory(name);
    copy_tree(Path::new(SOURCE_ASSETS), &root.join("assets"));
    root
}

fn run_full_app_startup(assets_root: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_bong-server"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("BONG_ASSETS_DIR", assets_root)
        .env("BONG_FULL_APP_STARTUP_SMOKE", "1")
        .env("BONG_SKIP_SKIN_PREFETCH", "1")
        .env("REDIS_URL", "redis://127.0.0.1:1")
        .output()
        .expect("startup smoke binary should launch")
}

fn assert_startup_succeeds(output: &std::process::Output) {
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
fn full_app_startup_smoke_initializes_core_resources_and_ticks_once() {
    let output = run_full_app_startup(Path::new(SOURCE_ROOT));
    assert_startup_succeeds(&output);
}

#[test]
fn full_app_startup_smoke_accepts_data_only_technique_extension() {
    let assets_root = copied_assets_root("technique-extension");
    let techniques_path = assets_root.join("assets/cultivation/techniques.toml");
    let mut techniques =
        fs::read_to_string(&techniques_path).expect("copied technique catalog must be readable");
    techniques.push_str(
        r#"

[[techniques]]
id = "test.data_only_startup_smoke"
display_name = "数据扩展探针"
grade = "common"
description = "仅由 TOML 增加，用于证明完整 App 启动不钉死历史功法集合。"
required_realm = "Awaken"
required_meridians = []
required_race = { kind = "any" }
qi_cost = 0.0
stamina_cost = 0.0
cast_ticks = 0
cooldown_ticks = 0
range = 0.0
icon_texture = "bong-client:textures/gui/items/skill_scroll_movement_dash.png"
category = "buff"
dispatch = "direct_generic"
"#,
    );
    fs::write(&techniques_path, techniques).expect("extended technique catalog must be writable");

    let output = run_full_app_startup(&assets_root);
    fs::remove_dir_all(&assets_root).expect("remove copied assets after startup smoke");

    assert_startup_succeeds(&output);
}

#[test]
fn full_app_startup_smoke_rejects_dangling_technique_scroll_reference() {
    let assets_root = copied_assets_root("dangling-scroll");
    let items_path = assets_root.join("assets/items/dangling_scroll_test.toml");
    fs::write(
        &items_path,
        r#"
[[item]]
id = "test_dangling_scroll"
name = "悬空残卷"
category = "scroll"
grid_w = 1
grid_h = 1
base_weight = 0.1
rarity = "common"
spirit_quality_initial = 0.1
description = "只用于启动引用完整性回归。"
[item.technique_scroll]
skill_id = "missing.runtime.technique"
"#,
    )
    .expect("dangling scroll catalog must be writable");

    let output = run_full_app_startup(&assets_root);
    fs::remove_dir_all(&assets_root).expect("remove copied assets after startup smoke");

    assert!(
        !output.status.success(),
        "startup must reject a dangling technique-scroll reference; status={:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("startup rejected technique scroll references"),
        "startup failure must identify the scroll-reference validator; output:\n{combined}"
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
