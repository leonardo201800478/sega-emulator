//! User-configurable storage directories and independent file-dialog history.
//!
//! The bootstrap pointer remains in the per-user application-data directory so
//! the selected configuration directory can itself be changed safely.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const APP_DATA_DIRECTORY: &str = "Sega Emulator";
const BOOTSTRAP_FILE: &str = "configuration_directory.txt";
const SETTINGS_FILE: &str = "directories.ini";

/// Independent directory categories exposed by the general settings screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryKind {
    Configuration,
    Roms,
    Bios,
    Logs,
    Saves,
    States,
    Screenshots,
    VulkanShaders,
    DirectXShaders,
    SharedShaders,
    ShaderPresets,
    ShaderTextures,
}

impl DirectoryKind {
    /// Stable key used in the on-disk settings file.
    const fn key(self) -> &'static str {
        match self {
            Self::Configuration => "configuration",
            Self::Roms => "roms",
            Self::Bios => "bios",
            Self::Logs => "logs",
            Self::Saves => "saves",
            Self::States => "states",
            Self::Screenshots => "screenshots",
            Self::VulkanShaders => "shaders_vulkan",
            Self::DirectXShaders => "shaders_directx",
            Self::SharedShaders => "shaders_shared",
            Self::ShaderPresets => "shader_presets",
            Self::ShaderTextures => "shader_textures",
        }
    }

    /// User-facing label in the general settings screen.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Configuration => "Configurações",
            Self::Roms => "ROMs",
            Self::Bios => "BIOS",
            Self::Logs => "Logs",
            Self::Saves => "SRAM / Saves",
            Self::States => "Save States",
            Self::Screenshots => "Capturas de tela",
            Self::VulkanShaders => "Shaders — Vulkan",
            Self::DirectXShaders => "Shaders — DirectX 11/12",
            Self::SharedShaders => "Shaders compartilhados (Slang)",
            Self::ShaderPresets => "Presets de shaders",
            Self::ShaderTextures => "Texturas de shaders",
        }
    }

    /// All supported categories in stable presentation order.
    pub const ALL: [Self; 12] = [
        Self::Configuration,
        Self::Roms,
        Self::Bios,
        Self::Logs,
        Self::Saves,
        Self::States,
        Self::Screenshots,
        Self::VulkanShaders,
        Self::DirectXShaders,
        Self::SharedShaders,
        Self::ShaderPresets,
        Self::ShaderTextures,
    ];
}

/// Persistent directory choices and last-visited folder for each category.
#[derive(Debug, Clone)]
pub struct DirectoryPreferences {
    paths: [PathBuf; 12],
    last_opened: [Option<PathBuf>; 12],
    settings_file: PathBuf,
    bootstrap_file: PathBuf,
}

impl DirectoryPreferences {
    /// Load persisted preferences, falling back to Documents-based defaults.
    pub fn load() -> Self {
        let bootstrap_directory = app_data_directory();
        let bootstrap_file = bootstrap_directory.join(BOOTSTRAP_FILE);
        let default_root = documents_directory()
            .unwrap_or_else(|| bootstrap_directory.clone())
            .join(APP_DATA_DIRECTORY);
        let configured_root = fs::read_to_string(&bootstrap_file)
            .ok()
            .map(|value| PathBuf::from(value.trim()))
            .filter(|path| path.is_dir())
            .unwrap_or_else(|| default_root.join("config"));
        let settings_file = configured_root.join(SETTINGS_FILE);

        let mut preferences = Self {
            paths: default_paths(&default_root),
            last_opened: std::array::from_fn(|_| None),
            settings_file,
            bootstrap_file,
        };
        preferences.paths[0] = configured_root;
        preferences.read_settings();
        preferences
    }

    /// Return the configured storage path for one category.
    pub fn path(&self, kind: DirectoryKind) -> &Path {
        &self.paths[index(kind)]
    }

