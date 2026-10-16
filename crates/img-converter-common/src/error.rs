use crate::codec_support::WindowsCodec;
use crate::format::ImageFormat;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Granular errors encountered during the conversion pipeline.
#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("Unsupported image format: {0}")]
    UnsupportedFormat(String),

    #[error("Conversion from {from_format} to {to_format} is not permitted")]
    DisallowedConversion {
        from_format: ImageFormat,
        to_format: ImageFormat,
    },

    #[error("Failed to decode image: {0}")]
    DecodeError(String),

    #[error("Failed to encode image: {0}")]
    EncodeError(String),

    #[error("I/O error: {0}")]
    IoError(String),

    #[error("Security limit exceeded: {0}")]
    LimitExceeded(String),

    #[error("File is corrupted or malformed: {0}")]
    CorruptFile(String),

    #[error("Target file already exists: {0}")]
    FileCollision(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    /// Windows cannot read the file without (or with an outdated) optional codec.
    #[error(
        "{1}. Install or update the \"{name}\" from the Microsoft Store",
        name = .0.store_name()
    )]
    MissingCodec(WindowsCodec, String),
}

/// What went wrong, independent of the technical details: used to explain
/// failures to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureKind {
    UnsupportedFormat,
    DisallowedConversion,
    Unreadable,
    EncodeFailed,
    FileSystem,
    TooLarge,
    AlreadyExists,
    InvalidArgument,
    MissingCodec(WindowsCodec),
}

impl ConversionError {
    pub fn kind(&self) -> FailureKind {
        match self {
            ConversionError::UnsupportedFormat(_) => FailureKind::UnsupportedFormat,
            ConversionError::DisallowedConversion { .. } => FailureKind::DisallowedConversion,
            ConversionError::DecodeError(_) | ConversionError::CorruptFile(_) => {
                FailureKind::Unreadable
            }
            ConversionError::EncodeError(_) => FailureKind::EncodeFailed,
            ConversionError::IoError(_) => FailureKind::FileSystem,
            ConversionError::LimitExceeded(_) => FailureKind::TooLarge,
            ConversionError::FileCollision(_) => FailureKind::AlreadyExists,
            ConversionError::InvalidArgument(_) => FailureKind::InvalidArgument,
            ConversionError::MissingCodec(codec, _) => FailureKind::MissingCodec(*codec),
        }
    }

    /// Returns the standardized process exit code for CLI reporting.
    pub fn exit_code(&self) -> i32 {
        match self {
            ConversionError::InvalidArgument(_) => ExitCode::USAGE_ERROR,
            ConversionError::UnsupportedFormat(_)
            | ConversionError::DisallowedConversion { .. } => ExitCode::UNSUPPORTED_FORMAT,
            ConversionError::DecodeError(_)
            | ConversionError::CorruptFile(_)
            | ConversionError::MissingCodec(..) => ExitCode::DECODE_ERROR,
            ConversionError::EncodeError(_) => ExitCode::ENCODE_ERROR,
            ConversionError::IoError(_) | ConversionError::FileCollision(_) => {
                ExitCode::FILESYSTEM_ERROR
            }
            ConversionError::LimitExceeded(_) => ExitCode::DECODE_ERROR,
        }
    }
}

/// Standardized CLI process exit codes.
pub struct ExitCode;

impl ExitCode {
    pub const SUCCESS: i32 = 0;
    pub const USAGE_ERROR: i32 = 1;
    pub const UNSUPPORTED_FORMAT: i32 = 2;
    pub const DECODE_ERROR: i32 = 3;
    pub const ENCODE_ERROR: i32 = 4;
    pub const FILESYSTEM_ERROR: i32 = 5;
    pub const PARTIAL_BATCH_FAILURE: i32 = 6;
}
