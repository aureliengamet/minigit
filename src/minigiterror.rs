use std::fmt;

use backtrace::Backtrace;

pub type MinigitResult<T> = Result<T, MinigitError>;

pub struct MinigitError {
    pub message: String,
    pub backtrace: Backtrace,
}

impl MinigitError {
    pub fn new(message: String) -> MinigitError {
        MinigitError {
            message,
            backtrace: Backtrace::new(),
        }
    }
}

impl fmt::Debug for MinigitError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.message, f)
    }
}