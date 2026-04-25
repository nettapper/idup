use crate::hash::{ImgHash, ImgHashKind};
use directories::ProjectDirs;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{query, query_as, query_scalar, FromRow, SqlitePool};
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};
use std::str::FromStr;

const IDUP_DIR_NAME: &str = "idup";
const IDUP_DB_NAME: &str = "idup.db3";

#[derive(Debug, FromRow)]
pub struct ImgData {
    pub path: String,
}

#[derive(Debug, FromRow)]
pub struct ImgDataGrouped {
    pub group_hash: String,
    pub path: String,
}

pub async fn open_pool() -> Result<SqlitePool, sqlx::Error> {
    let db_path = setup_dir();
    let db_url = format!("sqlite:{}", db_path.display());
    let connect_options = SqliteConnectOptions::from_str(&db_url)
        .map_err(|e| sqlx::Error::Configuration(e.into()))?
        .create_if_missing(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options)
        .await?;

    setup_db(&pool).await?;
    Ok(pool)
}

pub async fn exact_match(pool: &SqlitePool, path: &Path) -> Result<Vec<ImgData>, sqlx::Error> {
    let path = normalize_path(path);

    let query = "
        SELECT DISTINCT i_dup.path AS path
        FROM images i
        JOIN hashes h
          ON i.images_id = h.images_id
        JOIN hashes h_dup
          ON h.hash = h_dup.hash
        JOIN images i_dup
          ON h_dup.images_id = i_dup.images_id
        WHERE i.path = ?
          AND h.kind LIKE 'sha256%'
          AND h_dup.kind LIKE 'sha256%';
    ";

    query_as::<_, ImgData>(query)
        .bind(path)
        .fetch_all(pool)
        .await
}

pub async fn exact_matches_grouped(
    pool: &SqlitePool,
) -> Result<Vec<ImgDataGrouped>, sqlx::Error> {
    let query = "
        SELECT a.hash AS group_hash, i.path AS path
        FROM images i
        JOIN hashes a
          ON i.images_id = a.images_id
         AND a.kind = 'sha256 imgdata'
        WHERE a.hash IN (
            SELECT hash
            FROM hashes
            WHERE kind = 'sha256 imgdata'
            GROUP BY hash
            HAVING COUNT(*) > 1
        )
        ORDER BY a.hash, i.path;
    ";

    query_as::<_, ImgDataGrouped>(query).fetch_all(pool).await
}


pub async fn save(pool: &SqlitePool, img: &ImgHash) -> Result<(), sqlx::Error> {
    let path = normalize_path(&img.path);

    let image_id: i64 = match existing_image_id(pool, &path).await? {
        Some(id) => id,
        None => {
            query("INSERT INTO images (path) VALUES (?)")
                .bind(&path)
                .execute(pool)
                .await?;

            sqlx::query_scalar::<_, i64>("SELECT images_id FROM images WHERE path = ?")
                .bind(&path)
                .fetch_one(pool)
                .await?
        }
    };

    query(
        "INSERT INTO hashes (images_id, kind, hash)
         VALUES (?, ?, ?)
         ON CONFLICT(images_id, kind, hash)
         DO UPDATE SET hash = excluded.hash",
    )
    .bind(image_id)
    .bind(img.kind.to_string())
    .bind(&img.hash)
    .execute(pool)
    .await?;

    if let ImgHashKind::Phash = img.kind {
        save_partial_phash(pool, image_id, img).await?;
    }

    Ok(())
}

pub async fn random_images(
    pool: &SqlitePool,
    n: u32,
    filter: Option<&str>,
) -> Result<Vec<ImgData>, sqlx::Error> {
    match filter {
        None => {
            let query = "SELECT path FROM images ORDER BY RANDOM() LIMIT ?";
            query_as::<_, ImgData>(query)
                .bind(n)
                .fetch_all(pool)
                .await
        }
        Some(pattern) => {
            let query = "SELECT path FROM images WHERE path GLOB ? ORDER BY RANDOM() LIMIT ?";
            query_as::<_, ImgData>(query)
                .bind(pattern)
                .bind(n)
                .fetch_all(pool)
                .await
        }
    }
}