    /// Return the last folder visited by the category's native picker.
    pub fn last_opened(&self, kind: DirectoryKind) -> Option<&Path> {
        self.last_opened[index(kind)].as_deref()
    }

    /// Select a new storage directory without changing any other category.
    ///
    /// The selected directory must already exist and be a directory. The
    /// selection also becomes the initial folder for the next picker opening.
    pub fn set_path(&mut self, kind: DirectoryKind, path: PathBuf) -> io::Result<()> {
        if !path.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("O diretório selecionado não existe: {}", path.display()),
            ));
        }

        let slot = index(kind);
        let previous_path = self.paths[slot].clone();
        let previous_history = self.last_opened[slot].clone();
        self.paths[slot].clone_from(&path);
        self.last_opened[slot] = Some(path);

        if let Err(error) = self.save() {
            self.paths[slot] = previous_path;
            self.last_opened[slot] = previous_history;
            return Err(error);
        }
        Ok(())
    }

    /// Record the most recently visited directory without changing storage.
    pub fn record_last_opened(&mut self, kind: DirectoryKind, path: PathBuf) -> io::Result<()> {
        if !path.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("O diretório visitado não existe: {}", path.display()),
            ));
        }
        let slot = index(kind);
        let previous = self.last_opened[slot].replace(path);
        if let Err(error) = self.save() {
            self.last_opened[slot] = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Choose a valid initial directory for a picker.
    ///
    /// History takes precedence, then the configured path, then Documents.
    pub fn picker_start_directory(&self, kind: DirectoryKind) -> PathBuf {
        self.last_opened(kind)
            .filter(|path| path.is_dir())
            .or_else(|| self.path(kind).is_dir().then_some(self.path(kind)))
            .map(Path::to_path_buf)
            .or_else(documents_directory)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    /// Persist settings atomically and update the bootstrap pointer last.
    ///
    /// This supports moving the configuration directory without losing the
    /// previous settings file if writing the new location fails.
    pub fn save(&mut self) -> io::Result<()> {
        let configuration_directory = self.paths[0].clone();
        fs::create_dir_all(&configuration_directory)?;
        let settings_file = configuration_directory.join(SETTINGS_FILE);
        let temporary_file = settings_file.with_extension("ini.tmp");
        let mut contents = String::from("version=1\n");
        for kind in DirectoryKind::ALL {
            let slot = index(kind);
            contents.push_str(kind.key());
            contents.push('=');
            contents.push_str(&self.paths[slot].to_string_lossy());
            contents.push('\n');
            contents.push_str("last_");
            contents.push_str(kind.key());
            contents.push('=');
            if let Some(path) = self.last_opened[slot].as_ref() {
                contents.push_str(&path.to_string_lossy());
            }
            contents.push('\n');
        }

        fs::write(&temporary_file, contents)?;
        replace_file(&temporary_file, &settings_file)?;

        let bootstrap_directory = self
            .bootstrap_file
            .parent()
            .ok_or_else(|| io::Error::other("Diretório bootstrap inválido"))?;
        fs::create_dir_all(bootstrap_directory)?;
        let temporary_bootstrap = self.bootstrap_file.with_extension("tmp");
        fs::write(
            &temporary_bootstrap,
            configuration_directory.to_string_lossy().as_bytes(),
        )?;
        replace_file(&temporary_bootstrap, &self.bootstrap_file)?;
        self.settings_file = settings_file;
        Ok(())
    }

    fn read_settings(&mut self) {
        let Ok(contents) = fs::read_to_string(&self.settings_file) else {
            return;
        };
        for line in contents.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            if value.is_empty() {
                continue;
            }
            for kind in DirectoryKind::ALL {
                let slot = index(kind);
                if key == kind.key() {
                    // Keep unavailable paths: removable drives and network shares
                    // may be disconnected temporarily and must not lose settings.
                    self.paths[slot] = PathBuf::from(value);
                } else if key == format!("last_{}", kind.key()) {
                    self.last_opened[slot] = Some(PathBuf::from(value));
                }
            }
        }
        // The bootstrap-selected directory is authoritative for this session.
        self.paths[0] = self
            .settings_file
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();
    }
}

