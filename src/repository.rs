use std::path::PathBuf;

use crate::database::Database;
use crate::index::Index;
use crate::minigiterror::MinigitResult;
use crate::refs::Refs;
use crate::workspace::Workspace;

pub struct Repository {
    path: PathBuf,
    database: Option<Database>,
    index: Option<Index>,
    refs: Option<Refs>,
    workspace: Option<Workspace>,
}

impl Repository {
    pub fn new(path: PathBuf) -> Repository {
        let path = match path.ends_with(".git") {
            true => path,
            false => path.join(".git"),
        };
        Repository {
            path,
            database: None,
            index: None,
            refs: None,
            workspace: None,
        }
    }

    pub fn database(&mut self) -> &mut Database {
        if self.database.is_none() {
            self.database = Some(Database::new(self.path.join("objects")));
        }
        self.database.as_mut().unwrap()
    }

    pub fn index(&mut self) -> MinigitResult<&mut Index> {
        if self.index.is_none() {
            self.index = Some(Index::new(self.path.join("index"))?);
        }
        Ok(self.index.as_mut().unwrap())
    }

    pub fn index_take(&mut self) -> MinigitResult<Index> {
        self.index()?;
        Ok(self.index.take().unwrap())
    }

    pub fn refs(&mut self) -> &mut Refs {
        if self.refs.is_none() {
            self.refs = Some(Refs::new(self.path.clone()));
        }
        self.refs.as_mut().unwrap()
    }

    pub fn workspace(&mut self) -> &mut Workspace {
        if self.workspace.is_none() {
            self.workspace = Some(Workspace::new(self.path.parent().unwrap()));
        }
        self.workspace.as_mut().unwrap()
    }
}