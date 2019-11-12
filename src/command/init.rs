use std::clone::Clone;
use std::fs;
use std::path::PathBuf;

use crate::command::{Command, Runtime};
use crate::minigiterror::{MinigitError, MinigitResult};

pub struct InitCommand;

impl Command for InitCommand {
    fn execute(runtime: &mut Runtime) -> MinigitResult<()> {
        let mut path =
            if runtime.args.len() > 2 {
                PathBuf::from(&runtime.args[2])
            } else {
                runtime.dir.clone()
            };
        path.push(".git");
        for dir in ["objects", "refs"].iter() {
            let mut path = path.clone();
            path.push(dir);
            if let Err(e) = fs::create_dir_all(&path) {
                return Err(MinigitError::new(format!("Couldn't create .git directory: {}", e)));
            }
        }
        Ok(())
    }
}