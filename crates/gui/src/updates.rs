//! In-app updates: is there a newer Velo, and installing it.
//!
//! Velo asks GitHub's public API for the project's latest release, the same
//! page anyone downloads from, and compares its version with this build's.
//! Nothing about the person or their account is sent: the request carries a
//! generic user agent and nothing else.
//!
//! Installing downloads the release's installer, checks its SHA-256 against
//! the `SHA256SUMS.txt` published beside it, and opens it. The installer is
//! the ordinary one: it shows its Update screen, replaces Velo as one set and
//! keeps the sign-in, exactly as when run by hand.
//!
//! Windows only for now. A page is updated by its server, and a Linux
//! install by its package manager, so there the helpers below go unused.
#![cfg_attr(not(windows), allow(dead_code))]

/// Where releases are published.
pub const RELEASES_API: &str = "https://api.github.com/repos/Ruchin-Audichya/velo/releases/latest";

/// A newer release, ready to install.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// Without the leading `v`: `1.3.1`.
    pub version: String,
    /// The release's page, for its notes.
    pub page: String,
    pub installer: String,
    pub checksums: Option<String>,
}

/// Where an update check got to, for Settings and the side rail.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available(Release),
    Installing(String),
    Failed(String),
}

impl UpdateState {
    /// The version on offer, while one is.
    pub fn available(&self) -> Option<&Release> {
        match self {
            Self::Available(release) => Some(release),
            _ => None,
        }
    }
}

/// Whether `candidate` is a later version than `current`: numeric parts
/// compared in order, a missing part counting as zero, anything after `-`
/// (a pre-release tag) ignored, as is a leading `v`.
#[must_use]
pub fn is_newer(candidate: &str, current: &str) -> bool {
    fn parts(version: &str) -> Vec<u64> {
        version
            .trim()
            .trim_start_matches('v')
            .split('-')
            .next()
            .unwrap_or("")
            .split('.')
            .map(|part| part.parse().unwrap_or(0))
            .collect()
    }
    let (candidate, current) = (parts(candidate), parts(current));
    let len = candidate.len().max(current.len());
    for index in 0..len {
        let a = candidate.get(index).copied().unwrap_or(0);
        let b = current.get(index).copied().unwrap_or(0);
        if a != b {
            return a > b;
        }
    }
    false
}

/// Read the latest release out of GitHub's answer, when it is newer than
/// `current` and has an installer for this machine.
#[must_use]
pub fn release_from(json: &serde_json::Value, current: &str) -> Option<Release> {
    if json["draft"].as_bool() == Some(true) || json["prerelease"].as_bool() == Some(true) {
        return None;
    }
    let tag = json["tag_name"].as_str()?;
    if !is_newer(tag, current) {
        return None;
    }
    let assets = json["assets"].as_array()?;
    let url_of = |wanted: &dyn Fn(&str) -> bool| {
        assets.iter().find_map(|asset| {
            let name = asset["name"].as_str()?;
            wanted(name)
                .then(|| asset["browser_download_url"].as_str().map(str::to_string))
                .flatten()
        })
    };
    let installer = url_of(&|name| name.starts_with("Velo-Setup-") && name.ends_with("-x64.exe"))?;
    // Only from this project's own releases, whatever the answer says.
    if !installer.starts_with("https://github.com/Ruchin-Audichya/velo/releases/download/") {
        return None;
    }
    Some(Release {
        version: tag.trim_start_matches('v').to_string(),
        page: json["html_url"].as_str().unwrap_or_default().to_string(),
        installer,
        checksums: url_of(&|name| name == "SHA256SUMS.txt"),
    })
}

/// The checksum `SHA256SUMS.txt` lists for `file`, lowercase hex.
#[must_use]
pub fn expected_sum(sums: &str, file: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, name) = line.split_once(char::is_whitespace)?;
        (name.trim().trim_start_matches('*') == file && hash.len() == 64)
            .then(|| hash.to_ascii_lowercase())
    })
}

/// Ask whether a newer Velo is out. Blocking: run off the UI thread.
///
/// # Errors
///
/// The request failed or the answer could not be read.
pub fn check() -> Result<Option<Release>, String> {
    imp::check()
}

/// Download, verify and open the installer for `release`. Blocking.
///
/// # Errors
///
/// The download failed, its checksum did not match, or it would not start.
pub fn install(release: &Release) -> Result<(), String> {
    imp::install(release)
}

/// Whether this build can update itself.
pub const SUPPORTED: bool = cfg!(windows);

#[cfg(windows)]
mod imp {
    use super::Release;
    use std::io::Read as _;

    const AGENT: &str = concat!("Velo/", env!("CARGO_PKG_VERSION"), " (in-app updater)");
    /// Larger than any installer Velo has shipped, by a wide margin, so a
    /// broken or hostile answer cannot fill the disk.
    const MAX_INSTALLER: u64 = 256 * 1024 * 1024;

