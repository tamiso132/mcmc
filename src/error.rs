use std::fmt;

/// A custom error type for the application.
#[derive(Debug)]
pub enum AppError {
    /// Errors originating from the Slang compiler.
    Slang(shader_slang::Error),
    /// Errors from Vulkan operations (from `ash` or `vk-mem`).
    Vulkan(ash::vk::Result),
    /// Errors from I/O operations (e.g., reading/writing files).
    Io(std::io::Error),
    /// Errors related to C-style strings (e.g., null bytes).
    NulError(std::ffi::NulError),
    /// General string-based errors.
    Message(String),
}

/// Define how errors are displayed.
impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Slang(err) => write!(f, "Slang Error: {}", err),
            AppError::Vulkan(err) => write!(f, "Vulkan Error: {}", err),
            AppError::Io(err) => write!(f, "IO Error: {}", err),
            AppError::NulError(err) => write!(f, "FFI Nul Error: {}", err),
            AppError::Message(msg) => write!(f, "Error: {}", msg),
        }
    }
}

/// Implement the standard Error trait.
impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::Slang(err) => Some(err),
            AppError::Vulkan(err) => Some(err),
            AppError::Io(err) => Some(err),
            AppError::NulError(err) => Some(err),
            AppError::Message(_) => None,
        }
    }
}

/// Allow converting from `shader_slang::Error`.
impl From<shader_slang::Error> for AppError {
    fn from(err: shader_slang::Error) -> Self {
        AppError::Slang(err)
    }
}

/// Allow converting from `std::io::Error`.
impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err)
    }
}

/// Allow converting from `std::ffi::NulError`.
impl From<std::ffi::NulError> for AppError {
    fn from(err: std::ffi::NulError) -> Self {
        AppError::NulError(err)
    }
}

/// Allow converting from `String`.
impl From<String> for AppError {
    fn from(msg: String) -> Self {
        AppError::Message(msg)
    }
}

/// Allow converting from `&str`.
impl From<&str> for AppError {
    fn from(msg: &str) -> Self {
        AppError::Message(msg.to_string())
    }
}

/// Allow converting from `ash::vk::Result` (which is an enum, not an Error struct).
impl From<ash::vk::Result> for AppError {
    fn from(err: ash::vk::Result) -> Self {
        AppError::Vulkan(err)
    }
}

/// Define a custom Result type for convenience.
pub type AppResult<T> = Result<T, AppError>;
