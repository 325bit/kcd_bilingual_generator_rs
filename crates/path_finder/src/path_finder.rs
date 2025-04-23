use directories::UserDirs;
use std::{
    fs,
    io::{Error, ErrorKind},
    path::PathBuf,
};
use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};

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
        if let Ok(steam_path) = Self::find_steam_installation() {
            self.game_path = steam_path;
            return Ok(&self.game_path);
        }

        if let Ok(gog_path) = Self::find_gog_installation() {
            self.game_path = gog_path;
            return Ok(&self.game_path);
        }

        // If neither installation is found, return an error.
        Err(Error::new(
            ErrorKind::NotFound,
            "KingdomComeDeliverance2 installation not found in Steam or GOG",
        ))
    }

    fn find_steam_installation() -> Result<PathBuf, Error> {
        let steam_install_path = get_steam_install_path_from_registry()?;

        if let Some(path) = check_default_steam_library_path(&steam_install_path) {
            return Ok(path);
        }

        if let Some(path) = resolve_other_steam_library_paths(&steam_install_path) {
            return Ok(path);
        }

        Err(Error::new(
            ErrorKind::NotFound,
            "KingdomComeDeliverance2 Steam installation not found",
        ))
    }

    fn find_gog_installation() -> Result<PathBuf, Error> {
        // Check registry first
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let gog_keys = vec!["SOFTWARE\\GOG Galaxy", "SOFTWARE\\WOW6432Node\\GOG Galaxy"];

        for key in gog_keys {
            if let Ok(gog_key) = hklm.open_subkey(key) {
                if let Ok(install_path) = gog_key.get_value::<String, _>("path") {
                    let path = PathBuf::from(install_path.replace("\\", "/"))
                        .parent()
                        .unwrap()
                        .join("Games")
                        .join("KingdomComeDeliverance2");

                    if path.exists() {
                        return Ok(path);
                    }
                }
            }
        }

        // Check common GOG paths
        let common_paths = vec![
            PathBuf::from(r"C:\GOG Games\KingdomComeDeliverance2"),
            PathBuf::from(r"D:\GOG Games\KingdomComeDeliverance2"),
            UserDirs::new()
                .map(|ud| ud.home_dir().join("GOG Games/KingdomComeDeliverance2"))
                .unwrap_or_default(),
        ];

        for path in common_paths {
            if path.join("Data").exists() {
                return Ok(path);
            }
        }

        Err(std::io::Error::new(
            ErrorKind::NotFound,
            "GOG installation not found",
        ))
    }
}

// Function to get the Steam installation path from the Windows registry, returning a PathBuf.
fn get_steam_install_path_from_registry() -> Result<PathBuf, Error> {
    let hkcu = RegKey::predef(HKEY_LOCAL_MACHINE);
    // Try both registry keys
    let keys = [r"SOFTWARE\Valve\Steam", r"SOFTWARE\WOW6432Node\Valve\Steam"];

    for key in &keys {
        if let Ok(subkey) = hkcu.open_subkey(key) {
            if let Ok(install_path) = subkey.get_value::<String, _>("InstallPath") {
                return Ok(PathBuf::from(install_path));
            }
        }
    }

    Err(Error::new(
        ErrorKind::NotFound,
        "Steam installation path not found in registry",
    ))
}

// Function to check the default Steam library path for KingdomComeDeliverance2.
fn check_default_steam_library_path(steam_install_path: &PathBuf) -> Option<PathBuf> {
    let game_path = steam_install_path
        .join("steamapps")
        .join("common")
        .join("KingdomComeDeliverance2");

    if game_path.exists() {
        return Some(game_path);
    }

    None
}

// Function to resolve other library paths from libraryfolders.vdf.
fn resolve_other_steam_library_paths(steam_install_path: &PathBuf) -> Option<PathBuf> {
    let vdf_path = steam_install_path
        .join("steamapps")
        .join("libraryfolders.vdf");

    if !vdf_path.exists() {
        return None;
    }

    let content = fs::read_to_string(vdf_path).ok()?;

    // Search line by line for lines containing the "path" key.
    for line in content.lines() {
        if line.contains("\"path\"") {
            // Use a simple split on quotes to extract the path value.
            // This assumes the line format is something like: `"path"		"X:\Some\Path"`
            let parts: Vec<&str> = line.split('"').collect();
            if parts.len() >= 4 {
                let custom_path_str = parts[3];
                let custom_path = PathBuf::from(custom_path_str)
                    .join("steamapps")
                    .join("common")
                    .join("KingdomComeDeliverance2");

                if custom_path.exists() {
                    return Some(custom_path);
                }
            }
        }
    }

    None
}
