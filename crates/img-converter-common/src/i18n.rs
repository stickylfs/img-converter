use crate::codec_support::WindowsCodec;
use crate::error::FailureKind;
use crate::format::ImageFormat;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LanguageMode {
    #[default]
    Auto,
    En,
    Es,
}

impl std::str::FromStr for LanguageMode {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "es" | "spanish" | "español" | "castellano" => Ok(LanguageMode::Es),
            "en" | "english" | "ingles" | "inglés" => Ok(LanguageMode::En),
            _ => Ok(LanguageMode::Auto),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Language {
    #[default]
    En,
    Es,
}

impl Language {
    /// Determines the active language based on a LanguageMode configuration setting.
    pub fn resolve(mode: LanguageMode) -> Self {
        match mode {
            LanguageMode::En => Language::En,
            LanguageMode::Es => Language::Es,
            LanguageMode::Auto => Self::detect_system_language(),
        }
    }

    /// Detects Windows system UI language using Win32 API.
    pub fn detect_system_language() -> Self {
        #[cfg(windows)]
        {
            #[link(name = "kernel32")]
            extern "system" {
                fn GetUserDefaultUILanguage() -> u16;
            }

            let lang_id = unsafe { GetUserDefaultUILanguage() };
            Self::from_lang_id(lang_id)
        }
        #[cfg(not(windows))]
        {
            Language::En
        }
    }

    /// Maps a Windows LANGID to Language enum.
    /// Primary language ID is extracted with `lang_id & 0x3FF`.
    /// 0x0A is LANG_SPANISH (Spain, Mexico, Argentina, Colombia, etc.).
    pub fn from_lang_id(lang_id: u16) -> Self {
        let primary_lang = lang_id & 0x3FF;
        if primary_lang == 0x0A {
            Language::Es
        } else {
            Language::En
        }
    }

    /// Root context menu title: "Convert" / "Convert (N)" vs "Convertir" / "Convertir (N)"
    pub fn convert_title(&self, count: usize) -> String {
        match self {
            Language::Es => {
                if count > 1 {
                    format!("Convertir ({})", count)
                } else {
                    "Convertir".to_string()
                }
            }
            Language::En => {
                if count > 1 {
                    format!("Convert ({})", count)
                } else {
                    "Convert".to_string()
                }
            }
        }
    }

    /// Configure menu item: "Configure..." vs "Configurar..."
    pub fn configure_title(&self) -> &'static str {
        match self {
            Language::Es => "Configurar...",
            Language::En => "Configure...",
        }
    }

    /// Tooltip / description for root command
    pub fn root_tooltip(&self) -> &'static str {
        match self {
            Language::Es => "Convertir formato de imagen",
            Language::En => "Convert image format",
        }
    }

    /// Tooltip for configure command
    pub fn configure_tooltip(&self) -> &'static str {
        match self {
            Language::Es => "Abrir configuración de img-converter (config.ini)",
            Language::En => "Open img-converter configuration (config.ini)",
        }
    }
}

