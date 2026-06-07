use axum::{
    extract::{Query, State},
    response::Html,
    Form, Json,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

// ── Static assets ─────────────────────────────────────────────────────────────

pub async fn index() -> Html<&'static str> {
    Html(include_str!("../assets/index.html"))
}

pub async fn style() -> impl axum::response::IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/css")],
        include_str!("../assets/style.css"),
    )
}

// ── Scan ──────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ScanForm {
    path: String,
    /// Present as "true" when the checkbox is checked; absent otherwise.
    recursive: Option<String>,
    unzip: Option<String>,
    remove_archive: Option<String>,
    /// Preset: sha256 base + rotations, no flips, no phash
    exact: Option<String>,
    /// Preset: all hash variants
    all: Option<String>,
    /// Individual: include phash
    phash: Option<String>,
    /// Individual: include sha256 rotations
    rotations: Option<String>,
    /// Individual: include sha256 flip variants
    flips: Option<String>,
}

pub async fn scan(
    State(pool): State<SqlitePool>,
    Form(form): Form<ScanForm>,
) -> Html<String> {
    let path = PathBuf::from(&form.path);
    let recursive = form.recursive.is_some();

    let mut opts = if form.all.is_some() {
        idup::scan::ScanOptions::all()
    } else if form.exact.is_some() {
        idup::scan::ScanOptions::exact()
    } else {
        idup::scan::ScanOptions {
            rotations: form.rotations.is_some(),
            flips: form.flips.is_some(),
            phash: form.phash.is_some(),
            unzip: false,
            remove_archive: false,
        }
    };
    opts.unzip = form.unzip.is_some();
    opts.remove_archive = form.remove_archive.is_some();

    let stats = idup::scan::process_path(path, recursive, &opts, &pool).await;

    let mut notes: Vec<&str> = Vec::new();
    if recursive { notes.push("recursive"); }
    if opts.unzip { notes.push("unzip"); }
    if opts.remove_archive { notes.push("remove-archive"); }
    if opts.rotations { notes.push("rotations"); }
    if opts.flips { notes.push("flips"); }
    if opts.phash { notes.push("phash"); }
    let suffix = if notes.is_empty() {
        String::new()
    } else {
        format!(" ({})", notes.join(", "))
    };

    Html(format!(
        r#"<div class="result-success">
            Scan complete for <code>{}</code>{}
            <p class="muted" style="font-size:0.85rem;margin-top:1rem">{} files processed in {:.2}s</p>
        </div>"#,
        esc(&form.path),
        suffix,
        stats.processed,
        stats.elapsed_secs,
    ))
}

// ── Extract ───────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ExtractForm {
    video: String,
    interval: Option<f64>,
    interval_mode: Option<String>,
    start: Option<String>,
    stop: Option<String>,
    output: Option<String>,
    mkdir: Option<String>,
    force: Option<String>,
}

pub async fn extract(Form(form): Form<ExtractForm>) -> Html<String> {
    let video_path = PathBuf::from(form.video.trim());
    if form.video.trim().is_empty() {
        return Html(err_html("Video file path is required"));
    }

    let interval = form.interval.unwrap_or(1.0);
    if interval <= 0.0 {
        return Html(err_html("Interval must be greater than 0"));
    }

    let interval_mode = match form.interval_mode.as_deref() {
        Some("frame") => ivid::extract::IntervalMode::Frame,
        _ => ivid::extract::IntervalMode::Time,
    };

    let start_secs = if let Some(ref s) = form.start.filter(|s| !s.trim().is_empty()) {
        match ivid::time::parse_hhmmss(s) {
            Ok(secs) => Some(secs),
            Err(e) => return Html(err_html(&format!("Invalid start time: {e}"))),
        }
    } else {
        None
    };

    let stop_secs = if let Some(ref s) = form.stop.filter(|s| !s.trim().is_empty()) {
        match ivid::time::parse_hhmmss(s) {
            Ok(secs) => Some(secs),
            Err(e) => return Html(err_html(&format!("Invalid stop time: {e}"))),
        }
    } else {
        None
    };

    let video_stem = video_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");

    let output_dir = if let Some(ref out) = form.output.filter(|o| !o.trim().is_empty()) {
        PathBuf::from(out.trim())
    } else {
        PathBuf::from(format!("ivid_{video_stem}"))
    };

    let mkdir = form.mkdir.is_some();
    let force = form.force.is_some();

    let config = ivid::extract::ExtractConfig {
        video: video_path,
        output_dir,
        interval,
        interval_mode,
        start: start_secs,
        stop: stop_secs,
        force,
        mkdir,
    };

    match tokio::task::spawn_blocking(move || ivid::extract::run_extraction(&config)).await {
        Ok(Ok(result)) => {
            let output_dir_str = result.output_dir.to_string_lossy().into_owned();
            let encoded_path = url_encode(&output_dir_str);
            
            Html(format!(
                r#"<div class="result-success">
                    Extraction complete
                    <p class="muted" style="font-size:0.85rem;margin:1rem 0">
                        {} frames extracted in {:.2}s<br>
                        Output: <code>{}</code>
                    </p>
                    <a class="view-btn" href="/?panel=scan&path={}&recursive=true" style="display:inline-block;text-decoration:none">
                        Scan these frames
                    </a>
                </div>"#,
                result.frame_count,
                result.elapsed_secs,
                esc(&output_dir_str),
                encoded_path,
            ))
        }
        Ok(Err(e)) => Html(err_html(&e)),
        Err(e) => Html(err_html(&format!("Internal task execution error: {e}"))),
    }
}

// ── Update ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UpdateForm {
    path: Option<String>,
    /// Present as "true" when the checkbox is checked; absent otherwise.
    cleanup: Option<String>,
}

pub async fn update(
    State(pool): State<SqlitePool>,
    Form(form): Form<UpdateForm>,
) -> Html<String> {
    let path = form.path.filter(|p| !p.trim().is_empty()).map(PathBuf::from);
    let cleanup = form.cleanup.is_some();

    let stats = idup::update::process_update(path, cleanup, &pool).await;

    // Build result HTML with formatted statistics
    let cleaned_row = if cleanup {
        format!(
            r#"<tr><td style="text-align:right;padding-right:1rem">Cleaned:</td><td><code>{}</code></td></tr>"#,
            stats.cleaned
        )
    } else {
        String::new()
    };

    Html(format!(
        r#"<div class="result-success">
            Update complete
            <table style="margin-top:1rem;font-size:0.95rem;line-height:1.8">
                <tr>
                    <td style="text-align:right;padding-right:1rem">Verified:</td>
                    <td><code>{}</code></td>
                </tr>
                <tr>
                    <td style="text-align:right;padding-right:1rem">Updated:</td>
                    <td><code>{}</code></td>
                </tr>
                <tr>
                    <td style="text-align:right;padding-right:1rem">Missing:</td>
                    <td><code>{}</code></td>
                </tr>
                {cleaned_row}
            </table>
            <p class="muted" style="font-size:0.85rem;margin-top:1rem">{} files checked in {:.2}s</p>
        </div>"#,
        stats.verified, stats.updated, stats.missing, stats.total, stats.elapsed_secs
    ))
}

// ── Clean ─────────────────────────────────────────────────────────────────────

pub async fn clean(State(pool): State<SqlitePool>) -> Html<String> {
    match idup::db::wipe_db(&pool).await {
        Ok(()) => Html(
            r#"<div class="result-success">Database wiped. All images and hashes have been deleted.</div>"#
                .to_string(),
        ),
        Err(e) => Html(err_html(&e.to_string())),
    }
}

// ── List ──────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ListQuery {
    path: Option<String>,
}

