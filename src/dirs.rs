use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use unidirs::{Directories, UnifiedDirs, Utf8Path, Utf8PathBuf};

// Unwrap: We can't run the server without knowning where to place files, so panic here as there is
// no good recovery case other than throwing an error and shutting down.
pub static DIRS: Lazy<Dirs> = Lazy::new(|| Dirs::new().unwrap());

#[expect(clippy::struct_field_names)]
pub struct Dirs {
    database_file: Utf8PathBuf,
    settings_file: Utf8PathBuf,
    state_file: Utf8PathBuf,
    statistics_file: Utf8PathBuf,
}

impl Dirs {
    fn new() -> Result<Self> {
        let base = UnifiedDirs::simple("rocks", "dnaka91", env!("CARGO_PKG_NAME"))
            .default()
            .context("failed finding project directories")?;

        Ok(Self {
            database_file: base.data_dir().join("togglebot.db"),
            settings_file: base.config_dir().join("config.toml"),
            state_file: base.data_dir().join("state.json"),
            statistics_file: base.data_dir().join("statistics.json"),
        })
    }

    pub fn database_file(&self) -> &Utf8Path {
        &self.database_file
    }

    pub fn config_file(&self) -> &Utf8Path {
        &self.settings_file
    }

    pub fn state_file(&self) -> &Utf8Path {
        &self.state_file
    }

    pub fn statistics_file(&self) -> &Utf8Path {
        &self.statistics_file
    }
}
