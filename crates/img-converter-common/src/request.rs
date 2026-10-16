use crate::config::IniConfig;
use crate::error::{ConversionError, ExitCode, FailureKind};
use crate::format::ImageFormat;
use crate::limits::DecodeLimits;
use crate::policy::{CollisionPolicy, MetadataPolicy, TransparencyPolicy};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// High-level request payload for converting one or more images.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionRequest {
    /// List of input file paths.
    pub files: Vec<PathBuf>,
    /// Target image format.
    pub target_format: ImageFormat,
    /// Quality level (1-100) if applicable to target format. None uses format default.
    pub quality: Option<u8>,
    /// AVIF encoder speed (1 = slowest / smallest, 10 = fastest). None uses 6.
    pub avif_speed: Option<u8>,
    /// Scale factor for vector formats like SVG. None uses 1.0.
    pub scale: Option<f32>,
    /// Whether to generate multi-resolution ICO (16, 32, 48, 64, 128, 256) instead of single adapted size.
    pub ico_multi_resolution: bool,
    /// Rendering DPI for PDF page rasterization.
    pub pdf_dpi: Option<u32>,
    /// Whether to create a subfolder when extracting multi-page PDF images.
    pub pdf_create_subfolder: bool,
    /// Policy when target does not support alpha channel.
    pub transparency: TransparencyPolicy,
    /// Policy when destination filename already exists.
    pub collision: CollisionPolicy,
    /// Policy for preserving metadata.
    pub metadata: MetadataPolicy,
    /// Resource and security limits.
    pub limits: DecodeLimits,
    /// Optional custom output directory. If None, saves next to the source file.
    pub output_dir: Option<PathBuf>,
}

impl ConversionRequest {
    /// Builds a request for `files` using the user's `config.ini` settings.
    pub fn from_config(
        files: Vec<PathBuf>,
        target_format: ImageFormat,
        config: &IniConfig,
    ) -> Self {
        Self {
            files,
            target_format,
            quality: Some(config.quality_for(target_format)),
            avif_speed: Some(config.avif_speed),
            scale: Some(config.svg_scale),
            ico_multi_resolution: config.ico_multi_resolution,
            pdf_dpi: Some(config.pdf_dpi),
            pdf_create_subfolder: config.pdf_create_subfolder,
            transparency: config.transparency,
            collision: config.collision,
            metadata: config.metadata,
            limits: DecodeLimits::default(),
            output_dir: None,
        }
    }
}

/// Result of converting a single image file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleFileResult {
    /// Source input path.
    pub input_path: PathBuf,
    /// Destination path if successful.
    pub output_path: Option<PathBuf>,
    /// Input file size in bytes.
    pub input_bytes: u64,
    /// Output file size in bytes if successful.
    pub output_bytes: u64,
    /// Duration of processing in milliseconds.
    pub duration_ms: u64,
    /// Technical error message if conversion failed.
    pub error: Option<String>,
    /// Category of the failure, for user-facing explanations.
    #[serde(default)]
    pub failure: Option<FailureKind>,
}

impl SingleFileResult {
    pub fn success(
        input_path: PathBuf,
        output_path: PathBuf,
        input_bytes: u64,
        output_bytes: u64,
        duration_ms: u64,
    ) -> Self {
        Self {
            input_path,
            output_path: Some(output_path),
            input_bytes,
            output_bytes,
            duration_ms,
            error: None,
            failure: None,
        }
    }

    pub fn failure(input_path: PathBuf, error: &ConversionError, duration_ms: u64) -> Self {
        Self {
            input_path,
            output_path: None,
            input_bytes: 0,
            output_bytes: 0,
            duration_ms,
            error: Some(error.to_string()),
            failure: Some(error.kind()),
        }
    }

    pub fn is_success(&self) -> bool {
        self.error.is_none() && self.output_path.is_some()
    }
}

/// Overall summary of a batch conversion job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionSummary {
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    pub results: Vec<SingleFileResult>,
}

impl ConversionSummary {
    pub fn new(results: Vec<SingleFileResult>) -> Self {
        let total = results.len();
        let successful = results.iter().filter(|r| r.is_success()).count();
        let failed = total - successful;
        Self {
            total,
            successful,
            failed,
            results,
        }
    }

    /// Determines the appropriate process exit code for CLI reporting.
    pub fn exit_code(&self) -> i32 {
        if self.failed == 0 {
            ExitCode::SUCCESS
        } else if self.successful == 0 {
            ExitCode::DECODE_ERROR
        } else {
            ExitCode::PARTIAL_BATCH_FAILURE
        }
    }
}