/// Strings of the conversion error dialog shown by `img-converter.exe`.
impl Language {
    pub fn error_dialog_title(&self) -> &'static str {
        match self {
            Language::Es => "img-converter: error de conversión",
            Language::En => "img-converter: conversion error",
        }
    }

    /// Main instruction: what failed.
    pub fn failed_files(&self, failed: usize, total: usize) -> String {
        match (self, total) {
            (Language::Es, 1) => "No se ha podido convertir el archivo".to_string(),
            (Language::En, 1) => "The file could not be converted".to_string(),
            (Language::Es, _) => format!("No se han podido convertir {failed} de {total} archivos"),
            (Language::En, _) => format!("{failed} of {total} files could not be converted"),
        }
    }

    pub fn more_failures(&self, count: usize) -> String {
        match self {
            Language::Es => format!("… y {count} más"),
            Language::En => format!("… and {count} more"),
        }
    }

    /// Plain-language reason of a failed conversion.
    pub fn failure_reason(&self, kind: FailureKind, target: ImageFormat) -> String {
        let es = matches!(self, Language::Es);
        match kind {
            FailureKind::UnsupportedFormat => {
                if es {
                    "formato de archivo no compatible".to_string()
                } else {
                    "unsupported file format".to_string()
                }
            }
            FailureKind::DisallowedConversion => {
                if es {
                    format!(
                        "este tipo de archivo no se puede convertir a {}",
                        target.display_name()
                    )
                } else {
                    format!(
                        "this kind of file cannot be converted to {}",
                        target.display_name()
                    )
                }
            }
            FailureKind::Unreadable => {
                if es {
                    "el archivo está dañado o no se puede leer".to_string()
                } else {
                    "the file is damaged or cannot be read".to_string()
                }
            }
            FailureKind::EncodeFailed => {
                if es {
                    format!("no se ha podido crear el archivo {}", target.display_name())
                } else {
                    format!("the {} file could not be created", target.display_name())
                }
            }
            FailureKind::FileSystem => {
                if es {
                    "no se ha podido leer o guardar (¿archivo en uso, sin permisos o disco lleno?)"
                        .to_string()
                } else {
                    "could not read or save (file in use, no permission or disk full?)".to_string()
                }
            }
            FailureKind::TooLarge => {
                if es {
                    "la imagen supera los límites de tamaño (ver config.ini)".to_string()
                } else {
                    "the image exceeds the size limits (see config.ini)".to_string()
                }
            }
            FailureKind::AlreadyExists => {
                if es {
                    "ya existe un archivo con el mismo nombre".to_string()
                } else {
                    "a file with the same name already exists".to_string()
                }
            }
            FailureKind::InvalidArgument => {
                if es {
                    "el archivo no existe o no es un archivo".to_string()
                } else {
                    "the file does not exist or is not a file".to_string()
                }
            }
            FailureKind::MissingCodec(codec) => match (es, codec) {
                (true, WindowsCodec::Raw) => format!(
                    "Windows no puede leer esta foto RAW: instala o actualiza \"{}\"",
                    codec.store_name()
                ),
                (false, WindowsCodec::Raw) => format!(
                    "Windows cannot read this RAW photo: install or update the \"{}\"",
                    codec.store_name()
                ),
                (true, _) => format!(
                    "falta la extensión \"{}\" de Microsoft Store",
                    codec.store_name()
                ),
                (false, _) => format!(
                    "the \"{}\" from the Microsoft Store is missing",
                    codec.store_name()
                ),
            },
        }
    }

    /// Hint shown below the list when Store extensions can fix some failures.
    pub fn missing_codec_hint(&self, codec: WindowsCodec) -> String {
        match (self, codec) {
            (Language::Es, WindowsCodec::Heif) => "Las imágenes HEIC necesitan las extensiones gratuitas \"HEIF Image Extensions\" y \"HEVC Video Extensions\" de Microsoft Store.".to_string(),
            (Language::En, WindowsCodec::Heif) => "HEIC images need the \"HEIF Image Extensions\" and \"HEVC Video Extensions\" from the Microsoft Store.".to_string(),
            (Language::Es, _) => format!("La extensión gratuita \"{}\" de Microsoft Store lo soluciona.", codec.store_name()),
            (Language::En, _) => format!("The free \"{}\" from the Microsoft Store fixes this.", codec.store_name()),
        }
    }

    pub fn install_codec_button(&self, codec: WindowsCodec) -> String {
        match self {
            Language::Es => format!("Instalar {}", codec.store_name()),
            Language::En => format!("Get {}", codec.store_name()),
        }
    }

    pub fn open_log_button(&self) -> &'static str {
        match self {
            Language::Es => "Abrir registro",
            Language::En => "Open log",
        }
    }

    pub fn ok_button(&self) -> &'static str {
        match self {
            Language::Es => "Aceptar",
            Language::En => "OK",
        }
    }

    pub fn log_location(&self, path: &str) -> String {
        match self {
            Language::Es => format!("Registro completo: {path}"),
            Language::En => format!("Full log: {path}"),
        }
    }

    /// The shell extension cannot find `img-converter.exe` in `folder`.
    pub fn launch_missing(&self, folder: &str) -> String {
        match self {
            Language::Es => format!("No se ha podido iniciar la conversión: falta img-converter.exe en\n{folder}\n\nVuelve a ejecutar el instalador de img-converter y elige \"Reinstalar o reparar\"."),
            Language::En => format!("The conversion could not start: img-converter.exe is missing from\n{folder}\n\nRun the img-converter installer again and choose \"Reinstall or repair\"."),
        }
    }

    pub fn launch_failed(&self, error: &str) -> String {
        match self {
            Language::Es => format!("No se ha podido iniciar la conversión: {error}"),
            Language::En => format!("The conversion could not start: {error}"),
        }
    }

    pub fn startup_error(&self) -> &'static str {
        match self {
            Language::Es => "img-converter no ha podido iniciar la conversión",
            Language::En => "img-converter could not start the conversion",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lang_id_detection() {
        // Spanish locales: Spain (0x0C0A), Mexico (0x080A), Argentina (0x2C0A), US Spanish (0x540A)
        assert_eq!(Language::from_lang_id(0x0C0A), Language::Es);
        assert_eq!(Language::from_lang_id(0x080A), Language::Es);
        assert_eq!(Language::from_lang_id(0x2C0A), Language::Es);
        assert_eq!(Language::from_lang_id(0x040A), Language::Es);

        // English locales: US (0x0409), UK (0x0809), Australia (0x0C09), Canada (0x1009)
        assert_eq!(Language::from_lang_id(0x0409), Language::En);
        assert_eq!(Language::from_lang_id(0x0809), Language::En);
        assert_eq!(Language::from_lang_id(0x0C09), Language::En);

        // Other locales default to English: German (0x0407), French (0x040C), Japanese (0x0411)
        assert_eq!(Language::from_lang_id(0x0407), Language::En);
        assert_eq!(Language::from_lang_id(0x040C), Language::En);
        assert_eq!(Language::from_lang_id(0x0411), Language::En);
    }

    #[test]
    fn test_string_translations() {
        let es = Language::Es;
        let en = Language::En;

        assert_eq!(es.convert_title(1), "Convertir");
        assert_eq!(es.convert_title(3), "Convertir (3)");
        assert_eq!(es.configure_title(), "Configurar...");

        assert_eq!(en.convert_title(1), "Convert");
        assert_eq!(en.convert_title(3), "Convert (3)");
        assert_eq!(en.configure_title(), "Configure...");
    }
}
