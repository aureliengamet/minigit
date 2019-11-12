use std::fs::{File, OpenOptions, rename};
use std::io::Write;
use std::path::PathBuf;

use crate::minigiterror::{MinigitError, MinigitResult};

pub struct Lockfile {
    target_file_path: PathBuf,
    lock_file: File,
    lock_file_path: PathBuf,
    commit_has_been_called: bool,
}

impl Lockfile {
    pub fn new(path: PathBuf) -> MinigitResult<Lockfile> {
        let target_file_path = path;
        let lock_file_path = target_file_path.with_extension("lock");
        match OpenOptions::new().write(true).create_new(true).open(&lock_file_path) {
            Ok(lock_file) => Ok(Lockfile { target_file_path, lock_file, lock_file_path, commit_has_been_called: false }),
            Err(e) => Err(MinigitError::new(String::from(format!("Unable to create '{}': {}", lock_file_path.display(), e)))),
        }
    }

    pub fn write(&mut self, value: &[u8]) -> MinigitResult<()> {
        match &self.lock_file.write_all(value) {
            Ok(_) => Ok(()),
            Err(e) => Err(MinigitError::new(String::from(format!("Error writing to {}: {}", &self.lock_file_path.display(), e)))),
        }
    }

    pub fn write_str(&mut self, value: &str) -> MinigitResult<()> {
        self.write(value.as_bytes())
    }

    pub fn commit(mut self) -> MinigitResult<()> {
        self.commit_has_been_called = true;
        match rename(&self.lock_file_path, &self.target_file_path) {
            Ok(_) => Ok(()),
            Err(e) => Err(MinigitError::new(String::from(format!("Error renaming {} to {}: {}", &self.lock_file_path.display(), &self.target_file_path.display(), e)))),
        }
    }
}

impl Drop for Lockfile {
    fn drop(&mut self) {
        if !self.commit_has_been_called {
            if let Err(e) = std::fs::remove_file(&self.lock_file_path) {
                eprintln!("Error trying to delete {}: {}", self.lock_file_path.display(), e);
            }
        }
    }
}