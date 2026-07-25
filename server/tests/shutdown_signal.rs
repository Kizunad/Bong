#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use bong_server::craft::{load_recipe_unlock_log, RecipeId};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "bong-shutdown-signal-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create unique shutdown signal test root");
        Self { path }
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct ChildGuard {
    child: Child,
}

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self { child }
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    fn wait_with_timeout(&mut self, timeout: Duration) -> std::process::ExitStatus {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self
                .child
                .try_wait()
                .expect("poll shutdown signal probe child")
            {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "shutdown signal probe child did not exit within {timeout:?}"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

fn wait_for_ready(child: &mut ChildGuard, ready_path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if ready_path.exists() {
            return;
        }
        if let Some(status) = child
            .child
            .try_wait()
            .expect("poll shutdown signal probe while waiting for readiness")
        {
            panic!(
                "shutdown signal probe exited before readiness; status={status}; ready_path={}",
                ready_path.display()
            );
        }
        assert!(
            Instant::now() < deadline,
            "shutdown signal probe did not create readiness file within 10s: {}",
            ready_path.display()
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn spawn_probe(root: &TempRoot) -> (ChildGuard, PathBuf) {
    let unlock_path = root.path.join("data/craft/recipe_unlocks.json");
    let ready_path = root.path.join("probe.ready");
    let child = Command::new(env!("CARGO_BIN_EXE_bong-server"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("BONG_SHUTDOWN_SIGNAL_PROBE", "1")
        .env("BONG_SHUTDOWN_SIGNAL_PROBE_UNLOCK_PATH", &unlock_path)
        .env("BONG_SHUTDOWN_SIGNAL_PROBE_READY_PATH", &ready_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn shutdown signal probe binary");
    let mut child = ChildGuard::new(child);
    wait_for_ready(&mut child, &ready_path);
    (child, unlock_path)
}

fn assert_signal_flushes_dirty_unlock(signal: &str, label: &str) {
    let root = TempRoot::new(label);
    let (mut child, unlock_path) = spawn_probe(&root);
    let tmp_path = unlock_path.with_extension("tmp");

    assert!(
        !unlock_path.exists() && !tmp_path.exists(),
        "probe must remain below its 600-tick runtime flush interval before receiving {signal}"
    );

    let status = Command::new("kill")
        .args([format!("-{signal}"), child.pid().to_string()])
        .status()
        .expect("send signal to shutdown probe");
    assert!(status.success(), "kill -{signal} must target probe child");

    let exit = child.wait_with_timeout(Duration::from_secs(10));
    assert!(
        exit.success(),
        "shutdown probe must exit normally after SIG{signal}; status={exit}"
    );
    assert!(
        unlock_path.exists(),
        "SIG{signal} must cause Last flush to persist the unlock log"
    );
    assert!(
        !tmp_path.exists(),
        "atomic shutdown flush must not leave recipe_unlocks.tmp after SIG{signal}"
    );

    let restored = load_recipe_unlock_log(&unlock_path)
        .unwrap_or_else(|error| panic!("SIG{signal} unlock log must hydrate: {error}"));
    assert_eq!(
        restored.version, 1,
        "hydrated shutdown log must retain schema version"
    );
    assert!(
        restored
            .by_player
            .get("offline:shutdown-probe")
            .is_some_and(|recipes| recipes.contains(&RecipeId::new("craft.probe.shutdown.flush"))),
        "SIG{signal} shutdown flush must persist the dirty probe recipe"
    );
}

#[test]
fn sigint_runs_production_shutdown_bridge_and_flushes_unlock_state() {
    assert_signal_flushes_dirty_unlock("INT", "sigint");
}

#[test]
fn sigterm_runs_production_shutdown_bridge_and_flushes_unlock_state() {
    assert_signal_flushes_dirty_unlock("TERM", "sigterm");
}