    fn get(url: &str) -> Result<ureq::http::Response<ureq::Body>, String> {
        ureq::get(url)
            .header("User-Agent", AGENT)
            .header("Accept", "application/vnd.github+json")
            .call()
            .map_err(|error| format!("the update server could not be reached ({error})"))
    }

    pub(super) fn check() -> Result<Option<Release>, String> {
        let mut text = String::new();
        get(super::RELEASES_API)?
            .into_body()
            .into_reader()
            .take(2 * 1024 * 1024)
            .read_to_string(&mut text)
            .map_err(|error| format!("the update answer could not be read ({error})"))?;
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|error| format!("the update answer was not understood ({error})"))?;
        Ok(super::release_from(&json, env!("CARGO_PKG_VERSION")))
    }

    pub(super) fn install(release: &Release) -> Result<(), String> {
        use sha2::Digest as _;
        let name = release
            .installer
            .rsplit('/')
            .next()
            .filter(|name| name.starts_with("Velo-Setup-") && name.ends_with(".exe"))
            .ok_or("the installer's name was not recognised")?
            .to_string();
        let expected = match &release.checksums {
            Some(url) => {
                let mut sums = String::new();
                get(url)?
                    .into_body()
                    .into_reader()
                    .take(64 * 1024)
                    .read_to_string(&mut sums)
                    .map_err(|error| format!("the checksums could not be read ({error})"))?;
                Some(
                    super::expected_sum(&sums, &name)
                        .ok_or("the release lists no checksum for its installer")?,
                )
            }
            None => None,
        };
        let folder = velo_ipc::state_dir()
            .ok_or("Velo's data folder could not be located")?
            .join("updates");
        std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
        let path = folder.join(&name);
        let mut bytes = Vec::new();
        get(&release.installer)?
            .into_body()
            .into_reader()
            .take(MAX_INSTALLER)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("the download was interrupted ({error})"))?;
        if let Some(expected) = expected {
            let actual: String = sha2::Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            if actual != expected {
                return Err(
                    "the download did not match its published checksum, so it was not opened"
                        .into(),
                );
            }
        }
        std::fs::write(&path, &bytes).map_err(|error| error.to_string())?;
        std::process::Command::new(&path)
            .spawn()
            .map_err(|error| format!("the installer would not start ({error})"))?;
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Release;

    pub(super) fn check() -> Result<Option<Release>, String> {
        Ok(None)
    }

    pub(super) fn install(_: &Release) -> Result<(), String> {
        Err("updates are installed through your package manager here".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number_not_text() {
        assert!(is_newer("v1.3.1", "1.3.0"));
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(is_newer("2.0", "1.99.99"));
        assert!(!is_newer("1.3.0", "1.3.0"));
        assert!(!is_newer("v1.2.9", "1.3.0"));
        assert!(!is_newer("1.3.0-beta", "1.3.0"));
    }

    fn answer(tag: &str, installer_host: &str) -> serde_json::Value {
        serde_json::json!({
            "tag_name": tag,
            "html_url": format!("https://github.com/Ruchin-Audichya/velo/releases/tag/{tag}"),
            "draft": false,
            "prerelease": false,
            "assets": [
                {"name": format!("Velo-Setup-{}-x64.exe", tag.trim_start_matches('v')),
                 "browser_download_url": format!("{installer_host}/Velo-Setup-x64.exe")},
                {"name": "SHA256SUMS.txt",
                 "browser_download_url": format!("{installer_host}/SHA256SUMS.txt")}
            ]
        })
    }

    #[test]
    fn a_newer_release_from_this_project_is_offered() {
        let host = "https://github.com/Ruchin-Audichya/velo/releases/download/v9.0.0";
        let release = release_from(&answer("v9.0.0", host), "1.3.0").unwrap();
        assert_eq!(release.version, "9.0.0");
        assert!(release.checksums.is_some());
        assert!(
            release_from(&answer("v1.3.0", host), "1.3.0").is_none(),
            "not newer"
        );
    }

    #[test]
    fn an_installer_from_anywhere_else_is_refused() {
        let elsewhere = answer("v9.0.0", "https://example.invalid/download");
        assert!(release_from(&elsewhere, "1.3.0").is_none());
    }

    #[test]
    fn checksums_are_read_by_file_name() {
        let sums = "AB12".repeat(16)
            + "  Velo-Setup-1.3.1-x64.exe\n"
            + &"cd".repeat(32)
            + "  Velo-1.3.1-x64-portable.zip\n";
        assert_eq!(
            expected_sum(&sums, "Velo-Setup-1.3.1-x64.exe"),
            Some("ab12".repeat(16))
        );
        assert_eq!(expected_sum(&sums, "missing.exe"), None);
    }
}
