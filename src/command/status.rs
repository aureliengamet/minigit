use std::collections::BTreeSet;
use std::path::Path;

use crate::command::{Command, Runtime};
use crate::minigiterror::MinigitResult;
use crate::repository::Repository;

pub struct StatusCommand;

impl Command for StatusCommand {
    fn execute(runtime: &mut Runtime) -> MinigitResult<()> {
        let mut repository = Repository::new(runtime.dir.to_path_buf());
        repository.index()?.load_for_update()?;
        let mut untracked = BTreeSet::new();
        scan_workspace(&mut repository, &mut untracked, &runtime.dir)?;
        for path in untracked {
            writeln!(&mut runtime.stdout, "?? {}", path).unwrap();
        }
        Ok(())
    }
}

fn scan_workspace(repository: &mut Repository, untracked: &mut BTreeSet<String>, root: &Path) -> MinigitResult<()> {
    for path in repository.workspace().list_dir(&root)? {
        if repository.index()?.is_path_tracked(&path) {
            if repository.workspace().is_dir(&path)? {
                scan_workspace(repository, untracked, &path)?;
            }
        } else if is_trackable_file(repository, &path)? {
            if repository.workspace().is_dir(&path)? {
                untracked.insert(format!("{}{}", path.display(), std::path::MAIN_SEPARATOR));
            } else {
                untracked.insert(format!("{}", path.display()));
            }
        }
    }
    Ok(())
}

fn is_trackable_file(repository: &mut Repository, path: &Path) -> MinigitResult<bool> {
    if repository.workspace().is_file(path)? {
        return Ok(!repository.index()?.is_path_tracked(path));
    }
    if !repository.workspace().is_dir(path)? {
        return Ok(false);
    }
    let list_dir = repository.workspace().list_dir(path)?;
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for list in list_dir {
        match repository.workspace().is_dir(&list)? {
            true => dirs.push(list),
            false => files.push(list),
        }
    }
    for file in files {
        if is_trackable_file(repository, &file)? {
            return Ok(true);
        }
    }
    for dir in dirs {
        if is_trackable_file(repository, &dir)? {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use std::fs;

    #[test]
    fn test_list_untracked_files_in_name_order() {
        crate::tests::run_test(|repo_path| {
            fs::write(format!("{}/bob.txt", repo_path), "Bob").unwrap();
            fs::write(format!("{}/alice.txt", repo_path), "Alice").unwrap();
            crate::tests::execute_and_expect_success_message(
                repo_path,
                vec!(String::new(), String::from("status")),
                "?? alice.txt\n?? bob.txt\n".to_string());
        });
    }

    #[test]
    fn test_list_untracked_directory() {
        crate::tests::run_test(|repo_path| {
            fs::write(format!("{}/alice.txt", repo_path), "Alice").unwrap();
            fs::create_dir(format!("{}/dir", repo_path)).unwrap();
            fs::write(format!("{}/dir/bob.txt", repo_path), "Bob").unwrap();
            crate::tests::execute_and_expect_success_message(
                repo_path,
                vec!(String::new(), String::from("status")),
                "?? alice.txt\n?? dir/\n".to_string());
        });
    }

    #[test]
    fn test_list_untracked_files_inside_tracked_directories() {
        crate::tests::run_test(|repo_path| {
            fs::create_dir_all(format!("{}/a/b/c", repo_path)).unwrap();
            fs::write(format!("{}/a/outer.txt", repo_path), "Outer").unwrap();
            fs::write(format!("{}/a/b/inner.txt", repo_path), "Inner").unwrap();
            fs::write(format!("{}/a/b/c/file.txt", repo_path), "File").unwrap();
            crate::tests::execute_and_expect_success(repo_path, vec!(String::new(), "add".to_string(), format!("{}/a/b/inner.txt", repo_path)));
            crate::tests::execute_and_expect_success_message(
                repo_path,
                vec!(String::new(), String::from("status")),
                "?? a/b/c/\n?? a/outer.txt\n".to_string());
        });
    }

    #[test]
    fn test_do_not_list_untracked_directory() {
        crate::tests::run_test(|repo_path| {
            fs::create_dir(format!("{}/dir", repo_path)).unwrap();
            crate::tests::execute_and_expect_success_message(
                repo_path,
                vec!(String::new(), String::from("status")),
                "".to_string());
        });
    }

    #[test]
    fn test_list_untracked_directory_that_indirectly_contains_file() {
        crate::tests::run_test(|repo_path| {
            fs::create_dir(format!("{}/outer", repo_path)).unwrap();
            fs::create_dir(format!("{}/outer/inner", repo_path)).unwrap();
            fs::write(format!("{}/outer/inner/file.txt", repo_path), "File").unwrap();
            crate::tests::execute_and_expect_success_message(
                repo_path,
                vec!(String::new(), String::from("status")),
                "?? outer/\n".to_string());
        });
    }
}