use std::cmp::min;
use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, Local};

use crate::{oid_to_compressed_u8_array, u16_to_u8_array_big_endian, u32_to_u8_array_big_endian};
use crate::minigiterror::MinigitResult;
use crate::workspace::MinigitMetadata;

pub trait GitObject {
    fn get_data(&self) -> &Vec<u8>;
    fn get_type(&self) -> &str;
    fn get_oid(&self) -> &str;
    fn set_oid(&mut self, oid: String);
}

pub struct Blob {
    data: Vec<u8>,
    oid: String,
}

impl Blob {
    pub fn new(data: Vec<u8>) -> Blob {
        Blob { data, oid: String::new() }
    }
}

impl GitObject for Blob {
    fn get_data(&self) -> &Vec<u8> {
        &self.data
    }

    fn get_type(&self) -> &str {
        "blob"
    }

    fn get_oid(&self) -> &str {
        &self.oid
    }

    fn set_oid(&mut self, oid: String) {
        self.oid = oid;
    }
}

trait TreeOrEntry {
    fn get_oid(&self) -> &str;
    fn get_mode(&self) -> u32;
    fn get_name(&self) -> &str;
    fn add_entry(&mut self, components: Vec<String>, entry: Entry);
    fn traverse_private(&mut self, test: &mut FnMut(&mut Tree) -> MinigitResult<()>) -> MinigitResult<()>;
}

pub struct Tree {
    entries: Vec<Box<TreeOrEntry>>,
    name: String,
    oid: String,
    data: Vec<u8>,
}

impl Tree {
    fn new(name: &str) -> Tree {
        Tree { entries: Vec::new(), name: String::from(name), oid: String::new(), data: Vec::new() }
    }

    pub fn build(entries: Vec<Entry>) -> Tree {
        let mut root = Tree::new("root");
        for entry in entries.into_iter() {
            let components: Vec<String> = entry.get_path().parent().unwrap().components().map(|component| {
                match component {
                    Component::Normal(dir_path) => String::from(dir_path.to_str().unwrap()),
                    _ => panic!("There shouldn't be another type of component in this list"),
                }
            }).collect();
            root.add_entry(components, entry);
        }
        root
    }

    pub fn traverse(&mut self, function: &mut FnMut(&mut Tree) -> MinigitResult<()>) -> MinigitResult<()> {
        self.traverse_private(function)
    }
}

impl TreeOrEntry for Tree {
    fn get_oid(&self) -> &str {
        &self.oid
    }

    fn get_mode(&self) -> u32 {
        40000
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn add_entry(&mut self, mut components: Vec<String>, entry: Entry) {
        if components.is_empty() {
            self.entries.push(Box::new(entry));
        } else {
            let dir_name = components.remove(0);
            match self.entries.last_mut() {
                Some(ref mut tree) if dir_name == tree.get_name() => {
                    tree.add_entry(components, entry);
                }
                _ => {
                    let mut tree = Tree::new(&dir_name);
                    tree.add_entry(components, entry);
                    self.entries.push(Box::new(tree));
                }
            };
        }
    }

    fn traverse_private(&mut self, function: &mut FnMut(&mut Tree) -> MinigitResult<()>) -> MinigitResult<()> {
        for entry in self.entries.iter_mut() {
            entry.traverse_private(function)?;
        }
        self.data = Vec::new();
        for entry in self.entries.iter() {
            self.data.extend_from_slice(format!("{:o}", entry.get_mode()).as_bytes());
            self.data.extend_from_slice(" ".as_bytes());
            self.data.extend_from_slice(entry.get_name().as_bytes());
            self.data.push(0);
            self.data.extend_from_slice(&oid_to_compressed_u8_array(entry.get_oid()));
        }
        function(self)
    }
}

impl GitObject for Tree {
    fn get_data(&self) -> &Vec<u8> {
        &self.data
    }

    fn get_type(&self) -> &str {
        "tree"
    }

    fn get_oid(&self) -> &str {
        &self.oid
    }

    fn set_oid(&mut self, oid: String) {
        self.oid = oid;
    }
}

pub struct Commit {
    data: Vec<u8>,
    oid: String,
}

impl Commit {
    pub fn new(parent: &Option<String>, author: Author, message: &str, tree_oid: &str) -> Commit {
        Commit {
            data: Commit::build_data(parent, &author, message, tree_oid),
            oid: String::new(),
        }
    }

