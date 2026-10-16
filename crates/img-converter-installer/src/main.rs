#![windows_subsystem = "windows"]

//! `img-converter-setup.exe`: self-contained per-user installer and uninstaller.
//!
//! The generic install flow lives in `setup.rs`; this file adds the shell
//! extension registration and the img-converter texts.

mod dialog;
mod registry;
mod setup;
mod shortcut;

use dialog::{task_dialog, Button, DialogIcon};
use img_converter_common::IniConfig;
use img_converter_common::WindowsCodec;
use registry::{delete_reg_tree, delete_reg_value, get_reg_string, set_reg_string};
use setup::{App, File, Language, Paths, Shortcut, Texts};
use std::fs;
use std::process::Command;
use windows::Win32::System::Registry::HKEY_CURRENT_USER;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

const CLSID_STRING: &str = "{3CA619D1-34F6-42E1-B0E3-7AE990A32001}";
const HANDLER_NAME: &str = "ImgConverter";

const APPROVED_REG_KEY: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\Shell Extensions\\Approved";
const PDF_USER_CHOICE_KEY: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts\\.pdf\\UserChoice";
/// Uninstall-key value remembering the extra PDF ProgID handler registered at install.
const PDF_PROGID_VALUE: &str = "RegisteredPdfProgId";

const BUTTON_OPEN_STORE: i32 = 200;
const BUTTON_LATER: i32 = 201;

/// Classes whose classic context menu (`IContextMenu`) gets the handler, under
/// `HKCU\Software\Classes\<class>\shellex\ContextMenuHandlers\ImgConverter`.
const SHELL_CLASSES: &[&str] = &[
    "*",
    "SystemFileAssociations\\image",
    "SystemFileAssociations\\document",
    "SystemFileAssociations\\.jpg",
    "SystemFileAssociations\\.jpeg",
    "SystemFileAssociations\\.jfif",
    "SystemFileAssociations\\.png",
    "SystemFileAssociations\\.webp",
    "SystemFileAssociations\\.avif",
    "SystemFileAssociations\\.svg",
    "SystemFileAssociations\\.gif",
    "SystemFileAssociations\\.ico",
    "SystemFileAssociations\\.bmp",
    "SystemFileAssociations\\.tiff",
    "SystemFileAssociations\\.tif",
    "SystemFileAssociations\\.tga",
    "SystemFileAssociations\\.dds",
    "SystemFileAssociations\\.hdr",
    "SystemFileAssociations\\.exr",
    "SystemFileAssociations\\.heic",
    "SystemFileAssociations\\.heif",
    "SystemFileAssociations\\.cr2",
    "SystemFileAssociations\\.cr3",
    "SystemFileAssociations\\.nef",
    "SystemFileAssociations\\.arw",
    "SystemFileAssociations\\.dng",
    "SystemFileAssociations\\.raw",
    "SystemFileAssociations\\.pdf",
    "Acrobat.Document.DC",
    "AcroExch.Document.DC",
    "Acrobat.Document.2020",
    "AcroExch.Document.7",
    "MSEdgePDF",
    "ChromeHTML",
    "FirefoxHTML",
    "FoxitReaderDoc",
    "SumatraPDF",
    ".jpg",
    ".jpeg",
    ".jfif",
    ".png",
    ".webp",
    ".avif",
    ".svg",
    ".gif",
    ".ico",
    ".bmp",
    ".tiff",
    ".tif",
    ".tga",
    ".dds",
    ".hdr",
    ".exr",
    ".heic",
    ".heif",
    ".cr2",
    ".cr3",
    ".nef",
    ".arw",
    ".dng",
    ".raw",
    ".pdf",
];

fn handler_key(class: &str) -> String {
    format!("Software\\Classes\\{class}\\shellex\\ContextMenuHandlers\\{HANDLER_NAME}")
}

fn uninstall_key() -> String {
    format!(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{}",
        ImgConverter::ID
    )
}

/// ProgID of the user's default PDF app (e.g. Acrobat, Edge), if any.
fn default_pdf_prog_id() -> Option<String> {
    get_reg_string(HKEY_CURRENT_USER, PDF_USER_CHOICE_KEY, "ProgId")
        .ok()
        .filter(|id| !id.is_empty() && !id.contains('\\'))
}

struct ImgConverter;

