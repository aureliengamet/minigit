extern crate backtrace;
extern crate chrono;
extern crate crypto;
extern crate flate2;
extern crate rand;

use crate::command::Runtime;

mod workspace;
mod database;
mod gitobject;
mod refs;
mod lockfile;
mod index;
mod repository;
mod minigiterror;
mod command;

fn main() {
    let mut runtime = Runtime::default();
    std::process::exit(match command::execute(&mut runtime) {
        Ok(_) => 0,
        Err(error) => {
            writeln!(&mut runtime.stderr, "{}", error.message).unwrap();
            if let Ok(_) = runtime.get_env_var("MINIGIT_DEBUG") {
                writeln!(&mut runtime.stderr, "{:?}", error.backtrace).unwrap();
            }
            1
        }
    });
}

fn oid_to_compressed_u8_array(oid: &str) -> [u8; 20] {
    let mut result = [0; 20];
    for i in 0..(oid.len() / 2) {
        result[i] = u8::from_str_radix(&oid[i * 2..i * 2 + 2], 16).unwrap();
    }
    result
}

fn uncompress_u8_array_to_oid(input: &[u8]) -> String {
    let mut result = String::new();
    for byte in input {
        result.push_str(&format!("{:x}{:x}", byte >> 4, byte & 0xf));
    }
    result
}

fn u32_to_u8_array_big_endian(number: u32) -> [u8; 4] {
    [
        (number >> 24 & 0xff) as u8,
        (number >> 16 & 0xff) as u8,
        (number >> 8 & 0xff) as u8,
        (number & 0xff) as u8]
}

fn u8_array_to_u32_big_endian(input: &[u8]) -> u32 {
    let mut result = 0;
    result += (input[0] as u32) << 24;
    result += (input[1] as u32) << 16;
    result += (input[2] as u32) << 8;
    result += input[3] as u32;
    result
}

fn u16_to_u8_array_big_endian(number: u16) -> [u8; 2] {
    [
        (number >> 8 & 0xff) as u8,
        (number & 0xff) as u8]
}

fn u8_array_to_u16_big_endian(input: &[u8]) -> u16 {
    let mut result = 0;
    result += (input[0] as u16) << 8;
    result += input[1] as u16;
    result
}


#[cfg(test)]
mod tests {
    use std::{fs, panic};
    use std::io::Cursor;
    use std::iter;
    use std::path::PathBuf;

    use rand::distributions::Alphanumeric;
    use rand::Rng;

    use crate::command::{execute, Runtime};
    use crate::minigiterror::MinigitResult;
    use crate::repository::Repository;

    fn before_test() -> String {
        let mut rng = rand::thread_rng();
        let repo_name: String = iter::repeat(())
            .map(|_| rng.sample(Alphanumeric))
            .take(20)
            .collect();
        let repo_path = format!("/private/tmp/minigit_test/{}", repo_name);
        fs::create_dir_all(&repo_path).unwrap();
        let mut runtime = Runtime::default();
        runtime.dir = PathBuf::from(&repo_path);
        runtime.args = vec!(String::from("minigit"), String::from("init"), repo_path.clone());
        execute(&mut runtime).unwrap();
        repo_path
    }

    fn after_test(repo_path: String) {
        fs::remove_dir_all(repo_path).unwrap();
    }

    pub fn run_test<T>(test: T) -> () where T: FnOnce(&str) -> () + panic::UnwindSafe {
        let repo_path = before_test();
        let result = panic::catch_unwind(|| {
            test(&repo_path)
        });
        after_test(repo_path);
        assert!(result.is_ok())
    }

    fn execute_and_get_result(repo_path: &str, args: Vec<String>) -> MinigitResult<()> {
        let mut runtime = Runtime::default();
        runtime.dir = PathBuf::from(repo_path);
        runtime.args = args;
        execute(&mut runtime)
    }

    pub fn execute_and_expect_success(repo_path: &str, args: Vec<String>) {
        if let Err(e) = execute_and_get_result(repo_path, args) {
            panic!("Command terminated with an error, when success was expected: {}", e.message)
        }
    }

    pub fn execute_and_expect_success_message(repo_path: &str, args: Vec<String>, expected_stdout: String) {
        let mut stdout = String::new();
        let stdout_cursor = unsafe {
            Cursor::new(stdout.as_mut_vec())
        };
        {
            let mut runtime = Runtime::default();
            runtime.dir = PathBuf::from(repo_path);
            runtime.args = args;
            runtime.stdout = Box::new(stdout_cursor);
            if let Err(e) = execute(&mut runtime) {
                panic!("Command terminated with an error, when success was expected: {}", e.message)
            }
        }
        assert_eq!(expected_stdout, stdout);
    }

    pub fn execute_and_expect_error(repo_path: &str, args: Vec<String>) {
        if let Ok(()) = execute_and_get_result(repo_path, args) {
            panic!("Command executed succesfully, but en error was expected");
        }
    }

    pub fn execute_and_expect_error_message(repo_path: &str, args: Vec<String>, expected_error_message: String) {
        let mut runtime = Runtime::default();
        runtime.dir = PathBuf::from(repo_path);
        runtime.args = args;
        match execute(&mut runtime) {
            Ok(()) => panic!("Command executed succesfully, but en error was expected"),
            Err(err) => assert_eq!(expected_error_message, err.message),
        }
    }

    pub fn assert_index(repo_path: &str, expected_entries: Vec<(u32, String)>) {
        let mut repository = Repository::new(PathBuf::from(repo_path));
        let index = repository.index_take().unwrap();
        let actual_entries: Vec<(u32, String)> = index.load_and_get_entries().unwrap()
            .into_iter().map(|value| (value.get_mode(), String::from(value.get_path_as_str())))
            .collect();
        assert_eq!(expected_entries, actual_entries);
    }
}