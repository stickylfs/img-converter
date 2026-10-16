use img_converter_common::ImageFormat;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use windows::core::PWSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    CreateProcessW, CREATE_NO_WINDOW, DETACHED_PROCESS, PROCESS_INFORMATION, STARTUPINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK, MB_SETFOREGROUND};

/// Windows limits a command line to 32,767 characters; beyond this budget the
/// selection is handed over in a temporary list file instead.
const MAX_COMMAND_LINE_CHARS: usize = 30_000;

static LIST_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Path of `img-converter.exe`, installed next to this DLL.
pub fn get_converter_exe_path() -> PathBuf {
    crate::module_dir()
        .unwrap_or_else(crate::default_install_dir)
        .join("img-converter.exe")
}

fn quote(path: &Path) -> String {
    // Windows file names cannot contain quotes. Backslashes are only special right
    // before the closing quote, so double a trailing run (e.g. a drive root `D:\`).
    let s = path.display().to_string();
    let trailing = s.len() - s.trim_end_matches('\\').len();
    format!("\"{}{}\"", s, "\\".repeat(trailing))
}

/// Builds the worker command line, falling back to a UTF-8 list file in %TEMP%
/// when the selection is too long to fit.
fn build_command_line(
    exe: &Path,
    files: &[PathBuf],
    target_format: ImageFormat,
) -> std::io::Result<String> {
    let mut cmd = format!("{} --notify {}", quote(exe), target_format.extension());
    let inline_len: usize = files.iter().map(|f| f.as_os_str().len() + 3).sum();

    if cmd.len() + inline_len <= MAX_COMMAND_LINE_CHARS {
        for file in files {
            cmd.push(' ');
            cmd.push_str(&quote(file));
        }
        return Ok(cmd);
    }

    let list_path = std::env::temp_dir().join(format!(
        "img-converter-{}-{}.txt",
        std::process::id(),
        LIST_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let content: String = files
        .iter()
        .map(|f| format!("{}\r\n", f.display()))
        .collect();
    std::fs::write(&list_path, content)?;
    cmd.push_str(" --list ");
    cmd.push_str(&quote(&list_path));
    Ok(cmd)
}

/// Starts a detached worker process converting `files` to `target_format`.
/// Failures are reported to the user: Explorer ignores the returned error.
pub fn spawn_converter(files: &[PathBuf], target_format: ImageFormat) -> windows_core::Result<()> {
    if files.is_empty() {
        return Ok(());
    }

    let converter_exe = get_converter_exe_path();
    if !converter_exe.exists() {
        let lang = img_converter_common::IniConfig::load_or_default().resolved_language();
        let folder = converter_exe
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        show_error(lang.launch_missing(&folder));
        return Err(windows_core::Error::from(
            windows::Win32::Foundation::ERROR_FILE_NOT_FOUND.to_hresult(),
        ));
    }
    let result = start_process(&converter_exe, files, target_format);
    if let Err(e) = &result {
        let lang = img_converter_common::IniConfig::load_or_default().resolved_language();
        show_error(lang.launch_failed(&e.message()));
    }
    result
}

/// Shows an error on its own thread, so Explorer's window stays responsive.
fn show_error(text: String) {
    std::thread::spawn(move || {
        let text: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            MessageBoxW(
                None,
                windows::core::PCWSTR(text.as_ptr()),
                windows::core::w!("img-converter"),
                MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
            );
        }
    });
}

fn start_process(
    converter_exe: &Path,
    files: &[PathBuf],
    target_format: ImageFormat,
) -> windows_core::Result<()> {
    let cmd_line = build_command_line(converter_exe, files, target_format)
        .map_err(|_| windows_core::Error::from(windows::Win32::Foundation::E_FAIL))?;
    let mut wide_cmd: Vec<u16> = cmd_line.encode_utf16().chain(std::iter::once(0)).collect();
    let app_name: Vec<u16> = converter_exe
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let startup_info = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut process_info = PROCESS_INFORMATION::default();

    unsafe {
        // An explicit application name prevents CreateProcess from searching the
        // current directory or PATH for a different "img-converter.exe".
        CreateProcessW(
            windows::core::PCWSTR(app_name.as_ptr()),
            PWSTR(wide_cmd.as_mut_ptr()),
            None,
            None,
            false,
            CREATE_NO_WINDOW | DETACHED_PROCESS,
            None,
            None,
            &startup_info,
            &mut process_info,
        )?;

        let _ = CloseHandle(process_info.hProcess);
        let _ = CloseHandle(process_info.hThread);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_selection_is_inline() {
        let files = vec![PathBuf::from(r"C:\Fotos\a b.jpg"), PathBuf::from(r"D:\")];
        let cmd = build_command_line(
            Path::new(r"C:\x\img-converter.exe"),
            &files,
            ImageFormat::Png,
        )
        .unwrap();
        assert_eq!(
            cmd,
            r#""C:\x\img-converter.exe" --notify png "C:\Fotos\a b.jpg" "D:\\""#
        );
    }

    #[test]
    fn test_long_selection_uses_list_file() {
        let files: Vec<PathBuf> = (0..2000)
            .map(|i| {
                PathBuf::from(format!(
                    r"C:\Users\someone\Pictures\Holidays\IMG_{i:05}.jpg"
                ))
            })
            .collect();
        let cmd =
            build_command_line(Path::new("img-converter.exe"), &files, ImageFormat::Webp).unwrap();
        assert!(cmd.len() < MAX_COMMAND_LINE_CHARS);
        let list = cmd.split("--list ").nth(1).unwrap().trim_matches('"');
        let content = std::fs::read_to_string(list).unwrap();
        assert_eq!(content.lines().count(), 2000);
        std::fs::remove_file(list).unwrap();
    }
}