impl App for ImgConverter {
    const ID: &'static str = "img-converter";
    const DISPLAY_NAME: &'static str = "img-converter";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const FILES: &'static [File] = &[
        File {
            name: "img-converter.exe",
            bytes: include_bytes!(concat!(env!("OUT_DIR"), "/img-converter.exe")),
        },
        File {
            name: "img_converter_shell.dll",
            bytes: include_bytes!(concat!(env!("OUT_DIR"), "/img_converter_shell.dll")),
        },
        File {
            name: "app.ico",
            bytes: include_bytes!("../../../assets/app.ico"),
        },
    ];

    fn texts(lang: Language) -> Texts {
        match lang {
            Language::En => Texts {
                description: "Adds fast, offline image conversions to the File Explorer right-click menu.".into(),
                usage: "Right-click any image or PDF in File Explorer and choose \"Convert\".\n\nOn Windows 11, the entry is under \"Show more options\" (or Shift + right-click).".into(),
                uninstall_question: "The program, its context menu entries and its settings will be removed.".into(),
                uninstall_done: "All components, context menu entries and settings have been removed.".into(),
                arch_reason: Some("The File Explorer context menu only works with the version that matches your system".into()),
                finish_action: Some(("Configure...".into(), false)),
            },
            Language::Es => Texts {
                description: "Añade conversiones de imágenes rápidas y sin conexión al menú contextual del Explorador de Windows.".into(),
                usage: "Haz clic derecho sobre cualquier imagen o PDF en el Explorador de Windows y elige \"Convertir\".\n\nEn Windows 11, la opción aparece en \"Mostrar más opciones\" (o con Mayús + clic derecho).".into(),
                uninstall_question: "Se eliminarán el programa, sus entradas del menú contextual y su configuración.".into(),
                uninstall_done: "Se han eliminado los componentes, las entradas del menú contextual y la configuración.".into(),
                arch_reason: Some("El menú contextual del Explorador de Windows solo funciona con la versión que coincide con tu sistema".into()),
                finish_action: Some(("Configurar...".into(), false)),
            },
        }
    }

    fn install(paths: &Paths, _lang: Language) -> Result<(), String> {
        // 1. config.ini, replacing configurations from pre-1.0 releases (no
        //    [menu] section) with the current defaults.
        let config_path = IniConfig::config_path();
        let outdated = fs::read_to_string(&config_path).map_or(true, |c| !c.contains("[menu]"));
        if outdated {
            if let Some(dir) = config_path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(&config_path, img_converter_common::DEFAULT_CONFIG_INI);
        }

        // 2. The COM server of the shell extension (per-user)
        let clsid_key = format!("Software\\Classes\\CLSID\\{CLSID_STRING}");
        let inproc_key = format!("{clsid_key}\\InprocServer32");
        let dll_path = paths.install_dir.join("img_converter_shell.dll");
        set_reg_string(
            HKEY_CURRENT_USER,
            &clsid_key,
            None,
            "img-converter Context Command",
        )
        .and_then(|_| {
            set_reg_string(
                HKEY_CURRENT_USER,
                &inproc_key,
                None,
                &dll_path.to_string_lossy(),
            )
        })
        .and_then(|_| {
            set_reg_string(
                HKEY_CURRENT_USER,
                &inproc_key,
                Some("ThreadingModel"),
                "Apartment",
            )
        })
        .map_err(|code| format!("Cannot register the shell extension (error {code})"))?;

        // 3. Context menu handlers, also for the default PDF app so the menu
        //    shows up for PDFs whatever app opens them.
        for class in SHELL_CLASSES {
            let _ = set_reg_string(HKEY_CURRENT_USER, &handler_key(class), None, CLSID_STRING);
        }
        let _ = set_reg_string(
            HKEY_CURRENT_USER,
            APPROVED_REG_KEY,
            Some(CLSID_STRING),
            Self::ID,
        );
        if let Some(prog_id) = default_pdf_prog_id() {
            let _ = set_reg_string(
                HKEY_CURRENT_USER,
                &handler_key(&prog_id),
                None,
                CLSID_STRING,
            );
            let _ = set_reg_string(
                HKEY_CURRENT_USER,
                &uninstall_key(),
                Some(PDF_PROGID_VALUE),
                &prog_id,
            );
        }
        Ok(())
    }

