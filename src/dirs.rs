use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use unidirs::{Directories, UnifiedDirs, Utf8Path, Utf8PathBuf};

// Unwrap: We can't run the server without knowning where to place files, so panic here as there is
// no good recovery case other than throwing an error and shutting down.
pub static DIRS: Lazy<Dirs> = Lazy::new(|| Dirs::new().unwrap());

pub struct Dirs {
    settings_file: Utf8PathBuf,
    state_file: Utf8PathBuf,
    state_temp_file: Utf8PathBuf,
    doc_indexes_dir: Utf8PathBuf,
    dirs: UnifiedDirs,
}

impl Dirs {
    fn new() -> Result<Self> {
        let dirs = UnifiedDirs::simple("rocks", "dnaka91", env!("CARGO_PKG_NAME"))
            .default()
            .context("failed finding project directories")?;

        Ok(Self {
            settings_file: dirs.config_dir().join("config.toml"),
            state_file: dirs.config_dir().join("state.json"),
            state_temp_file: dirs.config_dir().join("~temp-state.json"),
            doc_indexes_dir: dirs.cache_dir().join("doc-indexes"),
            dirs,
        })
    }

    pub fn config_file(&self) -> &Utf8Path {
        &self.settings_file
    }

    pub fn data_dir(&self) -> &Utf8Path {
        self.dirs.data_dir()
    }

    pub fn state_file(&self) -> &Utf8Path {
        &self.state_file
    }

    pub fn state_temp_file(&self) -> &Utf8Path {
        &self.state_temp_file
    }

    pub fn doc_indexes_dir(&self) -> &Utf8Path {
        &self.doc_indexes_dir
    }
}
