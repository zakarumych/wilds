use std::fmt::{self, Display, Formatter};

/// Device memory is exhausted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OutOfMemory;

impl Display for OutOfMemory {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        fmt.write_str("Device memory is exhausted")
    }
}

impl std::error::Error for OutOfMemory {}

/// Device doesn't expose memory compatible with required usage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NoCompatibleMemory;

impl Display for NoCompatibleMemory {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        fmt.write_str("No compatible memory found")
    }
}

impl std::error::Error for NoCompatibleMemory {}

/// Possible errors that may occur during allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Error {
    /// Device memory is exhausted.
    OutOfMemory { source: OutOfMemory },
    /// Device doesn't expose memory compatible with required usage.
    NoCompatibleMemory { source: NoCompatibleMemory },
}

impl Display for Error {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Error::OutOfMemory { source } => Display::fmt(source, fmt),
            Error::NoCompatibleMemory { source } => Display::fmt(source, fmt),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::OutOfMemory { source } => Some(source),
            Error::NoCompatibleMemory { source } => Some(source),
        }
    }
}

impl From<OutOfMemory> for Error {
    fn from(source: OutOfMemory) -> Self {
        Error::OutOfMemory { source }
    }
}

impl From<NoCompatibleMemory> for Error {
    fn from(source: NoCompatibleMemory) -> Self {
        Error::NoCompatibleMemory { source }
    }
}

/// Possible error that may occur during memory mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MappingError {
    /// Device memory is exhausted.
    OutOfMemory { source: OutOfMemory },

    /// Memory is not host-visible.
    NonHostVisible,

    /// Mapping requests exceeds block size.
    OutOfBounds,
}

impl Display for MappingError {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        match self {
            MappingError::OutOfMemory { source } => Display::fmt(source, fmt),
            MappingError::NonHostVisible => {
                fmt.write_str("Memory is not host-visible and cannot be mapped")
            }
            MappingError::OutOfBounds => {
                fmt.write_str("Mapping out of the memory object bounds")
            }
        }
    }
}

impl std::error::Error for MappingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MappingError::OutOfMemory { source } => Some(source),
            _ => None,
        }
    }
}

impl From<OutOfMemory> for MappingError {
    fn from(source: OutOfMemory) -> Self {
        MappingError::OutOfMemory { source }
    }
}
