//! Categorized downloads shared by the save path and the Files screen.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Category {
    #[default]
    All,
    Images,
    Videos,
    Audio,
    Documents,
    Stickers,
    Other,
}

impl Category {
    pub const ALL: [Self; 7] = [
        Self::All,
        Self::Images,
        Self::Videos,
        Self::Audio,
        Self::Documents,
        Self::Stickers,
        Self::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All files",
            Self::Images => "Images",
            Self::Videos => "Videos",
            Self::Audio => "Audio",
            Self::Documents => "Documents",
            Self::Stickers => "Stickers",
            Self::Other => "Other",
        }
    }

    pub fn of(path: &Path) -> Self {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if extension == "webp"
            && path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| stem.starts_with("sticker-"))
        {
            return Self::Stickers;
        }
        match extension.as_str() {
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "heic" | "heif" | "avif" | "svg"
            | "tif" | "tiff" => Self::Images,
            "mp4" | "mov" | "webm" | "mkv" | "avi" | "m4v" | "3gp" => Self::Videos,
            "mp3" | "m4a" | "ogg" | "opus" | "wav" | "flac" | "aac" => Self::Audio,
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "md" | "csv"
            | "rtf" | "odt" | "ods" | "epub" => Self::Documents,
            _ => Self::Other,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub category: Category,
    pub size: u64,
    pub modified: Option<std::time::SystemTime>,
}

pub fn root() -> Result<PathBuf, String> {
    imp::root()
}

pub fn scan() -> Result<Vec<Entry>, String> {
    imp::scan()
}

pub fn size_label(size: u64) -> String {
    if size >= 1_073_741_824 {
        format!("{:.1} GB", size as f64 / 1_073_741_824.0)
    } else if size >= 1_048_576 {
        format!("{:.1} MB", size as f64 / 1_048_576.0)
    } else {
        format!("{} KB", size.div_ceil(1024))
    }
}

#[cfg(not(target_family = "wasm"))]
mod imp {
    use super::*;

    pub(super) fn root() -> Result<PathBuf, String> {
        let base = std::env::var_os("XDG_DOWNLOAD_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("USERPROFILE")
                    .or_else(|| std::env::var_os("HOME"))
                    .filter(|value| !value.is_empty())
                    .map(|value| PathBuf::from(value).join("Downloads"))
            })
            .ok_or_else(|| "The Downloads folder could not be located.".to_string())?;
        Ok(base.join("Velo"))
    }

    pub(super) fn scan() -> Result<Vec<Entry>, String> {
        let root = root()?;
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let mut entries = Vec::new();
        read_folder(&root, None, &mut entries)?;
        for category in Category::ALL
            .into_iter()
            .filter(|category| *category != Category::All)
        {
            let folder = root.join(category.label());
            if folder.exists() && !folder.is_symlink() {
                read_folder(&folder, Some(category), &mut entries)?;
            }
        }
        Ok(entries)
    }

    fn read_folder(
        folder: &Path,
        category: Option<Category>,
        entries: &mut Vec<Entry>,
    ) -> Result<(), String> {
        for item in std::fs::read_dir(folder).map_err(|error| error.to_string())? {
            let item = item.map_err(|error| error.to_string())?;
            if !item
                .file_type()
                .map_err(|error| error.to_string())?
                .is_file()
            {
                continue;
            }
            let metadata = match item.metadata() {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.to_string()),
            };
            let path = item.path();
            entries.push(Entry {
                category: category.unwrap_or_else(|| Category::of(&path)),
                name: item.file_name().to_string_lossy().into_owned(),
                size: metadata.len(),
                modified: metadata.modified().ok(),
                path,
            });
        }
        Ok(())
    }
}

#[cfg(target_family = "wasm")]
mod imp {
    use super::*;
    pub(super) fn root() -> Result<PathBuf, String> {
        Err("Your browser manages downloaded files in its Downloads view.".into())
    }
    pub(super) fn scan() -> Result<Vec<Entry>, String> {
        Err("Your browser manages downloaded files in its Downloads view.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classifies_by_extension_without_case_or_substring_confusion() {
        assert_eq!(Category::of(Path::new("PHOTO.JPG")), Category::Images);
        assert_eq!(Category::of(Path::new("report.pdf.exe")), Category::Other);
        assert_eq!(Category::of(Path::new("voice.opus")), Category::Audio);
        assert_eq!(Category::of(Path::new("movie.mp4")), Category::Videos);
        assert_eq!(Category::of(Path::new("report.xlsx")), Category::Documents);
    }
}
