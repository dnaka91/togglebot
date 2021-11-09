//! Implementation for the rust doc search command that is rather complex and lives in its own
//! module.

use std::time::Duration;

use anyhow::{bail, Result};
use docsearch::{Index, SimplePath};
use lru_time_cache::LruCache;
use once_cell::sync::Lazy;
use tokio::{fs, sync::Mutex};
use tracing::{debug, warn};

const CACHE_DIR: &str = concat!("/tmp/", env!("CARGO_PKG_NAME"), "/doc-indexes");

/// Cache of previously resolved paths to allow quick retrieval of doc links for frequently used
/// items.
static LINK_CACHE: Lazy<Mutex<LruCache<String, String>>> =
    Lazy::new(|| Mutex::new(LruCache::with_capacity(500)));

/// Find the direct link to the documentation page of any crate item from its fully qualified name.
/// Uses an in-memory and local file cache to reduce the amount of repeated index file download &
/// processing.
pub async fn find(path: &str) -> Result<String> {
    if let Some(link) = LINK_CACHE.lock().await.get(path) {
        debug!("loaded link for `{}` from memory cache", path);
        return Ok(link.clone());
    }

    let path = match path.parse::<SimplePath>() {
        Ok(p) => p,
        Err(e) => return Ok(format!("The path `{}` is invalid: {}", path, e)),
    };
    let index_file = index_file_name(&path);

    let index = match index_from_file(&index_file).await {
        Ok(i) => {
            debug!("loaded index for `{}` from file cache", path.crate_name());
            i
        }
        Err(e) => {
            debug!("getting fresh index because: {:?}", e);
            index_from_remote(&path, &index_file).await?
        }
    };

    Ok(if let Some(link) = index.find_link(&path) {
        LINK_CACHE
            .lock()
            .await
            .insert(path.into_inner(), link.clone());
        link
    } else {
        format!("Item `{}` doesn't exist", path)
    })
}

/// Generate the file name for the index cache from the given path.
fn index_file_name(path: &SimplePath) -> String {
    format!("{}/{}.json", CACHE_DIR, path.crate_name())
}

/// Try to load the index from local file cache.
async fn index_from_file(file_name: &str) -> Result<Index> {
    let meta = fs::metadata(&file_name).await?;

    if meta.modified()?.elapsed()? > Duration::from_secs(60 * 60 * 24 * 3) {
        bail!("file outdated");
    }

    let buf = fs::read(&file_name).await?;
    let index = serde_json::from_slice(&buf)?;

    Ok(index)
}

/// Load a fresh index from the remote source.
///
/// After retrieval the index is saved to the local disk cache. Failing to do so will **not** return
/// an error to allow getting the index independent of disk errors.
async fn index_from_remote(path: &SimplePath, file_name: &str) -> Result<Index> {
    let index = docsearch::search(path.crate_name(), None).await?;

    if let Err(e) = save_index(&index, file_name).await {
        warn!("failed to save index to cache: {:?}", e);
    }

    Ok(index)
}

/// Save an index to its known cache folder.
async fn save_index(index: &Index, file_name: &str) -> Result<()> {
    let buf = serde_json::to_vec(index)?;

    fs::create_dir_all(CACHE_DIR).await?;
    fs::write(&file_name, buf).await?;

    Ok(())
}