/// Returns all paths that are direct children of `dir` (non-recursive).
pub async fn images_in_dir(
    pool: &SqlitePool,
    dir: &str,
) -> Result<Vec<ImgData>, sqlx::Error> {
    let dir = dir.trim_end_matches('/');
    // Paths that start with "dir/" and have no further '/' after that prefix.
    let query = r#"
        SELECT path FROM images
        WHERE length(path) > length(?) + 1
          AND substr(path, 1, length(?) + 1) = ? || '/'
          AND instr(substr(path, length(?) + 2), '/') = 0
        ORDER BY path
    "#;
    query_as::<_, ImgData>(query)
        .bind(dir)
        .bind(dir)
        .bind(dir)
        .bind(dir)
        .fetch_all(pool)
        .await
}

/// Returns the full paths of immediate subdirectories of `dir` that contain at least one image.
pub async fn subdirs_in_dir(pool: &SqlitePool, dir: &str) -> Result<Vec<String>, sqlx::Error> {
    let dir = dir.trim_end_matches('/');
    let prefix = format!("{}/", dir);
    let pattern = format!("{}%", prefix);

    let rows = query_as::<_, ImgData>("SELECT path FROM images WHERE path LIKE ?")
        .bind(pattern)
        .fetch_all(pool)
        .await?;

    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for row in &rows {
        if let Some(rest) = row.path.strip_prefix(&prefix as &str) {
            if let Some(slash_pos) = rest.find('/') {
                seen.insert(format!("{}{}", prefix, &rest[..slash_pos]));
            }
        }
    }
    Ok(seen.into_iter().collect())
}

/// Returns all images matching a glob `filter`, optionally scoped to paths under `dir`.
pub async fn images_matching_filter_in_dir(
    pool: &SqlitePool,
    dir: Option<&str>,
    filter: &str,
) -> Result<Vec<ImgData>, sqlx::Error> {
    match dir {
        None => {
            query_as::<_, ImgData>(
                "SELECT path FROM images WHERE path GLOB ? ORDER BY path",
            )
            .bind(filter)
            .fetch_all(pool)
            .await
        }
        Some(dir) => {
            let dir = dir.trim_end_matches('/');
            let like_pat = format!("{}/", dir) + "%";
            query_as::<_, ImgData>(
                "SELECT path FROM images WHERE path GLOB ? AND path LIKE ? ORDER BY path",
            )
            .bind(filter)
            .bind(like_pat)
            .fetch_all(pool)
            .await
        }
    }
}

/// Returns up to `n` images in a deterministic pseudo-random order determined by `seed`.
/// Stable: given the same DB content, seed, and filter, the result is always identical.
pub async fn random_images_seeded(
    pool: &SqlitePool,
    n: u32,
    filter: Option<&str>,
    seed: u64,
) -> Result<Vec<ImgData>, sqlx::Error> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut rows = match filter {
        None => {
            query_as::<_, ImgData>("SELECT path FROM images ORDER BY path")
                .fetch_all(pool)
                .await?
        }
        Some(pat) => {
            query_as::<_, ImgData>(
                "SELECT path FROM images WHERE path GLOB ? ORDER BY path",
            )
            .bind(pat)
            .fetch_all(pool)
            .await?
        }
    };

    // Deterministic pseudo-random order: sort by hash(seed || path).
    rows.sort_by_key(|row| {
        let mut h = DefaultHasher::new();
        seed.hash(&mut h);
        row.path.hash(&mut h);
        h.finish()
    });

    rows.truncate(n as usize);
    Ok(rows)
}

/// Returns all image paths that share the given `sha256 imgdata` group hash.
pub async fn images_for_group(
    pool: &SqlitePool,
    group_hash: &str,
) -> Result<Vec<ImgData>, sqlx::Error> {
    let query = "
        SELECT i.path AS path
        FROM images i
        JOIN hashes h ON i.images_id = h.images_id
        WHERE h.kind = 'sha256 imgdata'
          AND h.hash = ?
        ORDER BY i.path;
    ";
    query_as::<_, ImgData>(query)
        .bind(group_hash)
        .fetch_all(pool)
        .await
}

/// Returns true if the given absolute path is tracked in the database.
pub async fn path_exists_in_db(pool: &SqlitePool, path: &str) -> Result<bool, sqlx::Error> {
    let count: i64 =
        query_scalar("SELECT COUNT(*) FROM images WHERE path = ?")
            .bind(path)
            .fetch_one(pool)
            .await?;
    Ok(count > 0)
}

/// Deletes all data from the database (images, hashes, partial_hashes).
pub async fn wipe_db(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    query("DELETE FROM partial_hashes").execute(pool).await?;
    query("DELETE FROM hashes").execute(pool).await?;
    query("DELETE FROM images").execute(pool).await?;
    Ok(())
}

