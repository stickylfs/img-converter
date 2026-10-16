#![windows_subsystem = "windows"]

//! `img-converter.exe`: conversion worker launched by the Explorer extension,
//! also usable from scripts.
//!
//! ```text
//! img-converter.exe [--notify] [--to] <format> <file>...
//! img-converter.exe [--notify] [--to] <format> --list <utf8-file-with-one-path-per-line>
//! ```
//!
//! The process exit code reports the outcome (see `ExitCode`).

mod dialog;

use dialog::{task_dialog, task_dialog_with_details, Button, DialogIcon};
use img_converter_common::{
    ConversionRequest, ConversionSummary, ExitCode, FailureKind, ImageFormat, IniConfig, Language,
    SingleFileResult, WindowsCodec,
};
use img_converter_core::ImageEngine;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use windows::core::{w, PCWSTR};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// The error log is truncated once it grows beyond this size.
const MAX_LOG_BYTES: u64 = 1024 * 1024;

/// Maximum number of failed files listed in the notification dialog, and in
/// its technical details.
const MAX_LISTED_FAILURES: usize = 5;
const MAX_DETAILED_FAILURES: usize = 20;

const BUTTON_OK: i32 = 100;
const BUTTON_OPEN_LOG: i32 = 101;
/// First of the "Get <codec>" buttons, one per missing codec.
const BUTTON_STORE_FIRST: i32 = 110;

struct Args {
    target_format: ImageFormat,
    files: Vec<PathBuf>,
    notify: bool,
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut notify = false;
    let mut format: Option<String> = None;
    let mut files = Vec::new();

    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--notify" => notify = true,
            "--to" | "-t" => format = iter.next().cloned(),
            "--list" => {
                let list = iter.next().ok_or("--list requires a file path")?;
                files.extend(read_list_file(Path::new(list))?);
            }
            _ if format.is_none() => format = Some(arg.clone()),
            _ => files.push(PathBuf::from(arg)),
        }
    }

    let format = format.ok_or("Missing target format")?;
    let target_format = ImageFormat::from_extension(&format)
        .filter(|f| ImageFormat::OUTPUT_FORMATS.contains(f))
        .ok_or_else(|| format!("Unsupported target format: {}", format))?;
    if files.is_empty() {
        return Err("No input files".to_string());
    }

    Ok(Args {
        target_format,
        files,
        notify,
    })
}

/// Reads a UTF-8 file list written by the shell extension when the selection is
/// too long for a command line. The file is deleted once read.
fn read_list_file(path: &Path) -> Result<Vec<PathBuf>, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Cannot read file list {}: {}", path.display(), e))?;
    let _ = fs::remove_file(path);
    Ok(content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .collect())
}

fn main() {
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_MULTITHREADED,
        );
    }

    let raw_args: Vec<String> = env::args().collect();
    let notify = raw_args.iter().any(|a| a == "--notify");
    let config = IniConfig::load_or_default();
    let lang = config.resolved_language();

    let args = match parse_args(&raw_args) {
        Ok(args) => args,
        Err(e) => {
            log_error(&e);
            if notify {
                show_startup_error(lang, &e);
            }
            std::process::exit(ExitCode::USAGE_ERROR);
        }
    };

    let request = ConversionRequest::from_config(args.files, args.target_format, &config);
    let summary = ImageEngine::convert_batch(&request);

    for res in &summary.results {
        if let Some(err) = &res.error {
            log_error(&format!(
                "Failed converting {} to {}: {}",
                res.input_path.display(),
                args.target_format,
                err
            ));
        }
    }

    if args.notify && summary.failed > 0 {
        show_failures(lang, &summary, args.target_format);
    }

    std::process::exit(summary.exit_code());
}

