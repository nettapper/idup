use crate::error::AppError;
use sqlx::SqlitePool;

pub struct ExploreData {
    pub title: String,
    pub dir: Option<String>,
    pub filter: Option<String>,
    pub subdirs: Vec<String>,
    pub paths: Vec<String>,
}

pub async fn fetch_explore_data(
    pool: &SqlitePool,
    dir: Option<&str>,
    filter: Option<&str>,
    hash: Option<&str>,
    seed: Option<u64>,
    n: u32,
) -> Result<ExploreData, AppError> {
    if let Some(h) = hash {
        let paths = idup::db::images_for_group(pool, h)
            .await?
            .into_iter()
            .map(|d| d.path)
            .collect::<Vec<_>>();
        let count = paths.len();
        let title = format!(
            "Explore — {count} image{} (dup group)",
            if count == 1 { "" } else { "s" }
        );
        return Ok(ExploreData {
            title,
            dir: None,
            filter: None,
            subdirs: Vec::new(),
            paths,
        });
    }

    if let Some(seed_val) = seed {
        let paths = idup::db::random_images_seeded(pool, n, filter, seed_val)
            .await?
            .into_iter()
            .map(|d| d.path)
            .collect::<Vec<_>>();
        let count = paths.len();
        let title = format!(
            "Explore — {count} random image{}",
            if count == 1 { "" } else { "s" }
        );
        return Ok(ExploreData {
            title,
            dir: None,
            filter: filter.map(|f| f.to_string()),
            subdirs: Vec::new(),
            paths,
        });
    }

    if let Some(f) = filter {
        let paths = idup::db::images_matching_filter_in_dir(pool, dir, f)
            .await?
            .into_iter()
            .map(|d| d.path)
            .collect::<Vec<_>>();
        let title = if let Some(d) = dir {
            format!("Explore — {} in {}", f, d)
        } else {
            format!("Explore — {}", f)
        };
        return Ok(ExploreData {
            title,
            dir: dir.map(|d| d.to_string()),
            filter: Some(f.to_string()),
            subdirs: Vec::new(),
            paths,
        });
    }

    if let Some(d) = dir {
        let (subdirs_res, images_res) = tokio::join!(
            idup::db::subdirs_in_dir(pool, d),
            idup::db::images_in_dir(pool, d),
        );
        let subdirs = subdirs_res?;
        let paths = images_res?
            .into_iter()
            .map(|d| d.path)
            .collect::<Vec<_>>();
        let title = format!("Explore — {d}");
        return Ok(ExploreData {
            title,
            dir: Some(d.to_string()),
            filter: None,
            subdirs,
            paths,
        });
    }

    // No params: default to "/" so the top-level directories are shown
    let d = "/";
    let (subdirs_res, images_res) = tokio::join!(
        idup::db::subdirs_in_dir(pool, d),
        idup::db::images_in_dir(pool, d),
    );
    let subdirs = subdirs_res?;
    let paths = images_res?
        .into_iter()
        .map(|d| d.path)
        .collect::<Vec<_>>();
    Ok(ExploreData {
        title: "Explore".to_string(),
        dir: Some(d.to_string()),
        filter: None,
        subdirs,
        paths,
    })
}
