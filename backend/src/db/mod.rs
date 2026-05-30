use std::{
    env, fs,
    path::{Path, PathBuf},
};

use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

#[allow(dead_code)]
pub const MIGRATIONS_DIR: &str = "migrations";

pub async fn new_sqlite_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let pool = if let Some(path) = sqlite_file_path(database_url) {
        ensure_parent_dir(&path);

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);

        SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?
    } else {
        SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?
    };

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

fn ensure_parent_dir(path: &Path) {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        let _ = fs::create_dir_all(parent);
    }
}

fn sqlite_file_path(database_url: &str) -> Option<PathBuf> {
    if !database_url.starts_with("sqlite:") {
        return None;
    }

    if database_url.contains(":memory:") {
        return None;
    }

    let path_part = database_url
        .trim_start_matches("sqlite://")
        .trim_start_matches("sqlite:");

    if path_part.is_empty() {
        return None;
    }

    let candidate = PathBuf::from(path_part);
    if candidate.is_absolute() {
        return Some(candidate);
    }

    env::current_dir().ok().map(|cwd| cwd.join(candidate))
}