    fn uninstall(_paths: &Paths) {
        for class in SHELL_CLASSES {
            let _ = delete_reg_tree(HKEY_CURRENT_USER, &handler_key(class));
        }
        let registered = get_reg_string(HKEY_CURRENT_USER, &uninstall_key(), PDF_PROGID_VALUE)
            .ok()
            .filter(|id| !id.is_empty() && !id.contains('\\'));
        for prog_id in registered.into_iter().chain(default_pdf_prog_id()) {
            let _ = delete_reg_tree(HKEY_CURRENT_USER, &handler_key(&prog_id));
        }
        let _ = delete_reg_value(HKEY_CURRENT_USER, APPROVED_REG_KEY, CLSID_STRING);
        let _ = delete_reg_tree(
            HKEY_CURRENT_USER,
            &format!("Software\\Classes\\CLSID\\{CLSID_STRING}"),
        );
    }

    fn shortcuts(_paths: &Paths, lang: Language) -> Vec<Shortcut> {
        vec![Shortcut {
            name: lang
                .pick("Settings (config.ini)", "Configuración (config.ini)")
                .into(),
            target: IniConfig::config_path(),
            description: lang
                .pick(
                    "Edit the img-converter settings",
                    "Editar la configuración de img-converter",
                )
                .into(),
        }]
    }

    fn finish_action(_paths: &Paths) {
        let config_path = IniConfig::config_path();
        if !config_path.exists() {
            let _ = IniConfig::load_or_default();
        }
        let _ = Command::new("notepad.exe").arg(&config_path).spawn();
    }

    fn after_install(_paths: &Paths, lang: Language) {
        // AVIF needs the "AV1 Video Extension" (preinstalled on Windows 11, a
        // free download on Windows 10).
        if !WindowsCodec::Av1.is_installed() {
            prompt_codec(
                lang,
                lang.pick(
                    "The AV1 Video Extension is needed to read AVIF images",
                    "Falta la extensión de vídeo AV1 para abrir imágenes AVIF",
                ),
                lang.pick(
                    "img-converter uses the Windows codec to read AVIF files. Install the free \"AV1 Video Extension\" from the Microsoft Store to convert from AVIF.\n\nAll other formats (and converting to AVIF) work without it.",
                    "img-converter usa el códec de Windows para leer archivos AVIF. Instala la extensión gratuita \"Extensión de vídeo AV1\" de Microsoft Store para convertir desde AVIF.\n\nEl resto de formatos (y la conversión a AVIF) funcionan sin ella.",
                ),
                WindowsCodec::Av1.store_url(),
            );
        }
        // Current camera RAW formats (e.g. Canon .cr3) need the LibRaw-based
        // Windows RAW codec (built into Windows 10 1903+ and Windows 11).
        if !WindowsCodec::Raw.is_installed() {
            prompt_codec(
                lang,
                lang.pick(
                    "The Raw Image Extension is needed to read camera RAW photos",
                    "Falta la extensión \"Raw Image Extension\" para abrir fotos RAW de cámara",
                ),
                lang.pick(
                    "img-converter uses the Windows codec to read camera RAW files (.cr2, .cr3, .nef, .arw, .dng...). Install the free \"Raw Image Extension\" from the Microsoft Store to convert them at full quality.\n\nWithout it, only the preview embedded by the camera can be converted.",
                    "img-converter usa el códec de Windows para leer fotos RAW de cámara (.cr2, .cr3, .nef, .arw, .dng...). Instala la extensión gratuita \"Raw Image Extension\" de Microsoft Store para convertirlas con la máxima calidad.\n\nSin ella, solo se puede convertir la vista previa que incluye la cámara.",
                ),
                WindowsCodec::Raw.store_url(),
            );
        }
    }
}

/// Suggests a free Windows codec from the Microsoft Store.
fn prompt_codec(lang: Language, instruction: &str, content: &str, store_url: &str) {
    let choice = task_dialog(
        lang.pick("img-converter Setup", "Instalación de img-converter"),
        instruction,
        content,
        &[
            Button {
                id: BUTTON_OPEN_STORE,
                text: lang.pick(
                    "Open Microsoft Store\nInstall the free extension from Microsoft.",
                    "Abrir Microsoft Store\nInstalar la extensión gratuita de Microsoft.",
                ),
            },
            Button {
                id: BUTTON_LATER,
                text: lang.pick(
                    "Not now\nYou can install it later.",
                    "Ahora no\nSe puede instalar más adelante.",
                ),
            },
        ],
        BUTTON_OPEN_STORE,
        DialogIcon::Information,
    );
    if choice == BUTTON_OPEN_STORE {
        let url: Vec<u16> = store_url.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            ShellExecuteW(
                None,
                windows::core::w!("open"),
                windows::core::PCWSTR(url.as_ptr()),
                None,
                None,
                SW_SHOWNORMAL,
            );
        }
    }
}

fn main() {
    setup::run::<ImgConverter>();
}
