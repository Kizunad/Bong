use std::fs::{self, File, OpenOptions};
use std::io::{Error, ErrorKind, Result, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

const MAX_TEMPORARY_ATTEMPTS: u32 = 32;

pub fn publish_if_requested_from_env() {
    let Some(ready_path) = std::env::var_os("BONG_SERVER_READY_PATH") else {
        return;
    };
    publish(Path::new(&ready_path)).unwrap_or_else(|error| {
        panic!(
            "publish production server readiness at {} failed: {error}",
            Path::new(&ready_path).display()
        )
    });
}

/// Publishes the readiness marker through the production atomic publication path.
#[doc(hidden)]
pub fn publish(ready_path: &Path) -> Result<()> {
    let parent = ready_path.parent().ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidInput,
            "readiness path must have a parent directory",
        )
    })?;
    let file_name = ready_path
        .file_name()
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "readiness path must name a file"))?;
    let parent_metadata = fs::metadata(parent)?;
    if !parent_metadata.is_dir() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "readiness parent must be a directory",
        ));
    }

    for attempt in 0..MAX_TEMPORARY_ATTEMPTS {
        let temporary = parent.join(format!(
            ".{}.{}.{}.tmp",
            file_name.to_string_lossy(),
            std::process::id(),
            attempt
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut temporary_file = match options.open(&temporary) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };

        let result = publish_created_temporary(
            parent,
            ready_path,
            &temporary,
            &mut temporary_file,
            std::process::id(),
        );
        drop(temporary_file);
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        return result;
    }

    Err(Error::new(
        ErrorKind::AlreadyExists,
        "could not allocate a private readiness temporary file",
    ))
}

fn publish_created_temporary(
    parent: &Path,
    ready_path: &Path,
    temporary: &Path,
    temporary_file: &mut File,
    pid: u32,
) -> Result<()> {
    writeln!(temporary_file, "pid={pid}")?;
    temporary_file.sync_all()?;
    fs::hard_link(temporary, ready_path)?;
    fs::remove_file(temporary)?;
    File::open(parent)?.sync_all()
}
