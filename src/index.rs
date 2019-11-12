use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crypto::digest::Digest;
use crypto::sha1::Sha1;

use crate::{oid_to_compressed_u8_array, u32_to_u8_array_big_endian, u8_array_to_u16_big_endian, u8_array_to_u32_big_endian, uncompress_u8_array_to_oid};
use crate::gitobject::Entry;
use crate::lockfile::Lockfile;
use crate::minigiterror::{MinigitError, MinigitResult};
use crate::workspace::MinigitMetadata;

pub struct Index {
    entries: BTreeMap<String, Entry>,
    parents: HashMap<String, HashSet<String>>,
    path: PathBuf,
    lockfile: Option<Lockfile>,
    changed: bool,
}

const FATAL_INDEX_TOO_SHORT_MESSAGE: &str = "fatal: index was shorter than expected";
const FATAL_INDEX_CORRUPTED_MESSAGE: &str = "fatal: index file corrupt";

impl Index {
    pub fn new(path: PathBuf) -> MinigitResult<Index> {
        let lockfile = match Lockfile::new(path.clone()) {
            Ok(lockfile) => lockfile,
            Err(mut error) => {
                error.message = format!("fatal: {}\n\n\
                Another git process seems to be running in this repository.\n\
                Please make sure all processes are terminated then try again.\n\
                If it still fails, a git process may have crashed in this repository earlier: remove the file manually to continue.",
                                        error.message);
                return Err(error);
            }
        };
        Ok(Index {
            entries: BTreeMap::new(),
            parents: HashMap::new(),
            path,
            lockfile: Some(lockfile),
            changed: false,
        })
    }

    pub fn load_and_get_entries(mut self) -> MinigitResult<Vec<Entry>> {
        self.load_for_update()?;
        Ok(self.entries.into_iter().map(|(_key, value)| value).collect())
    }

    pub fn load_for_update(&mut self) -> MinigitResult<()> {
        if self.lockfile.is_none() {
            return Err(MinigitError::new(format!("Lock file for path {} disappeared during the process, cannot continue", self.path.display())));
        }
        if !self.path.exists() {
            return Ok(());
        }

        self.clear();
        let data = match fs::read(&self.path) {
            Ok(data) => data,
            Err(e) => return Err(MinigitError::new(format!("Error reading file {}: {}", self.path.display(), e))),
        };
        let mut offset = 0;

        let count = self.read_header(&data, &mut offset)?;

        for _ in 0..count {
            let new_entry = self.read_entry(&data, &mut offset)?;
            self.insert_entry(new_entry);
        }

        self.verify_hash(offset, &data)
    }

    pub fn is_path_tracked(&self, path: &Path) -> bool {
        let path = format!("{}", path.display());
        self.entries.contains_key(&path) || self.parents.contains_key(&path)
    }

    fn clear(&mut self) {
        self.entries = BTreeMap::new();
        self.changed = false;
    }

    fn read_header(&self, data: &Vec<u8>, offset: &mut usize) -> MinigitResult<u32> {
        let signature = self.get_slice(&data, offset, 4)?;
        if signature != "DIRC".as_bytes() {
            match std::str::from_utf8(signature) {
                Ok(signature) => return Err(MinigitError::new(format!("Index signature: expected 'DIRC', got {}", signature))),
                Err(_) => return Err(MinigitError::new(format!("Index signature: expected 'DIRC', got incorrect utf8 bytes {:?}", signature))),
            }
        }
        let version = u8_array_to_u32_big_endian(self.get_slice(&data, offset, 4)?);
        if version != 2 {
            return Err(MinigitError::new(format!("Index version: expected 2, got {}", version)));
        }
        let count = u8_array_to_u32_big_endian(self.get_slice(&data, offset, 4)?);
        Ok(count)
    }

