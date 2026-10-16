pub mod codec_support;
pub mod config;
pub mod error;
pub mod format;
pub mod i18n;
pub mod limits;
pub mod matrix;
pub mod policy;
pub mod request;

pub use codec_support::WindowsCodec;
pub use config::{IniConfig, DEFAULT_CONFIG_INI};
pub use error::{ConversionError, ExitCode, FailureKind};
pub use format::ImageFormat;
pub use i18n::{Language, LanguageMode};
pub use limits::DecodeLimits;
pub use matrix::ConversionMatrix;
pub use policy::{CollisionPolicy, MetadataPolicy, TransparencyPolicy};
pub use request::{ConversionRequest, ConversionSummary, SingleFileResult};
