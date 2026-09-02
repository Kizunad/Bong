use std::fs;
use std::io::ErrorKind;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use bong_server::server_readiness::publish;

struct TestDir(PathBuf);

impl TestDir {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "bong-server-readiness-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create readiness test directory");
        Self(path)
    }

    fn join(&self, path: &str) -> PathBuf {
        self.0.join(path)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn publish_writes_exact_private_pid_line_and_cleans_temporary() {
    let directory = TestDir::new("success");
    let ready_path = directory.join("server.ready");

    publish(&ready_path).expect("publish readiness");

    assert_eq!(
        fs::read_to_string(&ready_path).expect("read readiness"),
        format!("pid={}\n", std::process::id()),
        "readiness must contain one exact PID line"
    );
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(&ready_path)
            .expect("read readiness metadata")
            .mode()
            & 0o777,
        0o600,
        "readiness evidence must remain private"
    );
    assert_eq!(
        fs::read_dir(&directory.0)
            .expect("enumerate readiness directory")
            .count(),
        1,
        "successful publish must remove its temporary name"
    );
}

#[test]
fn publish_never_overwrites_preexisting_target() {
    let directory = TestDir::new("preexisting");
    let ready_path = directory.join("server.ready");
    fs::write(&ready_path, b"operator-owned\n").expect("seed readiness target");

    let error = publish(&ready_path).expect_err("preexisting target must be rejected");

    assert_eq!(error.kind(), ErrorKind::AlreadyExists);
    assert_eq!(
        fs::read(&ready_path).expect("read preserved target"),
        b"operator-owned\n",
        "atomic no-replace publish must preserve the existing target"
    );
    assert_eq!(
        fs::read_dir(&directory.0)
            .expect("enumerate readiness directory")
            .count(),
        1,
        "failed publish must clean its temporary file"
    );
}

#[test]
fn concurrent_publishers_have_one_winner_without_overwrite() {
    let directory = TestDir::new("concurrent");
    let ready_path = Arc::new(directory.join("server.ready"));
    let barrier = Arc::new(Barrier::new(3));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let ready_path = Arc::clone(&ready_path);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            publish(&ready_path)
        }));
    }
    barrier.wait();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("publisher thread must not panic"))
        .collect();

    assert_eq!(
        results.iter().filter(|result| result.is_ok()).count(),
        1,
        "exactly one concurrent publisher must win"
    );
    assert_eq!(
        results
            .iter()
            .filter_map(|result| result.as_ref().err())
            .filter(|error| error.kind() == ErrorKind::AlreadyExists)
            .count(),
        1,
        "the losing publisher must observe AlreadyExists"
    );
    assert_eq!(
        fs::read_to_string(&*ready_path).expect("read readiness"),
        format!("pid={}\n", std::process::id())
    );
    assert_eq!(
        fs::read_dir(&directory.0)
            .expect("enumerate readiness directory")
            .count(),
        1,
        "concurrent publish must leave no temporary names"
    );
}

#[test]
fn publish_skips_colliding_temporary_name() {
    let directory = TestDir::new("temp-collision");
    let ready_path = directory.join("server.ready");
    let first_temporary = directory
        .0
        .join(format!(".server.ready.{}.0.tmp", std::process::id()));
    fs::write(&first_temporary, b"foreign-temp\n").expect("seed temporary collision");

    publish(&ready_path).expect("publisher must retry a temporary collision");

    assert_eq!(
        fs::read(&first_temporary).expect("read preserved temporary"),
        b"foreign-temp\n",
        "publisher must not remove a temporary file it did not create"
    );
    assert!(ready_path.is_file(), "readiness target must still publish");
}

#[test]
fn publish_rejects_path_without_filename() {
    let error =
        publish(Path::new("/")).expect_err("directory-only readiness path must be rejected");
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
}
