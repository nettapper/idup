use axum::{
    extract::{Query, RawQuery, State},
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

    crate::scan::process_path(path, recursive, &opts, &pool).await;

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
        r#"<div class="result-success">Scan complete for <code>{}</code>{}</div>"#,
        esc(&form.path),
        suffix,
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
                        let gallery_href = gallery_href_for_hash(hash);
                        html.push_str(&format!(
                            r#"<div class="group"><h3><a href="{gallery_href}" target="_blank" class="group-link">Group {num}</a></h3><ul>"#
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

// ── Compare ───────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CompareForm {
    img1: String,
    img2: String,
}

pub async fn compare(Form(form): Form<CompareForm>) -> Html<String> {
    let path1 = PathBuf::from(&form.img1);
    let path2 = PathBuf::from(&form.img2);

    let h1 = match crate::hash::phash::hash_path(&path1) {
        Ok(h) => h,
        Err(e) => {
            return Html(err_html(&format!("img1: {e}")));
        }
    };
    let h2 = match crate::hash::phash::hash_path(&path2) {
        Ok(h) => h,
        Err(e) => {
            return Html(err_html(&format!("img2: {e}")));
        }
    };

    let ph1 = h1.hash.clone();
    let ph2 = h2.hash.clone();

    let dist_html = match crate::hash::hamming_dist(h1, h2) {
        Ok(dist) => {
            let label = match dist {
                0 => "Identical",
                1..=5 => "Very similar",
                6..=10 => "Similar",
                _ => "Different",
            };
            format!(
                "<p><strong>Hamming distance:</strong> <code>{dist}</code> — {label}</p>"
            )
        }
        Err(e) => format!(
            r#"<p class="result-error">Distance error: {}</p>"#,
            esc(&e.to_string())
        ),
    };

    Html(format!(
        r#"<div class="compare-result">
  <p><strong>img1 pHash:</strong> <code>{ph1}</code></p>
  <p><strong>img2 pHash:</strong> <code>{ph2}</code></p>
  {dist_html}
</div>"#
    ))
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

    match crate::db::random_images(&pool, n, filter).await {
        Err(e) => Html(err_html(&e.to_string())),
        Ok(data) if data.is_empty() => {
            Html(r#"<p class="muted">No images in db.</p>"#.to_string())
        }
        Ok(data) => {
            let paths: Vec<String> = data.iter().map(|d| d.path.clone()).collect();
            let gallery_href = gallery_href_for_paths(&paths);
            let count = paths.len();

            let mut html = format!(
                r#"<div class="random-header"><a href="{gallery_href}" target="_blank" class="view-btn">View all {count} in browser</a></div>"#
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

// ── Gallery ───────────────────────────────────────────────────────────────────

/// Accepts either `?hash=<group_hash>` or repeated `?path=<abs_path>` params.
/// Uses RawQuery instead of Query<T> because serde_urlencoded does not support
/// Vec<String> from repeated keys (fails when only a single value is present).
pub async fn gallery(
    State(pool): State<SqlitePool>,
    RawQuery(raw): RawQuery,
) -> Html<String> {
    let query_str = raw.unwrap_or_default();
    let mut hash: Option<String> = None;
    let mut dir: Option<String> = None;
    let mut path_params: Vec<String> = Vec::new();

    for pair in query_str.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            let value = percent_decode(v);
            match k {
                "hash" => hash = Some(value),
                "dir" => dir = Some(value),
                "path" => path_params.push(value),
                _ => {}
            }
        }
    }

    let paths: Vec<String> = if let Some(ref h) = hash {
        match crate::db::images_for_group(&pool, h).await {
            Ok(data) => data.into_iter().map(|d| d.path).collect(),
            Err(e) => return Html(simple_error_page(&e.to_string())),
        }
    } else if let Some(ref d) = dir {
        match crate::db::images_in_dir(&pool, d).await {
            Ok(data) => data.into_iter().map(|d| d.path).collect(),
            Err(e) => return Html(simple_error_page(&e.to_string())),
        }
    } else {
        path_params
    };

    if paths.is_empty() {
        return Html(simple_error_page("No images found for this query."));
    }

    Html(build_gallery_page(&paths))
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

// ── Gallery page builder ──────────────────────────────────────────────────────

fn build_gallery_page(paths: &[String]) -> String {
    let count = paths.len();
    let title = format!("Gallery — {count} image{}", if count == 1 { "" } else { "s" });

    let mut cards = String::new();
    for p in paths {
        let filename = Path::new(p)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| p.clone());
        let dir = Path::new(p)
            .parent()
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_default();
        let img_src = format!("/api/image?path={}", url_encode(p));
        cards.push_str(&format!(
            r#"<div class="card" data-path="{data_path}" data-filename="{data_filename}" data-dir="{data_dir}" data-img-src="{data_img_src}">
  <div class="card-img">
    <img src="{img_src_html}" loading="lazy" alt="{alt}" />
  </div>
  <p class="name" title="{path_title}">{filename_html}</p>
</div>"#,
            data_path = esc(p),
            data_filename = esc(&filename),
            data_dir = esc(&dir),
            data_img_src = esc(&img_src),
            img_src_html = esc(&img_src),
            alt = esc(&filename),
            path_title = esc(p),
            filename_html = esc(&filename),
        ));
    }

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>{title} | idup</title>
  <style>
    *, *::before, *::after {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      font-family: ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, Consolas, monospace;
      background: #0f1117;
      color: #e2e8f0;
      min-height: 100vh;
    }}
    header {{
      background: #1a1d27;
      border-bottom: 1px solid #2d3148;
      padding: 1rem 1.5rem;
      display: flex;
      align-items: baseline;
      gap: 1rem;
    }}
    header .logo {{
      font-size: 1.1rem;
      font-weight: 700;
      color: #7c6af7;
      letter-spacing: 0.05em;
    }}
    header h1 {{
      font-size: 0.9rem;
      font-weight: 500;
      color: #94a3b8;
    }}
    header h1 span {{ color: #a78bfa; }}
    .grid {{
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
      gap: 1rem;
      padding: 1.5rem;
    }}
    .card {{
      background: #1a1d27;
      border: 1px solid #2d3148;
      border-radius: 8px;
      overflow: hidden;
      display: flex;
      flex-direction: column;
      cursor: pointer;
      transition: border-color 0.15s;
    }}
    .card:hover {{ border-color: #7c6af7; }}
    .card-img {{
      display: block;
      aspect-ratio: 1;
      overflow: hidden;
      background: #0f1117;
    }}
    .card img {{
      width: 100%;
      height: 100%;
      object-fit: contain;
      display: block;
      transition: opacity 0.2s;
    }}
    .card:hover img {{ opacity: 0.85; }}
    .card .name {{
      padding: 0.4rem 0.6rem;
      font-size: 0.72rem;
      color: #94a3b8;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
      border-top: 1px solid #2d3148;
    }}
    .empty {{
      padding: 3rem;
      color: #64748b;
      font-size: 0.875rem;
    }}
    /* ── Modal ── */
    .modal-overlay {{
      display: none;
      position: fixed;
      inset: 0;
      background: rgba(0,0,0,0.82);
      z-index: 100;
      align-items: center;
      justify-content: center;
    }}
    .modal-overlay.open {{ display: flex; }}
    .modal-box {{
      background: #1a1d27;
      border: 1px solid #2d3148;
      border-radius: 10px;
      max-width: min(92vw, 860px);
      width: 100%;
      max-height: 92vh;
      display: flex;
      flex-direction: column;
      overflow: hidden;
    }}
    .modal-header {{
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 0.75rem 1rem;
      border-bottom: 1px solid #2d3148;
      gap: 0.75rem;
    }}
    .modal-title {{
      font-size: 0.8rem;
      color: #94a3b8;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      text-decoration: none;
    }}
    .modal-title:hover {{
      color: #a78bfa;
      text-decoration: underline;
    }}
    .modal-close {{
      background: none;
      border: none;
      color: #64748b;
      font-size: 1.1rem;
      cursor: pointer;
      line-height: 1;
      padding: 0.2rem 0.4rem;
      border-radius: 4px;
      flex-shrink: 0;
    }}
    .modal-close:hover {{ color: #e2e8f0; background: #2d3148; }}
    .modal-img-area {{
      flex: 1;
      overflow: hidden;
      display: flex;
      align-items: center;
      justify-content: center;
      background: #0f1117;
      min-height: 0;
    }}
    .modal-img-area img {{
      max-width: 100%;
      max-height: 100%;
      object-fit: contain;
      display: block;
    }}
    .modal-footer {{
      padding: 0.75rem 1rem;
      border-top: 1px solid #2d3148;
      display: flex;
      flex-wrap: wrap;
      gap: 0.5rem;
      align-items: center;
    }}
    .modal-btn {{
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
    }}
    .modal-btn:hover {{ background: #7c6af7; color: #fff; }}
    .modal-btn.secondary {{
      border-color: #2d3148;
      color: #64748b;
    }}
    .modal-btn.secondary:hover {{ background: #2d3148; color: #e2e8f0; }}
  </style>
</head>
<body>
  <header>
    <div class="logo">idup</div>
    <h1><span>{title}</span></h1>
  </header>
  <div class="grid">
    {cards}
  </div>

  <!-- Modal overlay -->
  <div class="modal-overlay" id="modal" role="dialog" aria-modal="true">
    <div class="modal-box">
      <div class="modal-header">
        <a class="modal-title" id="modal-title" href="#" target="_blank" title="Browse siblings"></a>
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

  <script>
    const modal      = document.getElementById('modal');
    const modalImg   = document.getElementById('modal-img');
    const modalTitle = document.getElementById('modal-title');
    const modalOpen  = document.getElementById('modal-open');

    function openModal(card) {{
      const path   = card.dataset.path;
      const dir    = card.dataset.dir;
      const imgSrc = card.dataset.imgSrc;

      modalImg.src           = imgSrc;
      modalImg.alt           = path;
      modalTitle.textContent = path;
      modalTitle.href        = '/gallery?dir=' + encodeURIComponent(dir);
      modalOpen.href         = imgSrc;
      modal.classList.add('open');
      history.pushState({{ modal: true }}, '');
    }}

    function closeModal(fromPopstate) {{
      modal.classList.remove('open');
      modalImg.src = '';
      if (!fromPopstate) history.back();
    }}

    window.addEventListener('popstate', () => {{
      if (modal.classList.contains('open')) closeModal(true);
    }});

    document.querySelectorAll('.card').forEach(card => {{
      card.addEventListener('click', () => openModal(card));
    }});

    document.getElementById('modal-close').addEventListener('click', () => closeModal(false));

    modal.addEventListener('click', e => {{
      if (e.target === modal) closeModal(false);
    }});

    document.addEventListener('keydown', e => {{
      if (e.key === 'Escape' && modal.classList.contains('open')) closeModal(false);
    }});
  </script>
</body>
</html>"##,
        title = esc(&title),
        cards = cards,
    )
}

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

/// Decode a percent-encoded URL query-parameter value.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push((hi << 4 | lo) as char);
                i += 3;
                continue;
            }
        }
        // `+` is sometimes used to encode a space in query strings.
        if bytes[i] == b'+' {
            out.push(' ');
        } else {
            out.push(bytes[i] as char);
        }
        i += 1;
    }
    out
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
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

/// Build a /gallery URL for a group hash.
fn gallery_href_for_hash(hash: &str) -> String {
    format!("/gallery?hash={}", url_encode(hash))
}

/// Build a /gallery URL from a slice of absolute paths (repeated `path` params).
fn gallery_href_for_paths(paths: &[String]) -> String {
    let qs: String = paths
        .iter()
        .map(|p| format!("path={}", url_encode(p)))
        .collect::<Vec<_>>()
        .join("&");
    format!("/gallery?{qs}")
}
