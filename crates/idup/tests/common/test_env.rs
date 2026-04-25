use assert_cmd::Command;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

/// Isolated test environment for a single integration test.
///
/// Holds a `TempDir` (auto-cleaned on `Drop`) with the following layout:
///
/// ```text
/// <tmp>/
///   db/
///     idup.db3    ← pointed at by IDUP_DB_PATH
///   imgs/         ← place test images here
/// ```
///
/// Use `env.cmd()` to get a pre-configured `assert_cmd::Command` that
/// already has `IDUP_DB_PATH` set so every invocation talks to the
/// isolated database.
pub struct TestEnv {
    /// Kept alive so the temp dir isn't deleted while the test runs.
    _tmp: TempDir,
    /// Full path to the SQLite database file.
    pub db_path: PathBuf,
    /// Directory where test images can be written.
    pub img_dir: PathBuf,
}

impl TestEnv {
    /// Create a fresh, isolated test environment.
    pub fn new() -> Self {
        let tmp = tempdir().expect("failed to create temp dir for integration test");

        let db_dir = tmp.path().join("db");
        std::fs::create_dir_all(&db_dir)
            .expect("failed to create db sub-dir in temp dir");

        let img_dir = tmp.path().join("imgs");
        std::fs::create_dir_all(&img_dir)
            .expect("failed to create imgs sub-dir in temp dir");

        let db_path = db_dir.join("idup.db3");

        Self {
            _tmp: tmp,
            db_path,
            img_dir,
        }
    }

    /// Return an `assert_cmd::Command` for the `idup` binary with
    /// `IDUP_DB_PATH` pre-set to this environment's isolated database.
    pub fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("idup").expect("failed to find idup binary");
        cmd.env("IDUP_DB_PATH", &self.db_path);
        cmd
    }

    /// Convenience: path to a file inside `img_dir`.
    pub fn img_path(&self, filename: &str) -> PathBuf {
        self.img_dir.join(filename)
    }

    /// Convenience: path to the root of the temp directory.
    pub fn tmp_path(&self) -> &Path {
        self._tmp.path()
    }
}