    fn build_data(parent: &Option<String>, author: &Author, message: &str, tree_oid: &str) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice("tree ".as_bytes());
        data.extend_from_slice(tree_oid.as_bytes());
        if parent.is_some() {
            data.extend_from_slice("\nparent ".as_bytes());
            data.extend_from_slice(parent.as_ref().unwrap().as_bytes());
        }
        data.extend_from_slice("\nauthor ".as_bytes());
        data.extend_from_slice(author.to_string().as_bytes());
        data.extend_from_slice("\ncommitter ".as_bytes());
        data.extend_from_slice(author.to_string().as_bytes());
        data.extend_from_slice("\n\n".as_bytes());
        data.extend_from_slice(message.as_bytes());
        data
    }
}

impl GitObject for Commit {
    fn get_data(&self) -> &Vec<u8> {
        &self.data
    }

    fn get_type(&self) -> &str {
        "commit"
    }

    fn get_oid(&self) -> &str {
        &self.oid
    }

    fn set_oid(&mut self, oid: String) {
        self.oid = oid;
    }
}

pub struct Entry {
    path: PathBuf,
    path_as_str: String,
    oid: String,
    metadata: MinigitMetadata,
    flags: u16,
}

impl Entry {
    pub fn new(path: &Path, oid: &str, metadata: MinigitMetadata) -> Entry {
        let path_as_str = String::from(path.to_str().unwrap());
        let flags = min(path_as_str.len(), 0xfff) as u16;
        Entry {
            path: PathBuf::from(path),
            path_as_str,
            oid: String::from(oid),
            metadata,
            flags,
        }
    }

    pub fn get_path(&self) -> &Path {
        &self.path
    }

    pub fn get_path_as_str(&self) -> &str {
        &self.path_as_str
    }

    pub fn get_data(&self) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(&u32_to_u8_array_big_endian(self.metadata.ctime));
        data.extend_from_slice(&u32_to_u8_array_big_endian(self.metadata.ctime_nsec));
        data.extend_from_slice(&u32_to_u8_array_big_endian(self.metadata.mtime));
        data.extend_from_slice(&u32_to_u8_array_big_endian(self.metadata.mtime_nsec));
        data.extend_from_slice(&u32_to_u8_array_big_endian(self.metadata.dev));
        data.extend_from_slice(&u32_to_u8_array_big_endian(self.metadata.ino));
        data.extend_from_slice(&u32_to_u8_array_big_endian(self.metadata.mode));
        data.extend_from_slice(&u32_to_u8_array_big_endian(self.metadata.uid));
        data.extend_from_slice(&u32_to_u8_array_big_endian(self.metadata.gid));
        data.extend_from_slice(&u32_to_u8_array_big_endian(self.metadata.size));
        data.extend_from_slice(&oid_to_compressed_u8_array(&self.oid));
        data.extend_from_slice(&u16_to_u8_array_big_endian(self.flags));
        data.extend_from_slice(self.path_as_str.as_bytes());
        data.push(0);
        while data.len() % 8 != 0 {
            data.push(0);
        }
        data
    }

    pub fn get_mode(&self) -> u32 {
        self.metadata.mode
    }
}

impl TreeOrEntry for Entry {
    fn get_oid(&self) -> &str {
        &self.oid
    }

    fn get_mode(&self) -> u32 {
        self.metadata.mode
    }

    fn get_name(&self) -> &str {
        &self.path.file_name().unwrap().to_str().unwrap()
    }

    fn add_entry(&mut self, _components: Vec<String>, _entry: Entry) {
        panic!("The method add_entry is not implemented for Entry.");
    }

    fn traverse_private(&mut self, _function: &mut FnMut(&mut Tree) -> MinigitResult<()>) -> MinigitResult<()> {
        Ok(())
    }
}

pub struct Author {
    name: String,
    email: String,
    timestamp: DateTime<Local>,
}

impl Author {
    pub fn new(name: &str, email: &str, timestamp: DateTime<Local>) -> Author {
        Author { name: String::from(name), email: String::from(email), timestamp }
    }

    pub fn to_string(&self) -> String {
        let seconds = &self.timestamp.timestamp();
        let offset = &self.timestamp.offset().local_minus_utc();
        format!("{} <{}> {} {:+03}{:02}",
                &self.name,
                &self.email,
                seconds,
                offset / 3600,
                offset / 60 % 60)
    }
}