pub async fn list(
    State(pool): State<SqlitePool>,
    Query(params): Query<ListQuery>,
) -> Html<String> {
    // Treat an empty string the same as absent.
    let path = params.path.filter(|p| !p.trim().is_empty());

    match path {
        None => match idup::db::exact_matches_grouped(&pool).await {
            Err(e) => Html(err_html(&e.to_string())),
            Ok(data) if data.is_empty() => {
                Html(r#"<p class="muted">No duplicates found.</p>"#.to_string())
            }
            Ok(data) => {
                let mut html = String::from(r#"<div class="groups">"#);
                let mut current_hash = String::new();
                let mut group_num = 0usize;
                let mut group_paths: Vec<String> = Vec::new();

                let flush_group =
                    |html: &mut String, num: usize, hash: &str, paths: &[String]| {
                        let explore_href = explore_href_for_hash(hash);
                        html.push_str(&format!(
                            r#"<div class="group"><h3><a href="{explore_href}" target="_blank" class="group-link">Group {num}</a></h3><div class="dup-list">"#
                        ));
                        for p in paths {
                            let encoded = url_encode(p);
                            html.push_str(&format!(
                                r#"<div class="dup-item">
                                    <img src="/api/image?path={}" class="dup-thumb" loading="lazy" />
                                    <code>{}</code>
                                </div>"#,
                                encoded,
                                esc(p)
                            ));
                        }
                        html.push_str("</div></div>");
                    };

                for item in &data {
                    if item.group_hash != current_hash {
                        if group_num > 0 {
                            flush_group(&mut html, group_num, &current_hash, &group_paths);
                        }
                        group_num += 1;
                        current_hash = item.group_hash.clone();
                        group_paths.clear();
                    }
                    group_paths.push(item.path.clone());
                }
                if group_num > 0 {
                    flush_group(&mut html, group_num, &current_hash, &group_paths);
                }
                html.push_str("</div>");
                Html(html)
            }
        },

        Some(path_str) => {
            let path = PathBuf::from(&path_str);
            match idup::db::exact_match(&pool, &path).await {
                Err(e) => Html(err_html(&e.to_string())),
                Ok(data) if data.is_empty() => {
                    Html(r#"<p class="muted">No duplicates found.</p>"#.to_string())
                }
                Ok(data) => {
                    let mut html =
                        String::from(r#"<div class="group"><div class="dup-list">"#);
                    for item in &data {
                        let encoded = url_encode(&item.path);
                        html.push_str(&format!(
                            r#"<div class="dup-item">
                                <img src="/api/image?path={}" class="dup-thumb" loading="lazy" />
                                <code>{}</code>
                            </div>"#,
                            encoded,
                            esc(&item.path)
                        ));
                    }
                    html.push_str("</div></div>");
                    Html(html)
                }
            }
        }
    }
}

// ── Info ──────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct InfoQuery {
    file: String,
}

pub async fn info(Query(params): Query<InfoQuery>) -> Html<String> {
    let path = PathBuf::from(&params.file);
    let mut html = String::from(r#"<div class="info-result">"#);

    match idup::hash::phash::hash_path(&path) {
        Ok(ph) => html.push_str(&format!(
            "<p><strong>pHash:</strong> <code>{}</code></p>",
            esc(&ph.hash)
        )),
        Err(e) => html.push_str(&format!(
            r#"<p class="result-error">pHash error: {}</p>"#,
            esc(&e.to_string())
        )),
    }

    match idup::hash::sha256::hash_path(&path) {
        Ok(sh) => html.push_str(&format!(
            "<p><strong>SHA-256:</strong> <code>{}</code></p>",
            esc(&sh.hash)
        )),
        Err(e) => html.push_str(&format!(
            r#"<p class="result-error">SHA-256 error: {}</p>"#,
            esc(&e.to_string())
        )),
    }

    html.push_str("</div>");
    Html(html)
}

// ── Random ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RandomQuery {
    n: Option<u32>,
    filter: Option<String>,
}

pub async fn random(
    State(pool): State<SqlitePool>,
    Query(params): Query<RandomQuery>,
) -> Html<String> {
    let n = params.n.unwrap_or(20);
    let filter = params.filter.as_deref().filter(|s| !s.trim().is_empty());

    // Generate a seed so /explore can reproduce this exact random selection.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(42);

    match idup::db::random_images_seeded(&pool, n, filter, seed).await {
        Err(e) => Html(err_html(&e.to_string())),
        Ok(data) if data.is_empty() => {
            Html(r#"<p class="muted">No images in db.</p>"#.to_string())
        }
        Ok(data) => {
            let paths: Vec<String> = data.iter().map(|d| d.path.clone()).collect();
            let explore_href = explore_href_seeded(seed, n, filter);
            let count = paths.len();

            let mut html = format!(
                r#"<div class="random-header"><a href="{explore_href}" target="_blank" class="view-btn">View all {count} in browser</a></div>"#
            );
            html.push_str(r#"<ul class="random-list">"#);
            for p in &paths {
                html.push_str(&format!("<li><code>{}</code></li>", esc(p)));
            }
            html.push_str("</ul>");
            Html(html)
        }
    }
}

// ── Explore ───────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ExploreQuery {
    dir: Option<String>,
    filter: Option<String>,
    hash: Option<String>,
    seed: Option<u64>,
    n: Option<u32>,
}

pub async fn explore(
    State(pool): State<SqlitePool>,
    Query(params): Query<ExploreQuery>,
) -> Html<String> {
    let dir = params.dir.as_deref().filter(|s| !s.trim().is_empty());
    let filter = params.filter.as_deref().filter(|s| !s.trim().is_empty());
    let hash = params.hash.as_deref().filter(|s| !s.trim().is_empty());
    let seed = params.seed;
    let n = params.n.unwrap_or(20);

    if let Some(h) = hash {
        let paths = match idup::db::images_for_group(&pool, h).await {
            Ok(data) => data.into_iter().map(|d| d.path).collect::<Vec<_>>(),
            Err(e) => return Html(simple_error_page(&e.to_string())),
        };
        let count = paths.len();
        let title = format!(
            "Explore — {count} image{} (dup group)",
            if count == 1 { "" } else { "s" }
        );
        return Html(build_explore_page(&title, None, None, &[], &paths));
    }

    if let Some(seed_val) = seed {
        let paths = match idup::db::random_images_seeded(&pool, n, filter, seed_val).await {
            Ok(data) => data.into_iter().map(|d| d.path).collect::<Vec<_>>(),
            Err(e) => return Html(simple_error_page(&e.to_string())),
        };
        let count = paths.len();
        let title = format!(
            "Explore — {count} random image{}",
            if count == 1 { "" } else { "s" }
        );
        return Html(build_explore_page(&title, None, filter, &[], &paths));
    }

    if let Some(f) = filter {
        let paths = match idup::db::images_matching_filter_in_dir(&pool, dir, f).await {
            Ok(data) => data.into_iter().map(|d| d.path).collect::<Vec<_>>(),
            Err(e) => return Html(simple_error_page(&e.to_string())),
        };
        let title = if let Some(d) = dir {
            format!("Explore — {} in {}", f, d)
        } else {
            format!("Explore — {}", f)
        };
        return Html(build_explore_page(&title, dir, Some(f), &[], &paths));
    }

    if let Some(d) = dir {
        let (subdirs_res, images_res) = tokio::join!(
            idup::db::subdirs_in_dir(&pool, d),
            idup::db::images_in_dir(&pool, d),
        );
        let subdirs = match subdirs_res {
            Ok(s) => s,
            Err(e) => return Html(simple_error_page(&e.to_string())),
        };
        let paths = match images_res {
            Ok(p) => p.into_iter().map(|d| d.path).collect::<Vec<_>>(),
            Err(e) => return Html(simple_error_page(&e.to_string())),
        };
        let title = format!("Explore — {d}");
        return Html(build_explore_page(&title, Some(d), None, &subdirs, &paths));
    }

    // No params: default to "/" so the top-level directories are shown
    let d = "/";
    let (subdirs_res, images_res) = tokio::join!(
        idup::db::subdirs_in_dir(&pool, d),
        idup::db::images_in_dir(&pool, d),
    );
    let subdirs = match subdirs_res {
        Ok(s) => s,
        Err(e) => return Html(simple_error_page(&e.to_string())),
    };
    let paths = match images_res {
        Ok(p) => p.into_iter().map(|d| d.path).collect::<Vec<_>>(),
        Err(e) => return Html(simple_error_page(&e.to_string())),
    };
    Html(build_explore_page("Explore", Some(d), None, &subdirs, &paths))
}

// ── Image File ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ImageQuery {
    path: String,
}

pub async fn image_file(
    State(pool): State<SqlitePool>,
    Query(params): Query<ImageQuery>,
) -> axum::response::Response {
    use axum::body::Body;
    use axum::http::{header, StatusCode};

    // Gate: only serve paths tracked in the database.
    match idup::db::path_exists_in_db(&pool, &params.path).await {
        Ok(true) => {}
        Ok(false) => {
            return axum::response::Response::builder()
                .status(StatusCode::FORBIDDEN)
                .header(header::CONTENT_TYPE, "text/plain")
                .body(Body::from("Path not in database"))
                .unwrap();
        }
        Err(e) => {
            return axum::response::Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "text/plain")
                .body(Body::from(format!("DB error: {e}")))
                .unwrap();
        }
    }

    // Read the file from disk.
    let bytes = match tokio::fs::read(&params.path).await {
        Ok(b) => b,
        Err(_) => {
            return axum::response::Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(header::CONTENT_TYPE, "text/plain")
                .body(Body::from("File not found"))
                .unwrap();
        }
    };

    // Detect MIME type via magic bytes.
    let mime = infer::get(&bytes)
        .map(|t| t.mime_type())
        .unwrap_or("application/octet-stream");

    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "no-store, no-cache, must-revalidate")
        .body(Body::from(bytes))
        .unwrap()
}

// ── Explore page builder ──────────────────────────────────────────────────────

fn build_explore_page(
    title: &str,
    dir: Option<&str>,
    filter: Option<&str>,
    subdirs: &[String],
    paths: &[String],
) -> String {
    let dir_value = dir.map(esc).unwrap_or_default();
    let filter_value = filter.map(esc).unwrap_or_default();
    let breadcrumb_html = dir.map(|d| build_breadcrumb_html(d)).unwrap_or_default();
    let content_html = build_explore_content(dir, filter, subdirs, paths);

    let mut out = String::with_capacity(32_768);
    out.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
    out.push_str("  <meta charset=\"UTF-8\" />\n");
    out.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />\n");
    out.push_str(&format!("  <title>{} | idup</title>\n", esc(title)));
    out.push_str("  <style>");
    out.push_str(EXPLORE_CSS);
    out.push_str("  </style>\n</head>\n<body>\n");
    out.push_str("  <header>\n    <div class=\"logo\">idup</div>\n");
    out.push_str("    <form class=\"filter-bar\" method=\"get\" action=\"/explore\">\n");
    out.push_str(&format!(
        "      <input type=\"text\" name=\"dir\" placeholder=\"Directory path...\" value=\"{}\" />\n",
        dir_value
    ));
    out.push_str(&format!(
        "      <input type=\"text\" name=\"filter\" placeholder=\"Glob filter (e.g. *.jpg)\" value=\"{}\" />\n",
        filter_value
    ));
    out.push_str("      <button type=\"submit\">Go</button>\n    </form>\n  </header>\n");
    out.push_str(&breadcrumb_html);
    out.push_str(&content_html);
    out.push_str(EXPLORE_MODAL_HTML);
    out.push_str(EXPLORE_SCRIPT);
    out.push_str("</body>\n</html>\n");
    out
}

fn build_breadcrumb_html(dir: &str) -> String {
    let parts: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    let mut html = String::from("  <nav class=\"breadcrumb\">");
    if parts.is_empty() {
        // At filesystem root — show "root" as the current (non-linked) crumb
        html.push_str("<span class=\"current\">root</span>");
    } else {
        html.push_str("<a href=\"/explore\">root</a>");
        let mut cumulative = String::new();
        for (i, part) in parts.iter().enumerate() {
            cumulative.push('/');
            cumulative.push_str(part);
            html.push_str("<span class=\"sep\">/</span>");
            if i + 1 == parts.len() {
                html.push_str(&format!("<span class=\"current\">{}</span>", esc(part)));
            } else {
                let href = format!("/explore?dir={}", url_encode(&cumulative));
                html.push_str(&format!("<a href=\"{}\">{}</a>", esc(&href), esc(part)));
            }
        }
    }
    html.push_str("</nav>\n");
    html
}

fn build_explore_content(
    dir: Option<&str>,
    filter: Option<&str>,
    subdirs: &[String],
    paths: &[String],
) -> String {
    let mut out = String::new();

    if subdirs.is_empty() && paths.is_empty() {
        let msg = if dir.is_none() && filter.is_none() {
            "Enter a directory path or glob filter above to start exploring."
        } else {
            "No images found."
        };
        out.push_str(&format!("  <div class=\"empty\">{}</div>\n", msg));
        return out;
    }

    let show_headers = !subdirs.is_empty() && !paths.is_empty();

    if !subdirs.is_empty() {
        out.push_str("  <div class=\"section\">\n");
        if show_headers {
            out.push_str(&format!(
                "    <p class=\"section-title\">Folders <span class=\"count\">({})</span></p>\n",
                subdirs.len()
            ));
        }
        out.push_str("    <div class=\"grid\">\n");
        for d in subdirs {
            let name = Path::new(d)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| d.clone());
            let href = esc(&format!("/explore?dir={}", url_encode(d)));
            out.push_str(&format!(
                "      <a href=\"{href}\" class=\"dir-card\"><div class=\"dir-body\"><span class=\"dir-symbol\">dir</span></div><p class=\"name\" title=\"{full}\">{name}</p></a>\n",
                href = href,
                full = esc(d),
                name = esc(&name),
            ));
        }
        out.push_str("    </div>\n  </div>\n");
    }

    if !paths.is_empty() {
        out.push_str("  <div class=\"section\">\n");
        if show_headers {
            out.push_str(&format!(
                "    <p class=\"section-title\">Images <span class=\"count\">({})</span></p>\n",
                paths.len()
            ));
        }
        out.push_str("    <div class=\"grid\">\n");
        for p in paths {
            let filename = Path::new(p)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.clone());
            let parent_dir = Path::new(p)
                .parent()
                .map(|d| d.to_string_lossy().to_string())
                .unwrap_or_default();
            let img_src = format!("/api/image?path={}", url_encode(p));
            out.push_str(&format!(
                "      <div class=\"card\" data-path=\"{dp}\" data-dir=\"{dd}\" data-img-src=\"{ds}\"><div class=\"card-img\"><img src=\"{src}\" loading=\"lazy\" alt=\"{alt}\" /></div><p class=\"name\" title=\"{pt}\">{fn_}</p></div>\n",
                dp  = esc(p),
                dd  = esc(&parent_dir),
                ds  = esc(&img_src),
                src = esc(&img_src),
                alt = esc(&filename),
                pt  = esc(p),
                fn_ = esc(&filename),
            ));
        }
        out.push_str("    </div>\n  </div>\n");
    }

    out
}

