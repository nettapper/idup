use axum::{
    extract::{Query, State},
    response::Html,
    Form,
};
use serde::Deserialize;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

// ── Static assets ─────────────────────────────────────────────────────────────

pub async fn index() -> Html<&'static str> {
    Html(include_str!("../../assets/index.html"))
}

pub async fn style() -> impl axum::response::IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/css")],
        include_str!("../../assets/style.css"),
    )
}

// ── Scan ──────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ScanForm {
    path: String,
    /// Present as "true" when the checkbox is checked; absent otherwise.
    recursive: Option<String>,
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

    let opts = if form.all.is_some() {
        crate::scan::ScanOptions::all()
    } else if form.exact.is_some() {
        crate::scan::ScanOptions::exact()
    } else {
        crate::scan::ScanOptions {
            sha256_rotations: form.rotations.is_some(),
            sha256_flips: form.flips.is_some(),
            phash: form.phash.is_some(),
        }
    };

    let stats = crate::scan::process_path(path, recursive, &opts, &pool).await;

    let mut notes: Vec<&str> = Vec::new();
    if recursive { notes.push("recursive"); }
    if opts.sha256_rotations { notes.push("rotations"); }
    if opts.sha256_flips { notes.push("flips"); }
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

    let stats = crate::update::process_update(path, cleanup, &pool).await;

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
    match crate::db::wipe_db(&pool).await {
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
        None => match crate::db::exact_matches_grouped(&pool).await {
            Err(e) => Html(err_html(&e.to_string())),
            Ok(data) if data.is_empty() => {
                Html(r#"<p class="muted">No duplicates found.</p>"#.to_string())
            }
            Ok(data) => {
                let mut html = String::from(r#"<div class="groups">"#);
                let mut current_hash = String::new();
                let mut group_num = 0usize;
                // Accumulate paths per group so we can build the gallery link
                let mut group_paths: Vec<String> = Vec::new();

                // Helper closure to flush a completed group
                let flush_group =
                    |html: &mut String, num: usize, hash: &str, paths: &[String]| {
                        let explore_href = explore_href_for_hash(hash);
                        html.push_str(&format!(
                            r#"<div class="group"><h3><a href="{explore_href}" target="_blank" class="group-link">Group {num}</a></h3><ul>"#
                        ));
                        for p in paths {
                            html.push_str(&format!(
                                "<li><code>{}</code></li>",
                                esc(p)
                            ));
                        }
                        html.push_str("</ul></div>");
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
            match crate::db::exact_match(&pool, &path).await {
                Err(e) => Html(err_html(&e.to_string())),
                Ok(data) if data.is_empty() => {
                    Html(r#"<p class="muted">No duplicates found.</p>"#.to_string())
                }
                Ok(data) => {
                    let mut html =
                        String::from(r#"<div class="group"><ul>"#);
                    for item in &data {
                        html.push_str(&format!(
                            "<li><code>{}</code></li>",
                            esc(&item.path)
                        ));
                    }
                    html.push_str("</ul></div>");
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

    match crate::hash::phash::hash_path(&path) {
        Ok(ph) => html.push_str(&format!(
            "<p><strong>pHash:</strong> <code>{}</code></p>",
            esc(&ph.hash)
        )),
        Err(e) => html.push_str(&format!(
            r#"<p class="result-error">pHash error: {}</p>"#,
            esc(&e.to_string())
        )),
    }

    match crate::hash::sha256::hash_path(&path) {
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

    match crate::db::random_images_seeded(&pool, n, filter, seed).await {
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
        let paths = match crate::db::images_for_group(&pool, h).await {
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
        let paths = match crate::db::random_images_seeded(&pool, n, filter, seed_val).await {
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
        let paths = match crate::db::images_matching_filter_in_dir(&pool, dir, f).await {
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
            crate::db::subdirs_in_dir(&pool, d),
            crate::db::images_in_dir(&pool, d),
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
        crate::db::subdirs_in_dir(&pool, d),
        crate::db::images_in_dir(&pool, d),
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
    match crate::db::path_exists_in_db(&pool, &params.path).await {
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
        .header(header::CACHE_CONTROL, "public, max-age=3600")
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
      max-width: min(92vw, 860px);
      width: 100%;
      max-height: 92vh;
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
"#;

const EXPLORE_MODAL_HTML: &str = r##"  <div class="modal-overlay" id="modal" role="dialog" aria-modal="true">
    <div class="modal-box">
      <div class="modal-header">
        <a class="modal-title" id="modal-title" href="#" target="_blank" title="Browse directory"></a>
        <button class="modal-close" id="modal-close" aria-label="Close">&#x2715;</button>
      </div>
      <div class="modal-img-area">
        <img id="modal-img" src="" alt="" />
      </div>
      <div class="modal-footer">
        <a id="modal-open" href="#" target="_blank" class="modal-btn secondary">Open image</a>
      </div>
    </div>
  </div>
"##;

const EXPLORE_SCRIPT: &str = r#"  <script>
    const modal      = document.getElementById('modal');
    const modalImg   = document.getElementById('modal-img');
    const modalTitle = document.getElementById('modal-title');
    const modalOpen  = document.getElementById('modal-open');

    function openModal(card) {
      const path   = card.dataset.path;
      const dir    = card.dataset.dir;
      const imgSrc = card.dataset.imgSrc;

      modalImg.src           = imgSrc;
      modalImg.alt           = path;
      modalTitle.textContent = path;
      modalTitle.href        = '/explore?dir=' + encodeURIComponent(dir);
      modalOpen.href         = imgSrc;
      modal.classList.add('open');
      history.pushState({ modal: true }, '');
    }

    function closeModal(fromPopstate) {
      modal.classList.remove('open');
      modalImg.src = '';
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
      if (e.key === 'Escape' && modal.classList.contains('open')) closeModal(false);
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
