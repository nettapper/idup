use crate::hash::{ImgHash, ImgHashKind};
use sqlx::{query, query_as, query_scalar, FromRow, SqlitePool};
use sqlx::sqlite::SqliteConnectOptions;
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
    let base = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| Path::new("/home/cd/.local/share").to_path_buf());
    let db_path = base.join(IDUP_DIR_NAME).join(IDUP_DB_NAME);
    let parent = db_path
        .parent()
        .expect("Can't determine parent dir for idup db")
        .to_path_buf();

    std::fs::create_dir_all(parent).expect("Can't create db parent directory");
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