fn index(kind: DirectoryKind) -> usize {
    match kind {
        DirectoryKind::Configuration => 0,
        DirectoryKind::Roms => 1,
        DirectoryKind::Bios => 2,
        DirectoryKind::Logs => 3,
        DirectoryKind::Saves => 4,
        DirectoryKind::States => 5,
        DirectoryKind::Screenshots => 6,
        DirectoryKind::VulkanShaders => 7,
        DirectoryKind::DirectXShaders => 8,
        DirectoryKind::SharedShaders => 9,
        DirectoryKind::ShaderPresets => 10,
        DirectoryKind::ShaderTextures => 11,
    }
}

fn app_data_directory() -> PathBuf {
    std::env::var_os("APPDATA")
        .map_or_else(std::env::temp_dir, PathBuf::from)
        .join(APP_DATA_DIRECTORY)
}

fn documents_directory() -> Option<PathBuf> {
    let profile = std::env::var_os("USERPROFILE").map(PathBuf::from)?;
    let candidates = [
        std::env::var_os("OneDrive")
            .map(PathBuf::from)
            .map(|path| path.join("Documents")),
        std::env::var_os("OneDriveCommercial")
            .map(PathBuf::from)
            .map(|path| path.join("Documents")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|path| path.is_dir())
        .or_else(|| Some(profile.join("Documents")))
}

fn default_paths(root: &Path) -> [PathBuf; 12] {
    [
        root.join("config"),
        root.join("roms"),
        root.join("bios"),
        root.join("logs"),
        root.join("saves"),
        root.join("states"),
        root.join("screenshots"),
        root.join("shaders").join("vulkan"),
        root.join("shaders").join("directx"),
        root.join("shaders").join("shared"),
        root.join("shaders").join("presets"),
        root.join("shaders").join("textures"),
    ]
}

fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    // std::fs::rename replaces an existing file on supported desktop targets;
    // do not remove the destination first, which would create a data-loss gap.
    fs::rename(source, destination)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_have_independent_indices_and_labels() {
        assert_eq!(DirectoryKind::ALL.len(), 12);
        assert_eq!(DirectoryKind::Roms.label(), "ROMs");
        assert_ne!(index(DirectoryKind::Roms), index(DirectoryKind::Bios));
        assert_ne!(index(DirectoryKind::Saves), index(DirectoryKind::States));
    }

    #[test]
    fn defaults_are_separate_subdirectories() {
        let paths = default_paths(Path::new("root"));
        assert_eq!(paths[0], PathBuf::from("root/config"));
        assert_eq!(paths[1], PathBuf::from("root/roms"));
        assert_eq!(paths[4], PathBuf::from("root/saves"));
        assert_eq!(paths[5], PathBuf::from("root/states"));
        assert_eq!(paths[6], PathBuf::from("root/screenshots"));
        assert_eq!(paths[7], PathBuf::from("root/shaders/vulkan"));
        assert_eq!(paths[8], PathBuf::from("root/shaders/directx"));
        assert_eq!(paths[9], PathBuf::from("root/shaders/shared"));
        assert_eq!(paths[10], PathBuf::from("root/shaders/presets"));
        assert_eq!(paths[11], PathBuf::from("root/shaders/textures"));
    }

    #[test]
    fn replace_file_keeps_destination_content_updated() {
        let root = std::env::temp_dir().join(format!(
            "sega-directory-settings-test-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create test directory");
        let source = root.join("settings.tmp");
        let destination = root.join("settings.ini");
        fs::write(&destination, "old").expect("write old file");
        fs::write(&source, "new").expect("write new file");
        replace_file(&source, &destination).expect("replace settings");
        assert_eq!(fs::read_to_string(destination).expect("read result"), "new");
        let _ = fs::remove_dir_all(root);
    }
}
