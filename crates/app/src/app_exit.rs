use std::num::NonZero;

/// An event that indicates, that the application should exit.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AppExit {
    /// The application exited without any problems.
    #[default]
    Success,

    /// The application exited with an error.
    /// Holds the exit code that the process should return.
    Error(NonZero<u8>),
}

impl AppExit {
    #[must_use]
    pub const fn error() -> Self {
        Self::Error(NonZero::<u8>::MIN)
    }

    #[must_use]
    pub const fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    #[must_use]
    pub const fn from_code(code: u8) -> Self {
        match NonZero::<u8>::new(code) {
            Some(code) => Self::Error(code),
            None => Self::Success,
        }
    }
}

impl From<u8> for AppExit {
    fn from(value: u8) -> Self {
        Self::from_code(value)
    }
}

impl From<AppExit> for u8 {
    fn from(value: AppExit) -> Self {
        match value {
            AppExit::Success => 0,
            AppExit::Error(value) => value.get(),
        }
    }
}

impl std::process::Termination for AppExit {
    fn report(self) -> std::process::ExitCode {
        match self {
            Self::Success => std::process::ExitCode::SUCCESS,
            Self::Error(value) => std::process::ExitCode::from(value.get()),
        }
    }
}
