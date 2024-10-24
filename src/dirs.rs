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
    statistics_file: Utf8PathBuf,
    statistics_temp_file: Utf8PathBuf,
    base: UnifiedDirs,
}

impl Dirs {
    fn new() -> Result<Self> {
        let base = UnifiedDirs::simple("rocks", "dnaka91", env!("CARGO_PKG_NAME"))
            .default()
            .context("failed finding project directories")?;

        Ok(Self {
            settings_file: base.config_dir().join("config.toml"),
            state_file: base.data_dir().join("state.json"),
            state_temp_file: base.data_dir().join("~temp-state.json"),
            statistics_file: base.data_dir().join("statistics.json"),
            statistics_temp_file: base.data_dir().join("~temp-statistics.json"),
            base,
        })
    }

    pub fn config_file(&self) -> &Utf8Path {
        &self.settings_file
    }

    pub fn data_dir(&self) -> &Utf8Path {
        self.base.data_dir()
    }

    pub fn state_file(&self) -> &Utf8Path {
        &self.state_file
    }

    pub fn state_temp_file(&self) -> &Utf8Path {
        &self.state_temp_file
    }

    pub fn statistics_file(&self) -> &Utf8Path {
        &self.statistics_file
    }

    pub fn statistics_temp_file(&self) -> &Utf8Path {
        &self.statistics_temp_file
    }
}
