use std::fs;
use std::io;
use std::io::prelude::*;
use std::path::PathBuf;

use crypto::digest::Digest;
use crypto::sha1::Sha1;
use flate2::Compression;
use flate2::write::ZlibEncoder;

use crate::gitobject::GitObject;
use crate::minigiterror::{MinigitError, MinigitResult};

pub struct Database {
    path: PathBuf
}

impl Database {
    pub fn new(path: PathBuf) -> Database {
        Database { path }
    }

    pub fn store<T: GitObject>(&self, gitobject: &mut T) -> MinigitResult<()> {
        let mut bytes_buffer: Vec<u8> = Vec::new();
        bytes_buffer.extend_from_slice(gitobject.get_type().as_bytes());
        bytes_buffer.extend_from_slice(b" ");
        bytes_buffer.extend_from_slice(&gitobject.get_data().len().to_string().as_bytes());
        bytes_buffer.push(0);
        bytes_buffer.extend_from_slice(gitobject.get_data().as_slice());

        let mut hasher = Sha1::new();
        hasher.input(&bytes_buffer);
        gitobject.set_oid(hasher.result_str());

        match self.write_object(gitobject.get_oid(), bytes_buffer) {
            Ok(_) => Ok(()),
            Err(e) => Err(MinigitError::new(format!("Couldn't write bytes to disk: {}", e))),
        }
    }

    fn write_object(&self, oid: &str, content: Vec<u8>) -> Result<(), io::Error> {
        let mut root_path = PathBuf::from(&self.path);
        root_path.push(&oid[0..2]);
        if !root_path.exists() {
            fs::create_dir(&root_path)?;
        }

        let mut object_path = root_path.clone();
        object_path.push(&oid[2..]);
        if object_path.exists() {
            return Ok(());
        }

        let now = std::time::SystemTime::now();
        let nanos_since_epoch = now.duration_since(std::time::SystemTime::UNIX_EPOCH).unwrap().as_nanos();
        let tmp_filename = format!("tmp_{}", nanos_since_epoch);
        let mut tmp_path = root_path;
        tmp_path.push(tmp_filename);

        let mut zlib_encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        zlib_encoder.write_all(&content)?;
        let compressed_content = zlib_encoder.finish()?;

        fs::write(&tmp_path, &compressed_content)?;
        fs::rename(tmp_path, object_path)?;
        Ok(())
    }
}
