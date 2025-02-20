use anyhow::{Context, Result};
use serde::{Serialize, de::DeserializeOwned};

use self::connection::Connection;

pub mod connection;

/// Shorthand to pass zero parameters to helper functions.
///
/// Helpful as simply passing `[]` requires to pass a type explicitly and using `()` is wrong as
/// well because it's serialiazed as a single null value.
pub const NO_PARAMS: [(); 0] = [];

/// Execute a SQL command that doesn't return any values (`INSERT`, `UPDATE`, ...).
pub fn exec<P>(conn: &Connection, query: &str, params: P) -> Result<()>
where
    P: Serialize,
{
    conn.prepare_cached(query)
        .context("failed preparing query")?
        .execute(serde_rusqlite::to_params(params).context("failed converting params")?)
        .context("failed executing statement")?;

    Ok(())
}

/// Query a single row from the database.
pub fn query_one<P, T>(conn: &Connection, query: &str, params: P) -> Result<Option<T>>
where
    P: Serialize,
    T: DeserializeOwned,
{
    let mut stmt = conn
        .prepare_cached(query)
        .context("failed preparing query")?;
    let mut rows = stmt
        .query(serde_rusqlite::to_params(params).context("failed converting params")?)
        .context("failed executing statement")?;

    serde_rusqlite::from_rows_ref(&mut rows)
        .map(|r| r.context("failed reading row"))
        .next()
        .transpose()
}

/// Query multiple rows from the database.
pub fn query_vec<P, T>(conn: &Connection, query: &str, params: P) -> Result<Vec<T>>
where
    P: Serialize,
    T: DeserializeOwned,
{
    let mut stmt = conn
        .prepare_cached(query)
        .context("failed preparing query")?;
    let rows = stmt
        .query(serde_rusqlite::to_params(params).context("failed converting params")?)
        .context("failed executing statement")?;

    serde_rusqlite::from_rows(rows)
        .map(|r| r.context("failed reading row"))
        .collect()
}