/// Explains the failed conversions in plain language, with the technical
/// details in a collapsible section, and offers the fixes that apply: opening
/// the log or the Microsoft Store page of a missing Windows codec.
fn show_failures(lang: Language, summary: &ConversionSummary, target: ImageFormat) {
    let failures: Vec<&SingleFileResult> =
        summary.results.iter().filter(|r| !r.is_success()).collect();
    let file_name = |r: &SingleFileResult| {
        r.input_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| r.input_path.display().to_string())
    };

    let mut content = String::new();
    for res in failures.iter().take(MAX_LISTED_FAILURES) {
        let reason = match res.failure {
            Some(kind) => lang.failure_reason(kind, target),
            None => res.error.clone().unwrap_or_default(),
        };
        content.push_str(&format!("• {}: {}\n", file_name(res), reason));
    }
    if failures.len() > MAX_LISTED_FAILURES {
        content.push_str(&lang.more_failures(failures.len() - MAX_LISTED_FAILURES));
        content.push('\n');
    }

    // Windows codecs that would fix some of the failures, in order of appearance.
    let mut codecs: Vec<WindowsCodec> = Vec::new();
    for res in &failures {
        if let Some(FailureKind::MissingCodec(codec)) = res.failure {
            if !codecs.contains(&codec) {
                codecs.push(codec);
            }
        }
    }
    for codec in &codecs {
        content.push('\n');
        content.push_str(&lang.missing_codec_hint(*codec));
    }

    let mut details = String::new();
    for res in failures.iter().take(MAX_DETAILED_FAILURES) {
        details.push_str(&format!(
            "{}: {}\n",
            file_name(res),
            res.error.as_deref().unwrap_or_default()
        ));
    }
    let log = log_path().filter(|p| p.exists());
    if let Some(log) = &log {
        details.push('\n');
        details.push_str(&lang.log_location(&log.display().to_string()));
    }

    let store_labels: Vec<String> = codecs
        .iter()
        .map(|c| lang.install_codec_button(*c))
        .collect();
    let mut buttons: Vec<Button> = store_labels
        .iter()
        .zip(BUTTON_STORE_FIRST..)
        .map(|(text, id)| Button { id, text })
        .collect();
    if log.is_some() {
        buttons.push(Button {
            id: BUTTON_OPEN_LOG,
            text: lang.open_log_button(),
        });
    }
    buttons.push(Button {
        id: BUTTON_OK,
        text: lang.ok_button(),
    });

    let choice = task_dialog_with_details(
        lang.error_dialog_title(),
        &lang.failed_files(failures.len(), summary.total),
        content.trim_end(),
        details.trim_end(),
        &buttons,
        BUTTON_OK,
        DialogIcon::Warning,
    );
    match choice {
        BUTTON_OPEN_LOG => {
            if let Some(log) = log {
                let _ = Command::new("notepad.exe").arg(log).spawn();
            }
        }
        id if id >= BUTTON_STORE_FIRST => {
            if let Some(codec) = codecs.get((id - BUTTON_STORE_FIRST) as usize) {
                shell_open(codec.store_url());
            }
        }
        _ => {}
    }
}

/// Reports an error that prevented the conversion from starting.
fn show_startup_error(lang: Language, message: &str) {
    task_dialog(
        lang.error_dialog_title(),
        lang.startup_error(),
        message,
        &[],
        0,
        DialogIcon::Error,
    );
}

fn shell_open(target: &str) {
    let target: Vec<u16> = target.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(target.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        );
    }
}

fn log_path() -> Option<PathBuf> {
    IniConfig::config_path()
        .parent()
        .map(|dir| dir.join("last_error.log"))
}

fn log_error(msg: &str) {
    let Some(path) = log_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let too_big = fs::metadata(&path).is_ok_and(|m| m.len() > MAX_LOG_BYTES);
    let file = OpenOptions::new()
        .create(true)
        .append(!too_big)
        .write(true)
        .truncate(too_big)
        .open(&path);
    if let Ok(mut f) = file {
        let _ = writeln!(f, "[{}] {}", timestamp(), msg);
    }
}

/// Local time as `YYYY-MM-DD HH:MM:SS`.
fn timestamp() -> String {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        std::iter::once("img-converter.exe")
            .chain(list.iter().copied())
            .map(String::from)
            .collect()
    }

    #[test]
    fn test_parse_args() {
        let parsed = parse_args(&args(&["png", "a.jpg", "b.jpg"])).unwrap();
        assert_eq!(parsed.target_format, ImageFormat::Png);
        assert_eq!(parsed.files.len(), 2);
        assert!(!parsed.notify);

        let parsed = parse_args(&args(&["--notify", "--to", "JPEG", "a.png"])).unwrap();
        assert_eq!(parsed.target_format, ImageFormat::Jpeg);
        assert!(parsed.notify);

        assert!(parse_args(&args(&["png"])).is_err());
        assert!(parse_args(&args(&["xyz", "a.png"])).is_err());
        // Input-only formats are not valid targets.
        assert!(parse_args(&args(&["svg", "a.png"])).is_err());
    }

    #[test]
    fn test_list_file_is_read_and_deleted() {
        let list = std::env::temp_dir().join(format!("imgconv_list_{}.txt", std::process::id()));
        fs::write(&list, "C:\\a.png\r\n\r\nC:\\b c.png\r\n").unwrap();
        let parsed = parse_args(&args(&["webp", "--list", list.to_str().unwrap()])).unwrap();
        assert_eq!(
            parsed.files,
            vec![PathBuf::from("C:\\a.png"), PathBuf::from("C:\\b c.png")]
        );
        assert!(!list.exists());
    }
}
