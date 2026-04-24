use thiserror::Error;

/// Common errors shared across crates.
#[derive(Debug, Error)]
pub enum CommonError {
    /// An invalid numeric value was provided for an enum conversion.
    #[error("invalid {enum_name} value: {value}")]
    InvalidEnumValue { enum_name: &'static str, value: u64 },

    /// A GUID could not be parsed or is malformed.
    #[error("invalid GUID: {0}")]
    InvalidGuid(String),

    /// A packet was too short or otherwise malformed.
    #[error("malformed packet: {0}")]
    MalformedPacket(String),

    /// An I/O error occurred during read/write.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// A string was not valid UTF-8.
    #[error("invalid utf-8: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),

    /// Generic / catch-all error with a description.
    #[error("{0}")]
    Other(String),
}

impl CommonError {
    /// Convenience constructor for `Other` variant.
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}