pub async fn clear_hashes_for_path(pool: &SqlitePool, path: &Path) -> Result<(), sqlx::Error> {
    let path = normalize_path(path);

    query("DELETE FROM hashes WHERE images_id IN (SELECT images_id FROM images WHERE path = ?)")
        .bind(&path)
        .execute(pool)
        .await?;

    query("DELETE FROM partial_hashes WHERE images_id IN (SELECT images_id FROM images WHERE path = ?)")
        .bind(&path)
        .execute(pool)
        .await?;

    Ok(())
}

/// Get all image paths from the database, optionally filtered by a directory prefix.
pub async fn get_all_images(pool: &SqlitePool, filter_path: Option<&str>) -> Result<Vec<ImgData>, sqlx::Error> {
    match filter_path {
        None => {
            query_as::<_, ImgData>("SELECT path FROM images ORDER BY path")
                .fetch_all(pool)
                .await
        }
        Some(dir) => {
            // Return all paths that start with the given directory prefix
            let dir = dir.trim_end_matches('/');
            let pattern = format!("{}%", dir);
            query_as::<_, ImgData>("SELECT path FROM images WHERE path LIKE ? ORDER BY path")
                .bind(pattern)
                .fetch_all(pool)
                .await
        }
    }
}

/// Get all hash kinds stored for a given image path.
#[derive(Debug, FromRow)]
pub struct HashKindRow {
    pub kind: String,
}

pub async fn get_hash_kinds_for_image(pool: &SqlitePool, path: &str) -> Result<Vec<String>, sqlx::Error> {
    let results: Vec<HashKindRow> = query_as::<_, HashKindRow>(
        "SELECT DISTINCT kind FROM hashes WHERE images_id = (SELECT images_id FROM images WHERE path = ?)"
    )
    .bind(path)
    .fetch_all(pool)
    .await?;
    
    Ok(results.into_iter().map(|r| r.kind).collect())
}

/// Get a single hash from the database for a given path and kind.
#[derive(Debug, FromRow)]
pub struct HashRow {
    pub hash: String,
}

pub async fn get_single_hash(pool: &SqlitePool, path: &str, kind: &str) -> Result<Option<String>, sqlx::Error> {
    let result: Option<HashRow> = query_as::<_, HashRow>(
        "SELECT hash FROM hashes WHERE images_id = (SELECT images_id FROM images WHERE path = ?) AND kind = ?"
    )
    .bind(path)
    .bind(kind)
    .fetch_optional(pool)
    .await?;
    
    Ok(result.map(|r| r.hash))
}

#[derive(Debug, FromRow)]
pub struct HashCountRow {
    pub kind: String,
    pub count: i64,
}

pub struct DbStats {
    pub image_count: i64,
    pub hash_counts: Vec<HashCountRow>,
}

pub async fn db_stats(pool: &SqlitePool) -> Result<DbStats, sqlx::Error> {
    let image_count: i64 = query_scalar("SELECT COUNT(*) FROM images")
        .fetch_one(pool)
        .await?;

    let hash_counts: Vec<HashCountRow> = query_as::<_, HashCountRow>(
        "SELECT kind, COUNT(*) AS count FROM hashes GROUP BY kind ORDER BY kind",
    )
    .fetch_all(pool)
    .await?;

    Ok(DbStats {
        image_count,
        hash_counts,
    })
}

/// Delete an image and all its associated hashes from the database.
pub async fn delete_image(pool: &SqlitePool, path: &str) -> Result<(), sqlx::Error> {
    query("DELETE FROM partial_hashes WHERE images_id IN (SELECT images_id FROM images WHERE path = ?)")
        .bind(path)
        .execute(pool)
        .await?;
    
    query("DELETE FROM hashes WHERE images_id IN (SELECT images_id FROM images WHERE path = ?)")
        .bind(path)
        .execute(pool)
        .await?;
    
    query("DELETE FROM images WHERE path = ?")
        .bind(path)
        .execute(pool)
        .await?;
    
    Ok(())
}

async fn existing_image_id(pool: &SqlitePool, path: &str) -> Result<Option<i64>, sqlx::Error> {
    query_as::<_, (i64,)>("SELECT images_id FROM images WHERE path = ?")
        .bind(path)
        .fetch_optional(pool)
        .await
        .map(|row| row.map(|r| r.0))
}

