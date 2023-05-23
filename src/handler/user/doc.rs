//! Implementation for the rust doc search command that is rather complex and lives in its own
//! module.

#![allow(dead_code)]

use std::time::Duration;

use anyhow::{bail, Result};
use docsearch::{Index, SimplePath, Version};
use lru_time_cache::LruCache;
use once_cell::sync::{Lazy, OnceCell};
use reqwest::redirect;
use tokio::{fs, sync::Mutex};
use tracing::{debug, warn};
use unidirs::{Utf8Path, Utf8PathBuf};

use crate::dirs::DIRS;

/// Cache of previously resolved paths to allow quick retrieval of doc links for frequently used
/// items.
static LINK_CACHE: Lazy<Mutex<LruCache<String, String>>> =
    Lazy::new(|| Mutex::new(LruCache::with_capacity(500)));

/// Find the direct link to the documentation page of any crate item from its fully qualified name.
/// Uses an in-memory and local file cache to reduce the amount of repeated index file download &
/// processing.
pub async fn find(path: &str) -> Result<String> {
    if let Some(link) = LINK_CACHE.lock().await.get(path) {
        debug!(%path, "loaded link from memory cache");
        return Ok(link.clone());
    }

    let path = match path.parse::<SimplePath>() {
        Ok(p) => p,
        Err(e) => return Ok(format!("The path `{path}` is invalid: {e}")),
    };
    let index_file = index_file_name(&path);

    let index = match index_from_file(&index_file).await {
        Ok(i) => {
            debug!(crate = %path.crate_name(), "loaded index from file cache");
            i
        }
        Err(e) => {
            debug!(reason = ?e, "getting fresh index");
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
        format!("Item `{path}` doesn't exist")
    })
}

/// Generate the file name for the index cache from the given path.
fn index_file_name(path: &SimplePath) -> Utf8PathBuf {
    format!("{}/{}.json", DIRS.doc_indexes_dir(), path.crate_name()).into()
}

/// Try to load the index from local file cache.
async fn index_from_file(file_name: &Utf8Path) -> Result<Index> {
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
async fn index_from_remote(path: &SimplePath, file_name: &Utf8Path) -> Result<Index> {
    let state = docsearch::start_search(path.crate_name(), Version::Latest);
    let content = download_url(state.url()).await?;

    let state = state.find_index(&content)?;
    let content = download_url(state.url()).await?;

    let index = state.transform_index(&content)?;

    if let Err(e) = save_index(&index, file_name).await {
        warn!(error = ?e, "failed to save index to cache");
    }

    Ok(index)
}

/// Save an index to its known cache folder.
async fn save_index(index: &Index, file_name: &Utf8Path) -> Result<()> {
    let buf = serde_json::to_vec(index)?;

    fs::create_dir_all(DIRS.doc_indexes_dir()).await?;
    fs::write(&file_name, buf).await?;

    Ok(())
}

/// Download the given URL and return the content as string.
async fn download_url(url: &str) -> Result<String> {
    static CLIENT: OnceCell<reqwest::Client> = OnceCell::new();

    let client = CLIENT.get_or_try_init(|| {
        reqwest::Client::builder()
            .redirect(redirect::Policy::limited(10))
            .build()
    })?;

    client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await
        .map_err(Into::into)
}
