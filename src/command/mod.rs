use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;

use crate::command::add::AddCommand;
use crate::command::commit::CommitCommand;
use crate::command::init::InitCommand;
use crate::command::status::StatusCommand;
use crate::minigiterror::{MinigitError, MinigitResult};

mod add;
mod commit;
mod init;
mod status;

pub trait Command {
    fn execute(runtime: &mut Runtime) -> MinigitResult<()>;
}

pub fn execute(runtime: &mut Runtime) -> MinigitResult<()> {
    if runtime.args.len() == 1 {
        return Err(MinigitError::new(String::from("No command has been passed")));
    }
    match runtime.args.get(1).unwrap().as_str() {
        "add" => AddCommand::execute(runtime),
        "commit" => CommitCommand::execute(runtime),
        "init" => InitCommand::execute(runtime),
        "status" => StatusCommand::execute(runtime),
        unknown_command => Err(MinigitError::new(format!("Unknown git command {}", unknown_command))),
    }
}

pub struct Runtime<'a> {
    pub dir: PathBuf,
    pub env: HashMap<String, String>,
    pub args: Vec<String>,
    pub stdin: Box<dyn Read + 'a>,
    pub stdout: Box<dyn Write + 'a>,
    pub stderr: Box<dyn Write + 'a>,
}

impl<'a> Default for Runtime<'a> {
    fn default() -> Self {
        Runtime {
            dir: PathBuf::from("."),
            env: std::env::vars().collect(),
            args: std::env::args().collect(),
            stdin: Box::new(std::io::stdin()),
            stdout: Box::new(std::io::stdout()),
            stderr: Box::new(std::io::stderr()),
        }
    }
}

impl<'a> Runtime<'a> {
    pub fn get_env_var(&self, key: &str) -> MinigitResult<&String> {
        match self.env.get(key) {
            Some(value) => Ok(value),
            None => Err(MinigitError::new(format!("This environment variable is not set: {}", key))),
        }
    }

    pub fn read_from_stdin(&mut self) -> MinigitResult<String> {
        let mut input = String::new();
        if let Err(e) = self.stdin.read_to_string(&mut input) {
            return Err(MinigitError::new(format!("Error trying to read from stdin: {}", e)));
        }
        Ok(input)
    }
}