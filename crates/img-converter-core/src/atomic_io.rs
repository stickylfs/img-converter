use img_converter_common::{CollisionPolicy, ConversionError, ImageFormat};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Highest `name (N).ext` suffix tried before giving up.
const MAX_COLLISION_SUFFIX: u32 = 10_000;

/// An output path claimed for writing.
///
/// With [`CollisionPolicy::AutoIncrement`] and [`CollisionPolicy::Fail`] the path is
/// claimed atomically by creating an empty placeholder file (`O_CREAT | O_EXCL`),
/// so parallel conversions of `photo.jpg` and `photo.png` to WebP can never pick
/// the same name. The placeholder is removed if the reservation is dropped
/// without being committed.
#[derive(Debug)]
pub struct Reservation {
    path: PathBuf,
    placeholder: bool,
}

impl Reservation {
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Writes `data` to a temporary file next to the destination, flushes it to
    /// disk and atomically renames it over the reserved path.
    pub fn commit(mut self, data: &[u8]) -> Result<PathBuf, ConversionError> {
        write_atomic(&self.path, data)?;
        self.placeholder = false;
        Ok(std::mem::take(&mut self.path))
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        if self.placeholder {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Reserves `<dir>/<stem>.<ext>` according to `policy`, creating `dir` if needed.
pub fn reserve_destination(
    dir: &Path,
    stem: &str,
    target_format: ImageFormat,
    policy: CollisionPolicy,
) -> Result<Reservation, ConversionError> {
    fs::create_dir_all(dir).map_err(|e| ConversionError::IoError(e.to_string()))?;
    let ext = target_format.extension();
    let initial = dir.join(format!("{}.{}", stem, ext));

    match policy {
        CollisionPolicy::Overwrite => Ok(Reservation {
            path: initial,
            placeholder: false,
        }),
        CollisionPolicy::Fail => match create_placeholder(&initial)? {
            true => Ok(Reservation {
                path: initial,
                placeholder: true,
            }),
            false => Err(ConversionError::FileCollision(
                initial.display().to_string(),
            )),
        },
        CollisionPolicy::AutoIncrement => {
            if create_placeholder(&initial)? {
                return Ok(Reservation {
                    path: initial,
                    placeholder: true,
                });
            }
            for counter in 1..=MAX_COLLISION_SUFFIX {
                let candidate = dir.join(format!("{} ({}).{}", stem, counter, ext));
                if create_placeholder(&candidate)? {
                    return Ok(Reservation {
                        path: candidate,
                        placeholder: true,
                    });
                }
            }
            Err(ConversionError::FileCollision(format!(
                "Too many existing files named \"{}\" in {}",
                stem,
                dir.display()
            )))
        }
    }
}

/// Atomically creates an empty file; returns `false` if the path already exists.
fn create_placeholder(path: &Path) -> Result<bool, ConversionError> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(ConversionError::IoError(format!(
            "Cannot create {}: {}",
            path.display(),
            e
        ))),
    }
}

/// Writes `data` to a unique temporary file in the destination directory,
/// syncs it and renames it to `final_path` (replacing any existing file).
pub fn write_atomic(final_path: &Path, data: &[u8]) -> Result<(), ConversionError> {
    let parent = final_path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|e| ConversionError::IoError(e.to_string()))?;

    let temp_name = format!(
        ".{}.{}-{}.tmp",
        final_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("output"),
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed),
    );
    let temp_path = parent.join(temp_name);

    let result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp_path, final_path)
    })();

    result.map_err(|err| {
        let _ = fs::remove_file(&temp_path);
        ConversionError::IoError(format!("Failed to write {}: {}", final_path.display(), err))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("img_conv_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_collision_autoincrement() {
        let dir = test_dir("collision");
        let first = reserve_destination(
            &dir,
            "vacaciones",
            ImageFormat::Webp,
            CollisionPolicy::AutoIncrement,
        )
        .unwrap();
        // The first reservation is still pending: the second must not reuse its name.
        let second = reserve_destination(
            &dir,
            "vacaciones",
            ImageFormat::Webp,
            CollisionPolicy::AutoIncrement,
        )
        .unwrap();
        assert_eq!(first.path().file_name().unwrap(), "vacaciones.webp");
        assert_eq!(second.path().file_name().unwrap(), "vacaciones (1).webp");

        let written = first.commit(b"data").unwrap();
        assert_eq!(fs::read(&written).unwrap(), b"data");

        // Dropping an uncommitted reservation releases the name.
        let second_path = second.path().to_path_buf();
        drop(second);
        assert!(!second_path.exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_collision_overwrite_and_fail() {
        let dir = test_dir("overwrite");
        fs::write(dir.join("a.png"), b"old").unwrap();

        let fail = reserve_destination(&dir, "a", ImageFormat::Png, CollisionPolicy::Fail);
        assert!(matches!(fail, Err(ConversionError::FileCollision(_))));

        let overwrite =
            reserve_destination(&dir, "a", ImageFormat::Png, CollisionPolicy::Overwrite).unwrap();
        let path = overwrite.commit(b"new").unwrap();
        assert_eq!(fs::read(path).unwrap(), b"new");

        // No temporary files are left behind.
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        let _ = fs::remove_dir_all(&dir);
    }
}
