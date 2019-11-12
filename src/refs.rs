use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::lockfile::Lockfile;
use crate::minigiterror::{MinigitError, MinigitResult};

pub struct Refs {
    path: PathBuf,
}

impl Refs {
    pub fn new(path: PathBuf) -> Refs {
        Refs { path }
    }

    pub fn read_head(&self) -> MinigitResult<Option<String>> {
        let head_path = self.get_head_path();
        if !head_path.exists() {
            return Ok(None);
        }
        match fs::read_to_string(head_path) {
            Ok(head) => Ok(Some(head)),
            Err(e) => Err(MinigitError::new(String::from(format!("Error reading HEAD: {}", e)))),
        }
    }

    pub fn update_head(&self, oid: &str) -> MinigitResult<()> {
        let head_path = self.get_head_path();
        let mut head_lockfile = Lockfile::new(head_path)?;
        head_lockfile.write_str(oid)?;
        head_lockfile.commit()?;
        Ok(())
    }

    fn get_head_path(&self) -> PathBuf {
        self.path.join(Path::new("HEAD"))
    }
}