use directories::UserDirs;
use std::{
    fs,
    io::{Error, ErrorKind},
    path::{Path, PathBuf},
};

const GAME_DIR_NAME: &str = "KingdomComeDeliverance2";

pub struct PathFinder {
    game_path: PathBuf,
}

impl PathFinder {
    pub fn new() -> Self {
        PathFinder {
            game_path: PathBuf::new(), // Initialize with an empty PathBuf
        }
    }

    /// Tries to find the KingdomComeDeliverance2 game installation path.
    /// It checks the Steam installation first, then GOG.
    /// If a valid path is found, it stores it in `game_path` and returns it.
    /// Otherwise, it returns an error.
    pub fn find_game_path(&mut self) -> Result<&PathBuf, Error> {
        if let Ok(steam_path) = platform::find_steam_installation() {
            self.game_path = steam_path;
            return Ok(&self.game_path);
        }

        if let Ok(gog_path) = platform::find_gog_installation() {
            self.game_path = gog_path;
            return Ok(&self.game_path);
        }

        // If neither installation is found, return an error.
        Err(Error::new(
            ErrorKind::NotFound,
            format!("{GAME_DIR_NAME} installation not found in Steam or GOG"),
        ))
    }
}

/// `<library_root>/steamapps/common/KingdomComeDeliverance2`, if it exists.
fn game_path_in_library(library_root: &Path) -> Option<PathBuf> {
    let game_path = library_root.join("steamapps").join("common").join(GAME_DIR_NAME);
    game_path.exists().then_some(game_path)
}

/// Resolves additional Steam library folders from `libraryfolders.vdf`.
fn resolve_other_steam_library_paths(steam_install_path: &Path) -> Option<PathBuf> {
    let vdf_path = steam_install_path.join("steamapps").join("libraryfolders.vdf");
    let content = fs::read_to_string(vdf_path).ok()?;

    // Search line by line for lines containing the "path" key.
    // The expected line format is something like: `"path"		"X:\Some\Path"`
    for line in content.lines() {
        if line.contains("\"path\"") {
            let parts: Vec<&str> = line.split('"').collect();
            if parts.len() >= 4 {
                if let Some(found) = game_path_in_library(Path::new(parts[3])) {
                    return Some(found);
                }
            }
        }
    }

    None
}

/// GOG installs the game either under a well-known "GOG Games" folder or a user-chosen one.
fn find_gog_in_common_paths() -> Option<PathBuf> {
    let mut candidates = vec![
        PathBuf::from(r"C:\GOG Games").join(GAME_DIR_NAME),
        PathBuf::from(r"D:\GOG Games").join(GAME_DIR_NAME),
    ];

    if let Some(user_dirs) = UserDirs::new() {
        candidates.push(user_dirs.home_dir().join("GOG Games").join(GAME_DIR_NAME));
    }

    candidates.into_iter().find(|path| path.join("Data").exists())
}

#[cfg(windows)]
mod platform {
    use super::{Error, ErrorKind, find_gog_in_common_paths, game_path_in_library, resolve_other_steam_library_paths};
    use std::path::{Path, PathBuf};
    use winreg::{RegKey, enums::HKEY_LOCAL_MACHINE};

    pub fn find_steam_installation() -> Result<PathBuf, Error> {
        let steam_install_path = steam_install_path_from_registry()?;

        if let Some(path) = game_path_in_library(&steam_install_path) {
            return Ok(path);
        }

        if let Some(path) = resolve_other_steam_library_paths(&steam_install_path) {
            return Ok(path);
        }

        Err(Error::new(ErrorKind::NotFound, "KingdomComeDeliverance2 Steam installation not found"))
    }

    pub fn find_gog_installation() -> Result<PathBuf, Error> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

        for key in ["SOFTWARE\\GOG Galaxy", "SOFTWARE\\WOW6432Node\\GOG Galaxy"] {
            if let Ok(gog_key) = hklm.open_subkey(key) {
                if let Ok(install_path) = gog_key.get_value::<String, _>("path") {
                    // `path` points at GalaxyClient.exe; the games live next to the Galaxy folder.
                    if let Some(root) = PathBuf::from(install_path.replace('\\', "/")).parent().map(Path::to_path_buf) {
                        let candidate = root.join("Games").join("KingdomComeDeliverance2");

                        if candidate.exists() {
                            return Ok(candidate);
                        }
                    }
                }
            }
        }

        find_gog_in_common_paths().ok_or_else(|| Error::new(ErrorKind::NotFound, "GOG installation not found"))
    }

    /// Reads the Steam installation path from the Windows registry.
    fn steam_install_path_from_registry() -> Result<PathBuf, Error> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        // Try both registry keys
        let keys = [r"SOFTWARE\Valve\Steam", r"SOFTWARE\WOW6432Node\Valve\Steam"];

        for key in &keys {
            if let Ok(subkey) = hklm.open_subkey(key) {
                if let Ok(install_path) = subkey.get_value::<String, _>("InstallPath") {
                    return Ok(PathBuf::from(install_path));
                }
            }
        }

        Err(Error::new(ErrorKind::NotFound, "Steam installation path not found in registry"))
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{Error, ErrorKind, UserDirs, find_gog_in_common_paths, game_path_in_library, resolve_other_steam_library_paths};
    use std::path::PathBuf;

    /// Well-known Steam root directories on Linux / macOS (including Flatpak installs).
    fn steam_roots() -> Vec<PathBuf> {
        let mut roots = Vec::new();

        if let Some(user_dirs) = UserDirs::new() {
            let home = user_dirs.home_dir();
            roots.push(home.join(".steam").join("steam"));
            roots.push(home.join(".local").join("share").join("Steam"));
            roots.push(home.join(".var").join("app").join("com.valvesoftware.Steam").join(".local/share/Steam"));
            roots.push(home.join("Library").join("Application Support").join("Steam"));
        }

        roots
    }

    pub fn find_steam_installation() -> Result<PathBuf, Error> {
        for root in steam_roots() {
            if let Some(path) = game_path_in_library(&root) {
                return Ok(path);
            }

            if let Some(path) = resolve_other_steam_library_paths(&root) {
                return Ok(path);
            }
        }

        Err(Error::new(ErrorKind::NotFound, "KingdomComeDeliverance2 Steam installation not found"))
    }

    pub fn find_gog_installation() -> Result<PathBuf, Error> {
        find_gog_in_common_paths().ok_or_else(|| Error::new(ErrorKind::NotFound, "GOG installation not found"))
    }
}
