use chrono::Local;

use crate::command::{Command, Runtime};
use crate::gitobject::{Author, Commit, GitObject, Tree};
use crate::minigiterror::MinigitResult;
use crate::repository::Repository;

pub struct CommitCommand {}

impl Command for CommitCommand {
    fn execute(runtime: &mut Runtime) -> MinigitResult<()> {
        let mut repository = Repository::new(runtime.dir.join(".git"));

        let entries = repository.index_take()?.load_and_get_entries()?;

        let mut tree = Tree::build(entries);
        tree.traverse(&mut |tree| repository.database().store(tree))?;

        let parent = repository.refs().read_head()?;
        let author_name = runtime.get_env_var("GIT_AUTHOR_NAME")?;
        let author_email = runtime.get_env_var("GIT_AUTHOR_EMAIL")?;
        let author = Author::new(author_name, author_email, Local::now());
        let commit_message = runtime.read_from_stdin()?;
        let mut commit = Commit::new(&parent, author, &commit_message, tree.get_oid());
        repository.database().store(&mut commit)?;
        repository.refs().update_head(commit.get_oid())?;

        let root_message = match parent {
            Some(_) => "",
            None => "(root-commit) ",
        };
        writeln!(&mut runtime.stdout, "[{}{}] {}", root_message, commit.get_oid(), commit_message.lines().next().unwrap()).unwrap();
        Ok(())
    }
}