use std::path::{Path, PathBuf};

use crate::command::{Command, Runtime};
use crate::gitobject::{Blob, GitObject};
use crate::minigiterror::{MinigitError, MinigitResult};
use crate::repository::Repository;

pub struct AddCommand {}

impl Command for AddCommand {
    fn execute(runtime: &mut Runtime) -> Result<(), MinigitError> {
        if runtime.args.len() <= 2 {
            return Err(MinigitError::new(String::from("Nothing specified, nothing added.\nMaybe you wanted to say 'minigit add .'?")));
        }

        let mut repository = Repository::new(runtime.dir.join(".git"));
        repository.index()?.load_for_update()?;

        let mut added_file_paths: Vec<PathBuf> = Vec::new();
        for added_paths in runtime.args[2..].iter()
            .map(Path::new)
            .map(|added_path| repository.workspace().list_files_from_path(added_path)) {
            added_file_paths.extend(added_paths?);
        }

        if let Err(mut error) = store_in_database_and_update_index(added_file_paths, &mut repository) {
            error.message = format!("{}\nfatal: adding files failed", error.message);
            return Err(error);
        }

        repository.index()?.write_updates()?;
        Ok(())
    }
}

fn store_in_database_and_update_index(added_file_paths: Vec<PathBuf>, repository: &mut Repository) -> MinigitResult<()> {
    for added_file_path in added_file_paths {
        let data = repository.workspace().read_file(&added_file_path)?;
        let mut blob = Blob::new(data);
        repository.database().store(&mut blob)?;
        let metadata = repository.workspace().get_metadata(&added_file_path)?;
        repository.index()?.add(&added_file_path, blob.get_oid(), metadata);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn test_add_one_file() {
        crate::tests::run_test(|repo_path| {
            let file_path = format!("{}/hello.txt", repo_path);
            fs::write(&file_path, "Hello World").unwrap();
            crate::tests::execute_and_expect_success(repo_path, vec!(String::new(), String::from("add"), String::from("hello.txt")));
            crate::tests::assert_index(repo_path, vec!((0o100644, String::from("hello.txt"))));
        });
    }

    #[test]
    #[cfg(unix)]
    fn test_add_one_executable_file() {
        crate::tests::run_test(|repo_path| {
            let file_path = format!("{}/hello.txt", repo_path);
            fs::write(&file_path, "Hello World").unwrap();
            let mut perms = fs::metadata(&file_path).unwrap().permissions();
            perms.set_mode(0o770);
            fs::set_permissions(&file_path, perms).unwrap();
            crate::tests::execute_and_expect_success(repo_path, vec!(String::new(), String::from("add"), String::from("hello.txt")));
            crate::tests::assert_index(repo_path, vec!((0o100755, String::from("hello.txt"))));
        });
    }

    #[test]
    fn test_add_multiple_files() {
        crate::tests::run_test(|repo_path| {
            fs::write(format!("{}/alice.txt", repo_path), "Alice").unwrap();
            fs::write(format!("{}/bob.txt", repo_path), "Bob").unwrap();
            crate::tests::execute_and_expect_success(repo_path, vec!(String::new(), String::from("add"), String::from("bob.txt"), String::from("alice.txt")));
            crate::tests::assert_index(repo_path, vec!((0o100644, String::from("alice.txt")), (0o100644, String::from("bob.txt"))));
        });
    }

    #[test]
    fn test_add_multiple_inside_a_directory() {
        crate::tests::run_test(|repo_path| {
            fs::create_dir(format!("{}/nested", repo_path)).unwrap();
            fs::write(format!("{}/nested/alice.txt", repo_path), "Alice").unwrap();
            fs::write(format!("{}/nested/bob.txt", repo_path), "Bob").unwrap();
            crate::tests::execute_and_expect_success(repo_path, vec!(String::new(), String::from("add"), String::from("nested/alice.txt"), String::from("nested/bob.txt")));
            crate::tests::assert_index(repo_path, vec!((0o100644, String::from("nested/alice.txt")), (0o100644, String::from("nested/bob.txt"))));
        });
    }

    #[test]
    fn test_add_directory() {
        crate::tests::run_test(|repo_path| {
            fs::create_dir_all(format!("{}/nested", repo_path)).unwrap();
            fs::write(format!("{}/nested/alice.txt", repo_path), "Alice").unwrap();
            fs::write(format!("{}/nested/bob.txt", repo_path), "Bob").unwrap();
            crate::tests::execute_and_expect_success(repo_path, vec!(String::new(), String::from("add"), String::from("nested")));
            crate::tests::assert_index(repo_path, vec!((0o100644, String::from("nested/alice.txt")), (0o100644, String::from("nested/bob.txt"))));
        });
    }

    #[test]
    fn test_add_replace_file_by_directory() {
        crate::tests::run_test(|repo_path| {
            fs::write(format!("{}/alice.txt", repo_path), "Alice").unwrap();
            crate::tests::execute_and_expect_success(repo_path, vec!(String::new(), String::from("add"), String::from("alice.txt")));
            fs::remove_file(format!("{}/alice.txt", repo_path)).unwrap();
            fs::create_dir(format!("{}/alice.txt", repo_path)).unwrap();
            fs::write(format!("{}/alice.txt/bob.txt", repo_path), "Bob").unwrap();
            crate::tests::execute_and_expect_success(repo_path, vec!(String::new(), String::from("add"), String::from("alice.txt/bob.txt")));
            crate::tests::assert_index(repo_path, vec!((0o100644, String::from("alice.txt/bob.txt"))));
        });
    }

    #[test]
    fn test_add_replace_directory_by_file() {
        crate::tests::run_test(|repo_path| {
            fs::create_dir(format!("{}/nested", repo_path)).unwrap();
            fs::write(format!("{}/nested/alice.txt", repo_path), "Alice").unwrap();
            crate::tests::execute_and_expect_success(repo_path, vec!(String::new(), String::from("add"), String::from("nested/alice.txt")));
            fs::remove_dir_all(format!("{}/nested", repo_path)).unwrap();
            fs::write(format!("{}/nested", repo_path), "Nested").unwrap();
            crate::tests::execute_and_expect_success(repo_path, vec!(String::new(), String::from("add"), String::from("nested")));
            crate::tests::assert_index(repo_path, vec!((0o100644, String::from("nested"))));
        });
    }

    #[test]
    fn test_add_replace_complex_directory_by_file() {
        crate::tests::run_test(|repo_path| {
            fs::create_dir_all(format!("{}/nested/inner", repo_path)).unwrap();
            fs::write(format!("{}/nested/alice.txt", repo_path), "Alice").unwrap();
            fs::write(format!("{}/nested/inner/bob.txt", repo_path), "Bob").unwrap();
            crate::tests::execute_and_expect_success(repo_path, vec!(String::new(), String::from("add"), String::from("nested/alice.txt"), String::from("nested/inner/bob.txt")));
            fs::remove_dir_all(format!("{}/nested", repo_path)).unwrap();
            fs::write(format!("{}/nested", repo_path), "Nested").unwrap();
            crate::tests::execute_and_expect_success(repo_path, vec!(String::new(), String::from("add"), String::from("nested")));
            crate::tests::assert_index(repo_path, vec!((0o100644, String::from("nested"))));
        });
    }

    #[test]
    fn test_add_non_existent_file() {
        crate::tests::run_test(|repo_path| {
            crate::tests::execute_and_expect_error_message(
                repo_path,
                vec!(String::new(), String::from("add"), String::from("bad_path.txt")),
                format!("fatal: pathspec '{}/bad_path.txt' did not match any files", repo_path));
        });
    }

    #[test]
    fn test_add_unreadable_file() {
        crate::tests::run_test(|repo_path| {
            let file_path = format!("{}/hello.txt", repo_path);
            fs::write(&file_path, "Hello World").unwrap();
            let mut perms = fs::metadata(&file_path).unwrap().permissions();
            perms.set_mode(0o377);
            fs::set_permissions(&file_path, perms).unwrap();
            crate::tests::execute_and_expect_error_message(
                repo_path,
                vec!(String::new(), String::from("add"), String::from("hello.txt")),
                String::from("error: trying to read file 'hello.txt': Permission denied (os error 13)\nfatal: adding files failed"));
        });
    }

    #[test]
    fn test_add_index_lock_already_created() {
        crate::tests::run_test(|repo_path| {
            fs::write(format!("{}/.git/index.lock", repo_path), "Random Content").unwrap();
            crate::tests::execute_and_expect_error(
                repo_path,
                vec!(String::new(), String::from("add"), String::from("bad_path.txt")));
        });
    }
}