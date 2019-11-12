use std::error::Error;
use std::ffi::OsString;
use std::fs;
#[cfg(not(unix))]
use std::fs::Metadata;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::path::PathBuf;

use crate::minigiterror::{MinigitError, MinigitResult};

pub struct Workspace {
    path: PathBuf,
}

pub struct MinigitMetadata {
    pub ctime: u32,
    pub ctime_nsec: u32,
    pub mtime: u32,
    pub mtime_nsec: u32,
    pub dev: u32,
    pub ino: u32,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u32,
}

impl Workspace {
    pub fn new(path: &Path) -> Workspace {
        Workspace { path: path.canonicalize().unwrap() }
    }

    fn normalize_path(&self, path: &Path) -> MinigitResult<PathBuf> {
        let path = match path.is_absolute() {
            true => PathBuf::from(path),
            false => self.path.join(path),
        };
        if !path.exists() {
            return Err(MinigitError::new(format!("fatal: pathspec '{}' did not match any files", path.display())));
        }
        match path.canonicalize() {
            Ok(path) => Ok(path),
            Err(e) => Err(MinigitError::new(format!("Couldn't canonicalize path {}, error: {}", path.display(), e))),
        }
    }

    pub fn is_file(&self, path: &Path) -> MinigitResult<bool> {
        Ok(self.normalize_path(path)?.is_file())
    }

    pub fn is_dir(&self, path: &Path) -> MinigitResult<bool> {
        Ok(self.normalize_path(path)?.is_dir())
    }

    pub fn list_dir(&self, path: &Path) -> MinigitResult<Vec<PathBuf>> {
        let path = self.normalize_path(path)?;
        match self.list_dir_recurse(&path, Vec::new()) {
            Ok(files) => Ok(files),
            Err(e) => Err(MinigitError::new(format!("Error trying to list files from path {}: {}", path.display(), e))),
        }
    }

    fn list_dir_recurse(&self, path: &Path, mut result: Vec<PathBuf>) -> Result<Vec<PathBuf>, Box<Error>> {
        for entry in fs::read_dir(&path)? {
            let entry = entry?;
            let path = entry.path();
            if !self.is_dir_ignored(&path) && !self.is_file_ignored(&path) {
                result.push(PathBuf::from(path.strip_prefix(&self.path)?));
            }
        }
        Ok(result)
    }

    pub fn list_files_from_path(&self, path: &Path) -> MinigitResult<Vec<PathBuf>> {
        let path = self.normalize_path(path)?;
        match self.list_files_recurse(&path, Vec::new()) {
            Ok(files) => Ok(files),
            Err(e) => Err(MinigitError::new(format!("Error trying to list files from path {}: {}", path.display(), e))),
        }
    }

    fn list_files_recurse(&self, path: &Path, mut result: Vec<PathBuf>) -> Result<Vec<PathBuf>, Box<Error>> {
        if path.is_file() && !self.is_file_ignored(&path) {
            result.push(PathBuf::from(path.strip_prefix(&self.path)?));
        } else if path.is_dir() && !self.is_dir_ignored(&path) {
            for entry in fs::read_dir(&path)? {
                let entry = entry?;
                let path = entry.path();
                result = self.list_files_recurse(&path, result)?;
            }
        }
        Ok(result)
    }

    fn is_dir_ignored(&self, path: &Path) -> bool {
        let ignored_dirs = [OsString::from(".git"), OsString::from("target")];
        if let Some(filename) = path.file_name() {
            if ignored_dirs.contains(&filename.to_os_string()) {
                return true;
            }
        }
        false
    }

    fn is_file_ignored(&self, path: &Path) -> bool {
        let ignored_files = [OsString::from(".DS_Store")];
        let ignored_extensions = [OsString::from("iml")];
        if let Some(filename) = path.file_name() {
            if ignored_files.contains(&filename.to_os_string()) {
                return true;
            }
        }
        if let Some(extension) = path.extension() {
            if ignored_extensions.contains(&extension.to_os_string()) {
                return true;
            }
        }
        false
    }

    pub fn read_file(&self, path: &Path) -> MinigitResult<Vec<u8>> {
        match fs::read(self.path.join(path)) {
            Ok(bytes) => Ok(bytes),
            Err(e) => Err(MinigitError::new(String::from(format!("error: trying to read file '{}': {}", path.display(), e)))),
        }
    }

    pub fn get_metadata(&self, path: &Path) -> MinigitResult<MinigitMetadata> {
        match fs::metadata(self.path.join(path)) {
            Ok(metadata) => Ok(self._get_metadata(&metadata)),
            Err(e) => Err(MinigitError::new(format!("Couldn't read metadata of path {}: {}", path.display(), e))),
        }
    }

    #[cfg(unix)]
    fn _get_metadata(&self, metadata: &fs::Metadata) -> MinigitMetadata {
        let mode = match metadata.mode() & 0o100 > 0 {
            true => 0o100755,
            false => 0o100644
        };
        MinigitMetadata {
            ctime: metadata.ctime() as u32,
            ctime_nsec: metadata.ctime_nsec() as u32,
            mtime: metadata.mtime() as u32,
            mtime_nsec: metadata.mtime_nsec() as u32,
            dev: metadata.dev() as u32,
            ino: metadata.ino() as u32,
            mode,
            uid: metadata.uid(),
            gid: metadata.gid(),
            size: metadata.size() as u32,
        }
    }

    #[cfg(not(unix))]
    fn _get_metadata(&self, metadata: &fs::Metadata) -> MinigitMetadata {
        panic!("Not implemented for non Unix platforms at the moment.");
    }
}