const EXPLORE_CSS: &str = r#"
    *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, Consolas, monospace;
      background: #0f1117;
      color: #e2e8f0;
      min-height: 100vh;
    }
    header {
      background: #1a1d27;
      border-bottom: 1px solid #2d3148;
      padding: 0.75rem 1.5rem;
      display: flex;
      align-items: center;
      gap: 1rem;
      flex-wrap: wrap;
    }
    .logo {
      font-size: 1.1rem;
      font-weight: 700;
      color: #7c6af7;
      letter-spacing: 0.05em;
      white-space: nowrap;
    }
    .filter-bar {
      display: flex;
      gap: 0.5rem;
      align-items: center;
      flex: 1;
      flex-wrap: wrap;
    }
    .filter-bar input {
      background: #0f1117;
      border: 1px solid #2d3148;
      border-radius: 4px;
      color: #e2e8f0;
      font-family: inherit;
      font-size: 0.82rem;
      padding: 0.3rem 0.6rem;
    }
    .filter-bar input:focus { outline: none; border-color: #7c6af7; }
    .filter-bar input[name="dir"] { flex: 2; min-width: 160px; }
    .filter-bar input[name="filter"] { flex: 1; min-width: 120px; }
    .filter-bar button {
      background: #7c6af7;
      border: none;
      border-radius: 4px;
      color: #fff;
      cursor: pointer;
      font-family: inherit;
      font-size: 0.82rem;
      padding: 0.3rem 0.8rem;
      white-space: nowrap;
    }
    .filter-bar button:hover { background: #6b5ee0; }
    .breadcrumb {
      background: #1a1d27;
      border-bottom: 1px solid #2d3148;
      padding: 0.5rem 1.5rem;
      font-size: 0.8rem;
      color: #64748b;
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0.15rem;
    }
    .breadcrumb a { color: #a78bfa; text-decoration: none; }
    .breadcrumb a:hover { text-decoration: underline; }
    .breadcrumb .sep { color: #2d3148; padding: 0 0.1rem; }
    .breadcrumb .current { color: #e2e8f0; }
    .section { padding: 1.5rem; }
    .section + .section { padding-top: 0; }
    .section-title {
      font-size: 0.78rem;
      color: #64748b;
      text-transform: uppercase;
      letter-spacing: 0.08em;
      margin-bottom: 0.75rem;
    }
    .section-title .count { color: #475569; font-weight: normal; }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
      gap: 0.75rem;
    }
    .card {
      background: #1a1d27;
      border: 1px solid #2d3148;
      border-radius: 8px;
      overflow: hidden;
      display: flex;
      flex-direction: column;
      cursor: pointer;
      transition: border-color 0.15s;
    }
    .card:hover { border-color: #7c6af7; }
    .card-img {
      display: block;
      aspect-ratio: 1;
      overflow: hidden;
      background: #0f1117;
    }
    .card img {
      width: 100%;
      height: 100%;
      object-fit: contain;
      display: block;
      transition: opacity 0.2s;
    }
    .card:hover img { opacity: 0.85; }
    .card .name, .dir-card .name {
      padding: 0.4rem 0.6rem;
      font-size: 0.72rem;
      color: #94a3b8;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
      border-top: 1px solid #2d3148;
    }
    .dir-card {
      background: #1a1d27;
      border: 1px solid #2d3148;
      border-radius: 8px;
      overflow: hidden;
      display: flex;
      flex-direction: column;
      cursor: pointer;
      transition: border-color 0.15s;
      text-decoration: none;
      color: inherit;
    }
    .dir-card:hover { border-color: #7c6af7; }
    .dir-body {
      flex: 1;
      display: flex;
      align-items: center;
      justify-content: center;
      padding: 1.5rem 0.75rem;
      background: #151821;
    }
    .dir-symbol {
      font-size: 0.7rem;
      color: #475569;
      border: 1px solid #2d3148;
      border-radius: 3px;
      padding: 0.15rem 0.4rem;
    }
    .empty {
      padding: 3rem 1.5rem;
      color: #64748b;
      font-size: 0.875rem;
    }
    .modal-overlay {
      display: none;
      position: fixed;
      inset: 0;
      background: rgba(0,0,0,0.82);
      z-index: 100;
      align-items: center;
      justify-content: center;
    }
    .modal-overlay.open { display: flex; }
    .modal-box {
      background: #1a1d27;
      border: 1px solid #2d3148;
      border-radius: 10px;
      width: 90vw;
      height: 90vh;
      display: flex;
      flex-direction: column;
      overflow: hidden;
    }
    .modal-header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 0.75rem 1rem;
      border-bottom: 1px solid #2d3148;
      gap: 0.75rem;
    }
    .modal-title {
      font-size: 0.8rem;
      color: #94a3b8;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      text-decoration: none;
    }
    .modal-title:hover { color: #a78bfa; text-decoration: underline; }
    .modal-close {
      background: none;
      border: none;
      color: #64748b;
      font-size: 1.1rem;
      cursor: pointer;
      line-height: 1;
      padding: 0.2rem 0.4rem;
      border-radius: 4px;
      flex-shrink: 0;
    }
    .modal-close:hover { color: #e2e8f0; background: #2d3148; }
    .modal-img-area {
      flex: 1;
      overflow: hidden;
      display: flex;
      align-items: center;
      justify-content: center;
      background: #0f1117;
      min-height: 0;
    }
    .modal-img-area img {
      max-width: 100%;
      max-height: 100%;
      object-fit: contain;
      display: block;
    }
    .modal-footer {
      padding: 0.75rem 1rem;
      border-top: 1px solid #2d3148;
      display: flex;
      flex-wrap: wrap;
      gap: 0.5rem;
      align-items: center;
    }
    .modal-btn {
      font-family: inherit;
      font-size: 0.78rem;
      padding: 0.35rem 0.75rem;
      border-radius: 5px;
      border: 1px solid #7c6af7;
      background: transparent;
      color: #a78bfa;
      cursor: pointer;
      text-decoration: none;
      white-space: nowrap;
      transition: background 0.15s, color 0.15s;
    }
    .modal-btn:hover { background: #7c6af7; color: #fff; }
    .modal-btn.secondary { border-color: #2d3148; color: #64748b; }
    .modal-btn.secondary:hover { background: #2d3148; color: #e2e8f0; }

    /* Crop Mode styles */
    .crop-workspace {
      display: none;
      flex-direction: column;
      width: 100%;
      height: 100%;
      min-height: 0;
      position: relative;
    }
    .modal-overlay.crop-mode #modal-img { display: none; }
    .modal-overlay.crop-mode .crop-workspace { display: flex; }
    
    .crop-controls {
      display: flex;
      align-items: center;
      gap: 1rem;
      width: 100%;
      padding: 0.5rem 1rem;
      background: #151821;
      border-bottom: 1px solid #2d3148;
      font-size: 0.78rem;
      flex-wrap: wrap;
    }
    .crop-control-group {
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }
    .crop-control-group label {
      color: #94a3b8;
    }
    .crop-control-group input[type="range"] {
      width: 120px;
      accent-color: #7c6af7;
    }
    .crop-control-group input[type="number"], .crop-control-group input[type="text"] {
      background: #0f1117;
      border: 1px solid #2d3148;
      border-radius: 4px;
      color: #e2e8f0;
      padding: 0.2rem 0.4rem;
      font-family: inherit;
      font-size: 0.75rem;
    }
    .crop-canvas-container {
      flex: 1;
      display: flex;
      align-items: center;
      justify-content: center;
      width: 100%;
      height: 100%;
      position: relative;
      overflow: hidden;
      background: #0f1117;
      min-height: 0;
      padding: 1.5rem;
    }
    #crop-canvas {
      display: block;
      cursor: crosshair;
      box-shadow: 0 4px 20px rgba(0,0,0,0.5);
      max-width: 100%;
      max-height: 100%;
      object-fit: contain;
    }
    .crop-footer-controls {
      display: flex;
      justify-content: space-between;
      align-items: center;
      width: 100%;
      gap: 1rem;
      flex-wrap: wrap;
    }
    .crop-save-dialog {
      display: flex;
      align-items: center;
      gap: 0.75rem;
      flex-wrap: wrap;
      font-size: 0.78rem;
    }
    .crop-save-dialog label {
      display: flex;
      align-items: center;
      gap: 0.3rem;
      cursor: pointer;
      color: #94a3b8;
    }
    .crop-save-dialog input[type="text"] {
      background: #0f1117;
      border: 1px solid #2d3148;
      border-radius: 4px;
      color: #e2e8f0;
      padding: 0.2rem 0.4rem;
      font-family: inherit;
      font-size: 0.75rem;
      width: 180px;
    }
    .crop-footer-actions {
      display: flex;
      gap: 0.5rem;
    }
"#;

const EXPLORE_MODAL_HTML: &str = r##"  <div class="modal-overlay" id="modal" role="dialog" aria-modal="true">
    <div class="modal-box">
      <div class="modal-header">
        <a class="modal-title" id="modal-title" href="#" target="_blank" title="Browse directory"></a>
        <button class="modal-close" id="modal-close" aria-label="Close">&#x2715;</button>
      </div>
      <div class="modal-img-area">
        <img id="modal-img" src="" alt="" />
        <div class="crop-workspace">
          <div class="crop-controls">
            <div class="crop-control-group">
              <label for="rotate-slider">Rotate:</label>
              <input type="range" id="rotate-slider" min="-180" max="180" step="0.5" value="0" />
              <input type="number" id="rotate-num" min="-180" max="180" step="0.5" value="0" style="width: 65px;" />
              <span>&deg;</span>
            </div>
            <button class="modal-btn secondary" id="reset-crop-btn" style="padding: 0.2rem 0.5rem; font-size: 0.75rem;">Reset</button>
          </div>
          <div class="crop-canvas-container">
            <canvas id="crop-canvas"></canvas>
          </div>
        </div>
      </div>
      <div class="modal-footer">
        <div class="viewer-footer-controls" id="viewer-footer-controls" style="display: flex; gap: 0.5rem; align-items: center; width: 100%;">
          <a id="modal-open" href="#" target="_blank" class="modal-btn secondary">Open image</a>
          <button id="modal-crop-btn" class="modal-btn">Crop &amp; Rotate</button>
        </div>
        <div class="crop-footer-controls" id="crop-footer-controls" style="display: none;">
          <div class="crop-save-dialog">
            <label>
              <input type="checkbox" id="crop-overwrite" checked />
              Overwrite original
            </label>
            <div id="crop-filename-group" style="display: none; align-items: center; gap: 0.3rem;">
              <label for="crop-filename">New name:</label>
              <input type="text" id="crop-filename" placeholder="image_cropped.png" />
            </div>
          </div>
          <div class="crop-footer-actions">
            <button id="crop-cancel-btn" class="modal-btn secondary">Cancel</button>
            <button id="crop-save-btn" class="modal-btn">Save Crop</button>
          </div>
        </div>
      </div>
    </div>
  </div>
"##;

const EXPLORE_SCRIPT: &str = r#"  <script>
    const modal      = document.getElementById('modal');
    const modalImg   = document.getElementById('modal-img');
    const modalTitle = document.getElementById('modal-title');
    const modalOpen  = document.getElementById('modal-open');
    const imgArea    = document.querySelector('.modal-img-area');

    const cards = Array.from(document.querySelectorAll('.card'));
    let currentCardIndex = -1;

    // Crop Editor Elements
    const cropBtn              = document.getElementById('modal-crop-btn');
    const cropWorkspace        = document.querySelector('.crop-workspace');
    const viewerFooterControls = document.getElementById('viewer-footer-controls');
    const cropFooterControls   = document.getElementById('crop-footer-controls');
    const cropCanvas           = document.getElementById('crop-canvas');
    const rotateSlider         = document.getElementById('rotate-slider');
    const rotateNum            = document.getElementById('rotate-num');
    const resetCropBtn         = document.getElementById('reset-crop-btn');
    const cropOverwrite        = document.getElementById('crop-overwrite');
    const cropFilenameGroup    = document.getElementById('crop-filename-group');
    const cropFilename         = document.getElementById('crop-filename');
    const cropCancelBtn        = document.getElementById('crop-cancel-btn');
    const cropSaveBtn          = document.getElementById('crop-save-btn');

    let cropImg = new Image();
    let cropRotation = 0;
    let cropBox = null;
    const cropPad = 50;
    let cropDragState = { active: false, type: null, handle: null, startX: 0, startY: 0, boxStartX: 0, boxStartY: 0, boxStartW: 0, boxStartH: 0 };

    function updateModalContent() {
      if (currentCardIndex < 0 || currentCardIndex >= cards.length) return;
      const card = cards[currentCardIndex];
      const path   = card.dataset.path;
      const dir    = card.dataset.dir;
      const imgSrc = card.dataset.imgSrc;

      modalImg.src           = imgSrc;
      modalImg.alt           = path;
      modalTitle.textContent = path;
      modalTitle.href        = '/explore?dir=' + encodeURIComponent(dir);
      modalOpen.href         = imgSrc;
    }

    function showNext() {
      if (cards.length === 0) return;
      currentCardIndex = (currentCardIndex + 1) % cards.length;
      updateModalContent();
    }

    function showPrev() {
      if (cards.length === 0) return;
      currentCardIndex = (currentCardIndex - 1 + cards.length) % cards.length;
      updateModalContent();
    }

    function openModal(card) {
      currentCardIndex = cards.indexOf(card);
      updateModalContent();
      if (!modal.classList.contains('open')) {
        modal.classList.add('open');
        history.pushState({ modal: true }, '');
      }
    }

    function exitCropMode() {
      modal.classList.remove('crop-mode');
      viewerFooterControls.style.display = 'flex';
      cropFooterControls.style.display = 'none';
    }

    function closeModal(fromPopstate) {
      exitCropMode();
      modal.classList.remove('open');
      modalImg.src = '';
      currentCardIndex = -1;
      if (!fromPopstate) history.back();
    }

    window.addEventListener('popstate', () => {
      if (modal.classList.contains('open')) closeModal(true);
    });

    document.querySelectorAll('.card').forEach(card => {
      card.addEventListener('click', () => openModal(card));
    });

    document.getElementById('modal-close').addEventListener('click', () => closeModal(false));

    modal.addEventListener('click', e => {
      if (e.target === modal) closeModal(false);
    });

    document.addEventListener('keydown', e => {
      if (!modal.classList.contains('open')) return;
      if (e.key === 'Escape') {
        closeModal(false);
      } else if (e.key === 'ArrowRight') {
        if (modal.classList.contains('crop-mode')) return;
        showNext();
      } else if (e.key === 'ArrowLeft') {
        if (modal.classList.contains('crop-mode')) return;
        showPrev();
      }
    });

    imgArea.addEventListener('mousemove', e => {
      if (modal.classList.contains('crop-mode')) return;
      const rect = imgArea.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const width = rect.width;
      if (x < width * 0.25) {
        imgArea.style.cursor = 'w-resize';
      } else if (x > width * 0.75) {
        imgArea.style.cursor = 'e-resize';
      } else {
        imgArea.style.cursor = 'default';
      }
    });

    imgArea.addEventListener('click', e => {
      if (modal.classList.contains('crop-mode')) return;
      const rect = imgArea.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const width = rect.width;
      if (x < width * 0.25) {
        showPrev();
      } else if (x > width * 0.75) {
        showNext();
      }
    });

    // ── Crop Editor interactive logic ─────────────────────────────────────────

    function initCropEditor() {
      cropRotation = 0;
      rotateSlider.value = 0;
      rotateNum.value = 0;
      cropBox = null;
      
      const card = cards[currentCardIndex];
      const path = card.dataset.path;
      
      cropOverwrite.checked = true;
      cropFilenameGroup.style.display = 'none';
      
      const filename = path.split('/').pop();
      const dotIndex = filename.lastIndexOf('.');
      const stem = dotIndex !== -1 ? filename.substring(0, dotIndex) : filename;
      const ext = dotIndex !== -1 ? filename.substring(dotIndex + 1) : 'png';
      cropFilename.value = stem + '_cropped.' + ext;
      
      cropImg.src = modalImg.src;
      cropImg.onload = () => {
        // Wait for DOM layout to settle and size computations to be correct
        setTimeout(drawCropCanvas, 150);
      };
    }

    function drawCropCanvas() {
      if (!cropImg.complete || cropImg.naturalWidth === 0) return;
      
      const ctx = cropCanvas.getContext('2d');
      const w_orig = cropImg.naturalWidth;
      const h_orig = cropImg.naturalHeight;
      
      const alpha = cropRotation * Math.PI / 180.0;
      const cos = Math.abs(Math.cos(alpha));
      const sin = Math.abs(Math.sin(alpha));
      const w_rot = w_orig * cos + h_orig * sin;
      const h_rot = w_orig * sin + h_orig * cos;
      
      const w_work = w_rot + cropPad * 2;
      const h_work = h_rot + cropPad * 2;
      
      const container = document.querySelector('.crop-canvas-container');
      const rect = container.getBoundingClientRect();
      const w_max = Math.max(10, rect.width - 48) || 600;
      const h_max = Math.max(10, rect.height - 48) || 400;
      
      const scale = Math.min(w_max / w_work, h_max / h_work, 1.0);
      
      cropCanvas.width = w_work * scale;
      cropCanvas.height = h_work * scale;
      
      const imgScale = scale;
      const cx_canvas = cropCanvas.width / 2;
      const cy_canvas = cropCanvas.height / 2;
      
      if (!cropBox) {
        const bw = w_rot * imgScale * 0.8;
        const bh = h_rot * imgScale * 0.8;
        cropBox = {
          x: cx_canvas - bw / 2,
          y: cy_canvas - bh / 2,
          w: bw,
          h: bh
        };
      }
      
      // Checkerboard background
      ctx.fillStyle = '#1e2230';
      ctx.fillRect(0, 0, cropCanvas.width, cropCanvas.height);
      ctx.fillStyle = '#151821';
      const chSize = 12;
      for (let y = 0; y < cropCanvas.height; y += chSize * 2) {
        for (let x = 0; x < cropCanvas.width; x += chSize * 2) {
          ctx.fillRect(x, y, chSize, chSize);
          ctx.fillRect(x + chSize, y + chSize, chSize, chSize);
        }
      }
      
      // Draw rotated image
      ctx.save();
      ctx.translate(cx_canvas, cy_canvas);
      ctx.rotate(alpha);
      ctx.drawImage(cropImg, -w_orig * imgScale / 2, -h_orig * imgScale / 2, w_orig * imgScale, h_orig * imgScale);
      ctx.restore();
      
      // Dark overlay
      ctx.fillStyle = 'rgba(0, 0, 0, 0.6)';
      ctx.fillRect(0, 0, cropCanvas.width, cropCanvas.height);
      
      // Redraw inside crop box
      ctx.save();
      ctx.beginPath();
      ctx.rect(cropBox.x, cropBox.y, cropBox.w, cropBox.h);
      ctx.clip();
      ctx.translate(cx_canvas, cy_canvas);
      ctx.rotate(alpha);
      ctx.drawImage(cropImg, -w_orig * imgScale / 2, -h_orig * imgScale / 2, w_orig * imgScale, h_orig * imgScale);
      ctx.restore();
      
      // Crop border
      ctx.strokeStyle = '#7c6af7';
      ctx.lineWidth = 2;
      ctx.strokeRect(cropBox.x, cropBox.y, cropBox.w, cropBox.h);
      
      // Dashed grids
      ctx.strokeStyle = 'rgba(255, 255, 255, 0.4)';
      ctx.lineWidth = 1;
      ctx.setLineDash([4, 4]);
      ctx.beginPath();
      ctx.moveTo(cropBox.x + cropBox.w / 3, cropBox.y);
      ctx.lineTo(cropBox.x + cropBox.w / 3, cropBox.y + cropBox.h);
      ctx.moveTo(cropBox.x + 2 * cropBox.w / 3, cropBox.y);
      ctx.lineTo(cropBox.x + 2 * cropBox.w / 3, cropBox.y + cropBox.h);
      ctx.moveTo(cropBox.x, cropBox.y + cropBox.h / 3);
      ctx.lineTo(cropBox.x + cropBox.w, cropBox.y + cropBox.h / 3);
      ctx.moveTo(cropBox.x, cropBox.y + 2 * cropBox.h / 3);
      ctx.lineTo(cropBox.x + cropBox.w, cropBox.y + 2 * cropBox.h / 3);
      ctx.stroke();
      ctx.setLineDash([]);
      
      // Handles
      const handleSize = 8;
      ctx.fillStyle = '#7c6af7';
      ctx.strokeStyle = '#fff';
      ctx.lineWidth = 1.5;
      
      const handles = {
        tl: { x: cropBox.x, y: cropBox.y },
        tr: { x: cropBox.x + cropBox.w, y: cropBox.y },
        bl: { x: cropBox.x, y: cropBox.y + cropBox.h },
        br: { x: cropBox.x + cropBox.w, y: cropBox.y + cropBox.h }
      };
      
      for (const key in handles) {
        const h = handles[key];
        ctx.fillRect(h.x - handleSize / 2, h.y - handleSize / 2, handleSize, handleSize);
        ctx.strokeRect(h.x - handleSize / 2, h.y - handleSize / 2, handleSize, handleSize);
      }
    }

    function applySnapping(type, handle) {
      if (!cropImg.complete || cropImg.naturalWidth === 0 || !cropBox) return;
      
      const w_orig = cropImg.naturalWidth;
      const h_orig = cropImg.naturalHeight;
      const alpha = cropRotation * Math.PI / 180.0;
      const cos = Math.abs(Math.cos(alpha));
      const sin = Math.abs(Math.sin(alpha));
      const w_rot = w_orig * cos + h_orig * sin;
      const h_rot = w_orig * sin + h_orig * cos;
      
      const w_work = w_rot + cropPad * 2;
      
      const scale = cropCanvas.width / w_work;
      const imgScale = scale;
      const cx_canvas = cropCanvas.width / 2;
      const cy_canvas = cropCanvas.height / 2;
      
      const rx = cx_canvas - (w_rot * imgScale) / 2;
      const ry = cy_canvas - (h_rot * imgScale) / 2;
      const rx_end = rx + w_rot * imgScale;
      const ry_end = ry + h_rot * imgScale;
      
      const snapThresh = 8;
      
      if (type === 'move') {
        if (Math.abs(cropBox.x - rx) < snapThresh) {
          cropBox.x = rx;
        } else if (Math.abs(cropBox.x + cropBox.w - rx_end) < snapThresh) {
          cropBox.x = rx_end - cropBox.w;
        }
        if (Math.abs(cropBox.y - ry) < snapThresh) {
          cropBox.y = ry;
        } else if (Math.abs(cropBox.y + cropBox.h - ry_end) < snapThresh) {
          cropBox.y = ry_end - cropBox.h;
        }
      } else if (type === 'resize') {
        if (handle === 'br') {
          if (Math.abs(cropBox.x + cropBox.w - rx_end) < snapThresh) {
            cropBox.w = rx_end - cropBox.x;
          }
          if (Math.abs(cropBox.y + cropBox.h - ry_end) < snapThresh) {
            cropBox.h = ry_end - cropBox.y;
          }
        } else if (handle === 'tl') {
          const oppositeX = cropBox.x + cropBox.w;
          const oppositeY = cropBox.y + cropBox.h;
          if (Math.abs(cropBox.x - rx) < snapThresh) {
            cropBox.x = rx;
            cropBox.w = oppositeX - rx;
          }
          if (Math.abs(cropBox.y - ry) < snapThresh) {
            cropBox.y = ry;
            cropBox.h = oppositeY - ry;
          }
        } else if (handle === 'tr') {
          const oppositeY = cropBox.y + cropBox.h;
          if (Math.abs(cropBox.x + cropBox.w - rx_end) < snapThresh) {
            cropBox.w = rx_end - cropBox.x;
          }
          if (Math.abs(cropBox.y - ry) < snapThresh) {
            cropBox.y = ry;
            cropBox.h = oppositeY - ry;
          }
        } else if (handle === 'bl') {
          const oppositeX = cropBox.x + cropBox.w;
          if (Math.abs(cropBox.x - rx) < snapThresh) {
            cropBox.x = rx;
            cropBox.w = oppositeX - rx;
          }
          if (Math.abs(cropBox.y + cropBox.h - ry_end) < snapThresh) {
            cropBox.h = ry_end - cropBox.y;
          }
        }
      } else if (type === 'draw') {
        if (Math.abs(cropBox.x - rx) < snapThresh) {
          cropBox.w = cropBox.w + (cropBox.x - rx);
          cropBox.x = rx;
        }
        if (Math.abs(cropBox.y - ry) < snapThresh) {
          cropBox.h = cropBox.h + (cropBox.y - ry);
          cropBox.y = ry;
        }
        if (Math.abs(cropBox.x + cropBox.w - rx_end) < snapThresh) {
          cropBox.w = rx_end - cropBox.x;
        }
        if (Math.abs(cropBox.y + cropBox.h - ry_end) < snapThresh) {
          cropBox.h = ry_end - cropBox.y;
        }
      }
    }

    function getMousePos(e) {
      const rect = cropCanvas.getBoundingClientRect();
      return {
        x: (e.clientX - rect.left) * (cropCanvas.width / rect.width),
        y: (e.clientY - rect.top) * (cropCanvas.height / rect.height)
      };
    }
    
    function getHandleAt(mx, my) {
      const handleSize = 12;
      const handles = {
        tl: { x: cropBox.x, y: cropBox.y },
        tr: { x: cropBox.x + cropBox.w, y: cropBox.y },
        bl: { x: cropBox.x, y: cropBox.y + cropBox.h },
        br: { x: cropBox.x + cropBox.w, y: cropBox.y + cropBox.h }
      };
      for (const key in handles) {
        const h = handles[key];
        if (Math.abs(mx - h.x) <= handleSize && Math.abs(my - h.y) <= handleSize) {
          return key;
        }
      }
      return null;
    }
    
    function isInsideBox(mx, my) {
      return mx >= cropBox.x && mx <= cropBox.x + cropBox.w &&
             my >= cropBox.y && my <= cropBox.y + cropBox.h;
    }
    
    cropCanvas.addEventListener('mousedown', e => {
      e.preventDefault();
      const { x, y } = getMousePos(e);
      const handle = getHandleAt(x, y);
      
      if (handle) {
        cropDragState = {
          active: true,
          type: 'resize',
          handle: handle,
          startX: x,
          startY: y,
          boxStartX: cropBox.x,
          boxStartY: cropBox.y,
          boxStartW: cropBox.w,
          boxStartH: cropBox.h
        };
      } else if (isInsideBox(x, y)) {
        cropDragState = {
          active: true,
          type: 'move',
          startX: x,
          startY: y,
          boxStartX: cropBox.x,
          boxStartY: cropBox.y,
          boxStartW: cropBox.w,
          boxStartH: cropBox.h
        };
      } else {
        cropDragState = {
          active: true,
          type: 'draw',
          startX: x,
          startY: y
        };
        cropBox = { x: x, y: y, w: 1, h: 1 };
      }
    });
    
    cropCanvas.addEventListener('mousemove', e => {
      const { x, y } = getMousePos(e);
      
      if (!cropDragState.active) {
        const handle = getHandleAt(x, y);
        if (handle === 'tl' || handle === 'br') {
          cropCanvas.style.cursor = 'nwse-resize';
        } else if (handle === 'tr' || handle === 'bl') {
          cropCanvas.style.cursor = 'nesw-resize';
        } else if (isInsideBox(x, y)) {
          cropCanvas.style.cursor = 'move';
        } else {
          cropCanvas.style.cursor = 'crosshair';
        }
      }
      
      if (cropDragState.active) {
        if (cropDragState.type === 'move') {
          const dx = x - cropDragState.startX;
          const dy = y - cropDragState.startY;
          cropBox.x = Math.max(0, Math.min(cropCanvas.width - cropBox.w, cropDragState.boxStartX + dx));
          cropBox.y = Math.max(0, Math.min(cropCanvas.height - cropBox.h, cropDragState.boxStartY + dy));
        } else if (cropDragState.type === 'resize') {
          const dx = x - cropDragState.startX;
          const dy = y - cropDragState.startY;
          
          if (cropDragState.handle === 'br') {
            cropBox.w = Math.max(10, Math.min(cropCanvas.width - cropDragState.boxStartX, cropDragState.boxStartW + dx));
            cropBox.h = Math.max(10, Math.min(cropCanvas.height - cropDragState.boxStartY, cropDragState.boxStartH + dy));
          } else if (cropDragState.handle === 'tl') {
            const oppositeX = cropDragState.boxStartX + cropDragState.boxStartW;
            const oppositeY = cropDragState.boxStartY + cropDragState.boxStartH;
            cropBox.x = Math.max(0, Math.min(oppositeX - 10, cropDragState.boxStartX + dx));
            cropBox.y = Math.max(0, Math.min(oppositeY - 10, cropDragState.boxStartY + dy));
            cropBox.w = oppositeX - cropBox.x;
            cropBox.h = oppositeY - cropBox.y;
          } else if (cropDragState.handle === 'tr') {
            const oppositeY = cropDragState.boxStartY + cropDragState.boxStartH;
            cropBox.w = Math.max(10, Math.min(cropCanvas.width - cropDragState.boxStartX, cropDragState.boxStartW + dx));
            cropBox.y = Math.max(0, Math.min(oppositeY - 10, cropDragState.boxStartY + dy));
            cropBox.h = oppositeY - cropBox.y;
          } else if (cropDragState.handle === 'bl') {
            const oppositeX = cropDragState.boxStartX + cropDragState.boxStartW;
            cropBox.x = Math.max(0, Math.min(oppositeX - 10, cropDragState.boxStartX + dx));
            cropBox.w = oppositeX - cropBox.x;
            cropBox.h = Math.max(10, Math.min(cropCanvas.height - cropDragState.boxStartY, cropDragState.boxStartH + dy));
          }
        } else if (cropDragState.type === 'draw') {
          const x0 = Math.min(cropDragState.startX, x);
          const y0 = Math.min(cropDragState.startY, y);
          const w0 = Math.abs(x - cropDragState.startX);
          const h0 = Math.abs(y - cropDragState.startY);
          cropBox = {
            x: Math.max(0, x0),
            y: Math.max(0, y0),
            w: Math.max(10, Math.min(cropCanvas.width - x0, w0)),
            h: Math.max(10, Math.min(cropCanvas.height - y0, h0))
          };
        }
        applySnapping(cropDragState.type, cropDragState.handle);
        drawCropCanvas();
      }
    });
    
    window.addEventListener('mouseup', () => {
      cropDragState.active = false;
    });

    rotateSlider.addEventListener('input', e => {
      cropRotation = parseFloat(e.target.value);
      rotateNum.value = cropRotation;
      drawCropCanvas();
    });
    
    rotateNum.addEventListener('input', e => {
      let val = parseFloat(e.target.value);
      if (isNaN(val)) val = 0;
      val = Math.max(-180, Math.min(180, val));
      cropRotation = val;
      rotateSlider.value = val;
      drawCropCanvas();
    });
    
    resetCropBtn.addEventListener('click', () => {
      cropRotation = 0;
      rotateSlider.value = 0;
      rotateNum.value = 0;
      cropBox = null;
      drawCropCanvas();
    });

    cropOverwrite.addEventListener('change', () => {
      if (cropOverwrite.checked) {
        cropFilenameGroup.style.display = 'none';
      } else {
        cropFilenameGroup.style.display = 'flex';
      }
    });

    cropBtn.addEventListener('click', () => {
      modal.classList.add('crop-mode');
      viewerFooterControls.style.display = 'none';
      cropFooterControls.style.display = 'flex';
      initCropEditor();
    });
    
    cropCancelBtn.addEventListener('click', exitCropMode);

    cropSaveBtn.addEventListener('click', () => {
      if (!cropImg.complete || cropImg.naturalWidth === 0) return;
      
      const card = cards[currentCardIndex];
      const path = card.dataset.path;
      
      const w_orig = cropImg.naturalWidth;
      const h_orig = cropImg.naturalHeight;
      const alpha = cropRotation * Math.PI / 180.0;
      const cos = Math.abs(Math.cos(alpha));
      const sin = Math.abs(Math.sin(alpha));
      const w_rot = w_orig * cos + h_orig * sin;
      
      const w_work = w_rot + cropPad * 2;
      const scale = cropCanvas.width / w_work;
      
      const x_rot = cropBox.x / scale - cropPad;
      const y_rot = cropBox.y / scale - cropPad;
      const w_rot_out = cropBox.w / scale;
      const h_rot_out = cropBox.h / scale;
      
      const finalX = Math.round(x_rot);
      const finalY = Math.round(y_rot);
      const finalW = Math.round(w_rot_out);
      const finalH = Math.round(h_rot_out);
      
      const overwrite = cropOverwrite.checked;
      const newFilename = overwrite ? null : cropFilename.value;
      
      const payload = {
        input_path: path,
        overwrite: overwrite,
        new_filename: newFilename,
        rotate_deg: cropRotation,
        crop_rect: {
          x: finalX,
          y: finalY,
          w: finalW,
          h: finalH
        }
      };
      
      cropSaveBtn.disabled = true;
      cropSaveBtn.textContent = 'Saving...';
      
      fetch('/api/crop', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify(payload)
      })
      .then(res => res.json())
      .then(data => {
        cropSaveBtn.disabled = false;
        cropSaveBtn.textContent = 'Save Crop';
        if (data.success) {
          exitCropMode();
          location.reload();
        } else {
          alert('Failed to crop image: ' + data.message);
        }
      })
      .catch(err => {
        cropSaveBtn.disabled = false;
        cropSaveBtn.textContent = 'Save Crop';
        alert('Error saving crop: ' + err);
      });
    });
  </script>
"#;

fn simple_error_page(msg: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8" /><title>Error | idup</title>
<style>
  body {{ font-family: monospace; background: #0f1117; color: #f87171; padding: 2rem; }}
</style>
</head>
<body><p>{}</p></body>
</html>"#,
        esc(msg)
    )
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn err_html(msg: &str) -> String {
    format!(r#"<div class="result-error">{}</div>"#, esc(msg))
}

/// Percent-encode characters that are not safe as a URL query-parameter value.
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for b in s.bytes() {
        match b {
            // Unreserved characters: leave as-is
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
            | b'-' | b'_' | b'.' | b'~' | b'/' => out.push(b as char),
            // Everything else: percent-encode
            _ => {
                out.push('%');
                out.push(char::from_digit((b >> 4) as u32, 16).unwrap().to_ascii_uppercase());
                out.push(char::from_digit((b & 0xf) as u32, 16).unwrap().to_ascii_uppercase());
            }
        }
    }
    out
}

/// Build an /explore URL for a duplicate group hash.
fn explore_href_for_hash(hash: &str) -> String {
    format!("/explore?hash={}", url_encode(hash))
}

/// Build an /explore URL for a seeded random result.
fn explore_href_seeded(seed: u64, n: u32, filter: Option<&str>) -> String {
    let mut href = format!("/explore?seed={}&n={}", seed, n);
    if let Some(f) = filter {
        href.push_str(&format!("&filter={}", url_encode(f)));
    }
    href
}

// ── Crop handler ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CropRect {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
}

#[derive(Deserialize)]
pub struct CropRequest {
    input_path: String,
    overwrite: bool,
    new_filename: Option<String>,
    rotate_deg: Option<f64>,
    crop_rect: Option<CropRect>,
}

#[derive(Serialize)]
pub struct CropResponse {
    success: bool,
    message: String,
    new_path: Option<String>,
    img_src: Option<String>,
}

pub async fn crop(
    State(pool): State<SqlitePool>,
    Json(req): Json<CropRequest>,
) -> Json<CropResponse> {
    let in_path = Path::new(&req.input_path);
    if !in_path.exists() {
        return Json(CropResponse {
            success: false,
            message: format!("Input file does not exist: {}", req.input_path),
            new_path: None,
            img_src: None,
        });
    }

    let out_path = if req.overwrite {
        in_path.to_path_buf()
    } else {
        let parent = in_path.parent().unwrap_or_else(|| Path::new("."));
        let name = match &req.new_filename {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => {
                let stem = in_path.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
                let ext = in_path.extension().and_then(|e| e.to_str()).unwrap_or("png");
                format!("{}_cropped.{}", stem, ext)
            }
        };
        parent.join(name)
    };

    let crop_rect_tuple = req.crop_rect.map(|r| (r.x, r.y, r.w, r.h));

    // Call icrop library to rotate and crop
    if let Err(err) = icrop::rotate_and_crop(in_path, &out_path, crop_rect_tuple, req.rotate_deg) {
        return Json(CropResponse {
            success: false,
            message: format!("Image processing failed: {}", err),
            new_path: None,
            img_src: None,
        });
    }

    // Immediately scan/hash the output path to update the database
    let scan_opts = idup::scan::ScanOptions::all();
    idup::scan::process_path(out_path.clone(), false, &scan_opts, &pool).await;

    let out_path_str = out_path.to_string_lossy().into_owned();
    let img_src = format!("/api/image?path={}", url_encode(&out_path_str));

    Json(CropResponse {
        success: true,
        message: if req.overwrite {
            "Image cropped and updated in-place.".to_string()
        } else {
            format!("Image cropped and saved as new file: {}", out_path.file_name().and_then(|f| f.to_str()).unwrap_or(""))
        },
        new_path: Some(out_path_str),
        img_src: Some(img_src),
    })
}

