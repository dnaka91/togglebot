//! Implementation for the rust doc search command that is rather complex and lives in its own
//! module.

use std::time::Duration;

use anyhow::{bail, Result};
use docsearch::{Fqn, Index};
use log::{debug, warn};
use lru_time_cache::LruCache;
use once_cell::sync::Lazy;
use tokio::{fs, sync::Mutex};

const CACHE_DIR: &str = concat!("/tmp/", env!("CARGO_CRATE_NAME"), "/doc-indexes");

/// Cache of previously resolved FQNs to allow quick retrieval of doc links for frequently used
/// items.
static LINK_CACHE: Lazy<Mutex<LruCache<String, String>>> =
    Lazy::new(|| Mutex::new(LruCache::with_capacity(500)));

/// Find the direct link to the documentation page of any crate item from its fully qualified name.
/// Uses an in-memory and local file cache to reduce the amount of repeated index file download &
/// processing.
pub async fn find(fqn: &str) -> Result<String> {
    if let Some(link) = LINK_CACHE.lock().await.get(fqn) {
        debug!("loaded link for `{}` from memory cache", fqn);
        return Ok(link.clone());
    }

    let fqn = fqn.parse::<Fqn>()?;
    let index_file = index_file_name(&fqn);

    let index = match index_from_file(&index_file).await {
        Ok(i) => {
            debug!("loaded index for `{}` from file cache", fqn.crate_name());
            i
        }
        Err(e) => {
            debug!("getting fresh index because: {:?}", e);
            index_from_remote(&fqn, &index_file).await?
        }
    };

    Ok(if let Some(link) = index.find_link(&fqn) {
        LINK_CACHE
            .lock()
            .await
            .insert(fqn.into_inner(), link.clone());
        link
    } else {
        format!("Item `{}` doesn't exist", fqn)
    })
}

/// Generate the file name for the index cache from the given FQN.
fn index_file_name(fqn: &Fqn) -> String {
    format!("{}/{}.json", CACHE_DIR, fqn.crate_name())
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
async fn index_from_remote(fqn: &Fqn, file_name: &str) -> Result<Index> {
    let index = docsearch::search(fqn.crate_name(), None).await?;

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