fn normalize_path(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

async fn save_partial_phash(
    pool: &SqlitePool,
    image_id: i64,
    img: &ImgHash,
) -> Result<(), sqlx::Error> {
    let chunk_size = 4;
    let chunks: Vec<String> = img
        .hash
        .as_bytes()
        .chunks(chunk_size)
        .map(|chunk| String::from_utf8_lossy(chunk).to_string())
        .collect();

    for (idx, chunk) in chunks.iter().enumerate() {
        query(
            "INSERT OR REPLACE INTO partial_hashes (sequence, part_hash, images_id)
             values (?, ?, ?)",
        )
        .bind(idx as i64)
        .bind(chunk)
        .bind(image_id)
        .execute(pool)
        .await?;
    }

    Ok(())
}

fn setup_dir() -> PathBuf {
    if let Ok(override_path) = std::env::var("IDUP_DB_PATH") {
        let db_path = PathBuf::from(&override_path);
        let parent = db_path
            .parent()
            .expect("Can't determine parent dir for IDUP_DB_PATH");
        create_dir_all(parent).expect("Can't create db parent directory for IDUP_DB_PATH");
        println!("[idup] IDUP_DB_PATH is set — using db at: {}", override_path);
        return db_path;
    }

    let proj_dirs = ProjectDirs::from("", "", IDUP_DIR_NAME)
        .expect("Could not determine user data directory");

    let db_path = proj_dirs.data_dir().join(IDUP_DB_NAME);
    let parent = db_path
        .parent()
        .expect("Can't determine parent dir for idup db");

    create_dir_all(parent).expect("Can't create db parent directory");

    db_path
}

async fn setup_db(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    query(
        r#"
        CREATE TABLE IF NOT EXISTS images (
            images_id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT UNIQUE
        );

        CREATE TABLE IF NOT EXISTS hashes (
            images_id INTEGER,
            kind TEXT,
            hash TEXT,
            PRIMARY KEY (images_id, kind, hash),
            FOREIGN KEY (images_id) REFERENCES images (images_id)
        );

        -- this table only supports one kind of partial hash currently (phash)
        CREATE TABLE IF NOT EXISTS partial_hashes (
            images_id INTEGER,
            sequence INTEGER,
            part_hash TEXT,
            PRIMARY KEY (images_id, sequence),
            FOREIGN KEY (images_id) REFERENCES images (images_id)
        );
        "#,
    )
    .execute(pool)
    .await?;

    let hash_is_pk_col = query_scalar::<_, i64>(
        "SELECT COUNT(*)
         FROM pragma_table_info('hashes')
         WHERE name = 'hash'
           AND pk > 0",
    )
    .fetch_one(pool)
    .await?;

    if hash_is_pk_col == 0 {
        query(
            r#"
            CREATE TABLE hashes_v2 (
                images_id INTEGER,
                kind TEXT,
                hash TEXT,
                PRIMARY KEY (images_id, kind, hash),
                FOREIGN KEY (images_id) REFERENCES images (images_id)
            );

            INSERT INTO hashes_v2 (images_id, kind, hash)
            SELECT images_id, kind, hash
            FROM hashes;

            DROP TABLE hashes;

            ALTER TABLE hashes_v2 RENAME TO hashes;
            "#,
        )
        .execute(pool)
        .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::{ImgHash, ImgHashKind};
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn saves_multiple_sha256_variants_for_same_path() -> sqlx::Result<()> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;

        setup_db(&pool).await?;

        let path = Path::new("/tmp/test-idup-image.jpg").to_path_buf();

        let hashes = vec![
            ImgHash {
                path: path.clone(),
                kind: ImgHashKind::Sha256("imgdata".to_string()),
                hash: "11111111".to_string(),
            },
            ImgHash {
                path: path.clone(),
                kind: ImgHashKind::Sha256("imgdata rot90".to_string()),
                hash: "22222222".to_string(),
            },
            ImgHash {
                path,
                kind: ImgHashKind::Sha256("imgdata".to_string()),
                hash: "33333333".to_string(),
            },
        ];

        for hash in hashes {
            save(&pool, &hash).await?;
        }

        let path = "/tmp/test-idup-image.jpg";
        let count: i64 = query_scalar(
            "SELECT COUNT(*)
             FROM hashes
             WHERE images_id = (SELECT images_id FROM images WHERE path = ?)
               AND kind LIKE 'sha256%'
            "
        )
        .bind(path)
        .fetch_one(&pool)
        .await?;

        assert_eq!(count, 3);

        Ok(())
    }
}
