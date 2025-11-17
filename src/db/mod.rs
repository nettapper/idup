use rusqlite::{params, Connection, Result};
use std::path::{Path,PathBuf};
use std::fs::create_dir_all;
use std::vec::Vec;
use crate::hash::{ImgHash, ImgHashKind};
use directories::ProjectDirs;

// TODO might need to mv all const to common location
const IDUP_DIR_NAME: &str = "idup";
const IDUP_DB_NAME: &str = "idup.db3";

#[derive(Debug)]
pub struct ImgData {
    pub path: PathBuf,
    pub sha256: String,
    pub phash: String
}

pub fn exact_match(path: &PathBuf) -> Result<Vec<ImgData>> {
    println!("exact_match on path: {:?}", path);
    // TODO I could use a struct & pass that in to enforce the absolute path
    // SAFETY: all paths passed to the db need to be absolute
    let path = path.canonicalize().unwrap();
    let conn = open_db()?;
    // TODO should we show the given path in the output?
    let mut stmt = conn.prepare("
        SELECT DISTINCT i_dup.path
        FROM images i
        JOIN hashes h
          ON i.images_id = h.images_id
        JOIN hashes h_dup
          ON h.hash = h_dup.hash
        JOIN images i_dup
          ON h_dup.images_id = i_dup.images_id
        WHERE i.path = (?1)
          AND h.kind LIKE 'sha256%'
          AND h_dup.kind LIKE 'sha256%'
        ;"
    )?;
    let iter = stmt.query_map([path.to_str()], |row| {
        let path: String = row.get(0)?;
        // TODO ImgData might not be the right struct here
        Ok(ImgData {
            path: Path::new(&path).to_path_buf(),
            sha256: "".to_string(),
            phash: "".to_string(),
        })
    })?;
    iter.collect()
}

pub fn exact_matches() -> Result<Vec<ImgData>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare("
        SELECT DISTINCT i.path, b.cnt
        FROM images i
        JOIN hashes a
          ON i.images_id = a.images_id
        JOIN (
          SELECT hash, count(*) as cnt
          FROM hashes
          WHERE kind like 'sha256%'
          GROUP BY hash
          HAVING count(*) > 1
        ) b
          ON a.hash = b.hash
        ORDER BY b.cnt DESC
        ;"
    )?;
    let iter = stmt.query_map([], |row| {
        let s: String = row.get(0)?;
        Ok(ImgData {
            path: Path::new(&s).to_path_buf(),
            sha256: "".to_string(),
            phash: "".to_string(), // TODO this isn't the phash
        })
    })?;
    iter.collect()
}

pub fn save(img: &ImgHash) -> Result<(), rusqlite::Error> {
    let conn = open_db()?;
    // check if img already in db
    let mut stmt = conn.prepare("SELECT count(*) FROM images WHERE path = (?1)")?;
    let iter = stmt.query_map([img.path.to_str()], |row| row.get(0))?;
    let count = iter.fold(0, |acc, elem: Result<u64, rusqlite::Error>| acc + elem.unwrap_or(0));  // TODO error handling
    if count == 0 {
        // add img to db if not existing
        conn.execute(
            "INSERT INTO images (path) values (?1)",
            params![img.path.to_str()],
        )?;
    } else {
        // img already exists so clear old hashes (to help prevent stale data)
        conn.execute(
            "DELETE FROM hashes WHERE images_id = (?1)",
            params![img.path.to_str()],
        )?;
    }
    // now update hashes for current image
    conn.execute(
        "INSERT OR REPLACE INTO hashes (kind, hash, images_id)
           values (?1, ?2, (SELECT images_id FROM images WHERE path = ?3))",
        params![img.kind.to_string(), img.hash, img.path.to_str()],
    )?;
    // now save partial_hashes
    match img.kind {
        ImgHashKind::Phash => {
            save_partial_phash(&img, &conn)?;
        },
        _ => {}
    }
    Ok(())
}

fn save_partial_phash(img: &ImgHash, conn: &Connection) -> Result<(), rusqlite::Error> {
    // TODO split up the hash into multiple non-overlapping segments
    let chunk_size = 4;
    let mut chunks: Vec<&str> = vec!["a"; img.hash.len()/chunk_size+1];
    println!("debug chunks={:?}", &chunks);
    // TODO pull this into a util file & write a test for it
    println!("debug img.hash={:?}", &img.hash);
    for (i,_) in img.hash.chars().enumerate() {
        println!("debug i={} j={}", i-i%chunk_size, i+1);
        println!("debug chunk={:?}", &img.hash[i-i%chunk_size..i+1]);
        chunks[i/chunk_size] = &img.hash[i-i%chunk_size..i+1];
    }
    println!("debug chunks={:?}", &chunks);
    // save each in the partial_hashes table w/ it's sequence number
    for (i,chunk) in chunks.iter().enumerate() {
        conn.execute(
            "INSERT OR REPLACE INTO partial_hashes (sequence, part_hash, images_id)
               values (?1, ?2, (SELECT images_id FROM images WHERE path = ?3))",
            params![i, chunk, img.path.to_str()],
        )?;
    }
    Ok(())
}

fn open_db() -> Result<Connection> {
    let db_path = setup_dir();
    let conn = Connection::open(db_path)?;
    setup_db(&conn)?;
    Ok(conn)
}

fn setup_dir() -> PathBuf {
    let proj_dirs = ProjectDirs::from("", "", IDUP_DIR_NAME)
        .expect("Could not determine user data directory");

    let db_path = proj_dirs.data_dir().join(IDUP_DB_NAME);
    let parent = db_path.parent()
        .expect(&format!("Can't determine parent for db_path={:?}", db_path));

    create_dir_all(parent)
        .expect(&format!("Can't create dir parent={:?}", parent));

    db_path
}

fn setup_db(conn: &Connection) -> Result<()> {
    conn.execute_batch("
        BEGIN;
        CREATE TABLE IF NOT EXISTS images (
          images_id INTEGER PRIMARY KEY AUTOINCREMENT,
          path TEXT UNIQUE
        );

        CREATE TABLE IF NOT EXISTS hashes (
          images_id INTEGER,
          kind TEXT,
          hash TEXT,
          PRIMARY KEY (images_id, kind),
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

        COMMIT;",
    )?;

    // TODO update index
    // conn.execute("
    //     CREATE INDEX IF NOT EXISTS hashed_sha256
    //     ON hashes (
    //       sha256
    //     )",
    //     [],
    // )?;

    Ok(())
}
