use axum::{
    extract::{Query, State},
    response::Html,
    Form,
};
use serde::Deserialize;
use sqlx::SqlitePool;
use std::path::PathBuf;

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
}

pub async fn scan(
    State(pool): State<SqlitePool>,
    Form(form): Form<ScanForm>,
) -> Html<String> {
    let path = PathBuf::from(&form.path);
    let recursive = form.recursive.is_some();

    crate::scan::process_path(path, recursive, &pool).await;

    Html(format!(
        r#"<div class="result-success">Scan complete for <code>{}</code>{}</div>"#,
        esc(&form.path),
        if recursive { " (recursive)" } else { "" }
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

                for item in &data {
                    if item.group_hash != current_hash {
                        if group_num > 0 {
                            html.push_str("</ul></div>");
                        }
                        group_num += 1;
                        html.push_str(&format!(
                            r#"<div class="group"><h3>Group {group_num}</h3><ul>"#
                        ));
                        current_hash = item.group_hash.clone();
                    }
                    html.push_str(&format!(
                        "<li><code>{}</code></li>",
                        esc(&item.path)
                    ));
                }
                if group_num > 0 {
                    html.push_str("</ul></div>");
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