    fn get_slice<'a>(&self, data: &'a Vec<u8>, offset: &mut usize, size: usize) -> MinigitResult<&'a [u8]> {
        if data.len() < *offset + size {
            return Err(MinigitError::new(format!("{}", FATAL_INDEX_TOO_SHORT_MESSAGE)));
        }
        let old_offset = *offset;
        *offset = *offset + size;
        Ok(&data[old_offset..*offset])
    }

    fn read_entry(&self, data: &Vec<u8>, offset: &mut usize) -> MinigitResult<Entry> {
        let metadata = MinigitMetadata {
            ctime: u8_array_to_u32_big_endian(self.get_slice(&data, offset, 4)?),
            ctime_nsec: u8_array_to_u32_big_endian(self.get_slice(&data, offset, 4)?),
            mtime: u8_array_to_u32_big_endian(self.get_slice(&data, offset, 4)?),
            mtime_nsec: u8_array_to_u32_big_endian(self.get_slice(&data, offset, 4)?),
            dev: u8_array_to_u32_big_endian(self.get_slice(&data, offset, 4)?),
            ino: u8_array_to_u32_big_endian(self.get_slice(&data, offset, 4)?),
            mode: u8_array_to_u32_big_endian(self.get_slice(&data, offset, 4)?),
            uid: u8_array_to_u32_big_endian(self.get_slice(&data, offset, 4)?),
            gid: u8_array_to_u32_big_endian(self.get_slice(&data, offset, 4)?),
            size: u8_array_to_u32_big_endian(self.get_slice(&data, offset, 4)?),
        };
        let oid = uncompress_u8_array_to_oid(self.get_slice(&data, offset, 20)?);
        // Unused atm
        let _flags = u8_array_to_u16_big_endian(self.get_slice(&data, offset, 2)?);
        let (path_size, padding_size) = self.get_entry_size(&data, *offset, 2, 8)?;
        let path_bytes = self.get_slice(&data, offset, path_size)?;
        *offset += padding_size;
        let path_as_str = match std::str::from_utf8(path_bytes) {
            Ok(path) => path,
            Err(_) => return Err(MinigitError::new(format!("{}", FATAL_INDEX_CORRUPTED_MESSAGE))),
        };
        Ok(Entry::new(Path::new(path_as_str), &oid, metadata))
    }

    fn get_entry_size(&self, data: &Vec<u8>, offset: usize, entry_min_size: usize, entry_block_size: usize) -> MinigitResult<(usize, usize)> {
        let mut entry_size = entry_min_size;
        if data.len() < offset + entry_size {
            return Err(MinigitError::new(format!("{}", FATAL_INDEX_TOO_SHORT_MESSAGE)));
        }
        while data[offset + entry_size - 1] != 0 {
            entry_size += entry_block_size;
            if data.len() < offset + entry_size {
                return Err(MinigitError::new(format!("{}", FATAL_INDEX_TOO_SHORT_MESSAGE)));
            }
        }
        let mut path_size = entry_size;
        while data[offset + path_size - 1] == 0 {
            path_size -= 1;
            if path_size == 0 {
                return Err(MinigitError::new(format!("{}", FATAL_INDEX_CORRUPTED_MESSAGE)));
            }
        }
        let padding_size = entry_size - path_size;
        Ok((path_size, padding_size))
    }

    fn verify_hash(&self, offset: usize, data: &Vec<u8>) -> MinigitResult<()> {
        if data.len() < offset + 20 {
            return Err(MinigitError::new(format!("{}", FATAL_INDEX_TOO_SHORT_MESSAGE)));
        }
        let mut hasher = Sha1::new();
        hasher.input(&data[..data.len() - 20]);
        let expected_hash = oid_to_compressed_u8_array(&hasher.result_str());
        let actual_hash = &data[data.len() - 20..];
        match expected_hash == actual_hash {
            true => Ok(()),
            false => Err(MinigitError::new(format!("{}", FATAL_INDEX_CORRUPTED_MESSAGE)))
        }
    }

    pub fn add(&mut self, path: &Path, oid: &str, metadata: MinigitMetadata) {
        let entry = Entry::new(path, oid, metadata);
        self.discard_conflicts(&entry);
        self.insert_entry(entry);
        self.changed = true;
    }

    fn insert_entry(&mut self, entry: Entry) {
        let path_as_str = String::from(entry.get_path_as_str());
        let mut ancestors = entry.get_path().ancestors();
        ancestors.next();
        for ancestor in ancestors {
            let ancestor_as_str = ancestor.to_str().unwrap();
            if !self.parents.contains_key(ancestor_as_str) {
                self.parents.insert(String::from(ancestor_as_str), HashSet::new());
            }
            self.parents.get_mut(ancestor_as_str).unwrap().insert(path_as_str.clone());
        }
        self.entries.insert(path_as_str, entry);
    }

    fn discard_conflicts(&mut self, entry: &Entry) {
        for ancestor in entry.get_path().ancestors() {
            let ancestor_as_str = ancestor.to_str().unwrap();
            if ancestor_as_str == "" {
                break;
            }
            self.entries.remove(ancestor_as_str);
        }
        if let Some(children_paths) = self.parents.get_mut(entry.get_path_as_str()) {
            for children_path in children_paths.iter() {
                self.entries.remove(children_path);
            }
            children_paths.clear();
        }
    }

    pub fn write_updates(&mut self) -> MinigitResult<bool> {
        if self.lockfile.is_none() || !self.changed {
            return Ok(false);
        }
        let mut lockfile = self.lockfile.take().unwrap();
        let mut hasher = Sha1::new();

        self.write_str(&mut lockfile, &mut hasher, "DIRC")?;
        self.write(&mut lockfile, &mut hasher, &u32_to_u8_array_big_endian(2))?;
        self.write(&mut lockfile, &mut hasher, &u32_to_u8_array_big_endian(self.entries.len() as u32))?;

        for (_, entry) in self.entries.iter() {
            self.write(&mut lockfile, &mut hasher, &entry.get_data())?;
        }

        let index_oid = hasher.result_str();
        lockfile.write(&oid_to_compressed_u8_array(&index_oid))?;
        lockfile.commit()?;
        self.changed = true;

        Ok(true)
    }

    fn write(&self, lockfile: &mut Lockfile, hasher: &mut Sha1, data: &[u8]) -> MinigitResult<()> {
        hasher.input(data);
        lockfile.write(data)?;
        Ok(())
    }

    fn write_str(&self, lockfile: &mut Lockfile, hasher: &mut Sha1, data: &str) -> MinigitResult<()> {
        self.write(lockfile, hasher, data.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use std::iter;

    use rand::distributions::Alphanumeric;
    use rand::prelude::*;

    use crate::workspace::Workspace;

    use super::*;

    fn prepare_test_context(paths: &[&str]) -> (Index) {
        let mut rng = rand::thread_rng();
        let index_name: String = iter::repeat(())
            .map(|_| rng.sample(Alphanumeric))
            .take(20)
            .collect();
        let mut index = Index::new(PathBuf::from(format!("/tmp/{}", index_name))).unwrap();
        let workspace = Workspace::new(Path::new("."));
        for path in paths {
            let metadata = workspace.get_metadata(Path::new("Cargo.lock")).unwrap();
            let oid: String = iter::repeat(())
                .map(|_| rng.sample(Alphanumeric))
                .take(20)
                .collect();
            index.add(Path::new(path), &oid, metadata);
        }
        index
    }

    #[test]
    fn test_add_one_entry() {
        let index = prepare_test_context(&["alice.txt"]);
        let actual_paths: Vec<String> = index.entries.into_iter().map(|(_, value)| String::from(value.get_path_as_str())).collect();
        assert_eq!(vec!("alice.txt"), actual_paths);
    }

    #[test]
    fn test_add_multiple_values() {
        let index = prepare_test_context(&["bob.txt", "alice.txt"]);
        let actual_paths: Vec<String> = index.entries.into_iter().map(|(_, value)| String::from(value.get_path_as_str())).collect();
        assert_eq!(vec!("alice.txt", "bob.txt"), actual_paths);
    }

    #[test]
    fn test_add_multiple_inside_a_directory() {
        let index = prepare_test_context(&["nested/bob.txt", "nested/alice.txt"]);
        let actual_paths: Vec<String> = index.entries.into_iter().map(|(_, value)| String::from(value.get_path_as_str())).collect();
        assert_eq!(vec!("nested/alice.txt", "nested/bob.txt"), actual_paths);
    }

    #[test]
    fn test_add_replace_file_by_directory() {
        let index = prepare_test_context(&["alice.txt", "bob.txt", "alice.txt/nested.txt"]);
        let actual_paths: Vec<String> = index.entries.into_iter().map(|(_, value)| String::from(value.get_path_as_str())).collect();
        assert_eq!(vec!("alice.txt/nested.txt", "bob.txt"), actual_paths);
    }

    #[test]
    fn test_add_replace_directory_by_file() {
        let index = prepare_test_context(&["alice.txt", "nested/bob.txt", "nested"]);
        let actual_paths: Vec<String> = index.entries.into_iter().map(|(_, value)| String::from(value.get_path_as_str())).collect();
        assert_eq!(vec!("alice.txt", "nested"), actual_paths);
    }

    #[test]
    fn test_add_replace_complex_directory_by_file() {
        let index = prepare_test_context(&["alice.txt", "nested/bob.txt", "nested/inner/claire.txt", "nested"]);
        let actual_paths: Vec<String> = index.entries.into_iter().map(|(_, value)| String::from(value.get_path_as_str())).collect();
        assert_eq!(vec!("alice.txt", "nested"), actual_paths);
    }
}