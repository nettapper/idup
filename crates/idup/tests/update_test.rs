mod common;

use common::{image_gen, test_env::TestEnv};

/// Full lifecycle test:
///
/// 1. Scan a directory containing a duplicate pair + one unique image.
/// 2. Confirm `dups` reports the pair as duplicates.
/// 3. Delete one of the duplicate pair from disk.
/// 4. Run `update --cleanup` → missing file removed from DB.
/// 5. Confirm `dups` now reports no duplicates (orphaned original is unique).
/// 6. Run `clean` (confirmed with "y") → wipe the entire DB.
/// 7. Confirm `random` reports an empty DB.
#[test]
fn scan_update_cleanup_then_clean() {
    let env = TestEnv::new();

    // ── 1. Write images ───────────────────────────────────────────────────
    // img_a and img_a_dup use the same seed → identical pixel data → exact duplicate.
    // img_b is unique.
    image_gen::write_png(&env.img_dir, "img_a.png", 1);
    image_gen::write_png(&env.img_dir, "img_a_dup.png", 1);
    image_gen::write_png(&env.img_dir, "img_b.png", 2);

    // ── 2. Scan ───────────────────────────────────────────────────────────
    env.cmd()
        .args(["scan", "--recursive", env.img_dir.to_str().unwrap()])
        .assert()
        .success();

    // ── 3. Confirm duplicates are present ─────────────────────────────────
    let out = env.cmd().arg("dups").assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("img_a.png"),
        "expected img_a.png in dups before update, got:\n{stdout}"
    );
    assert!(
        stdout.contains("img_a_dup.png"),
        "expected img_a_dup.png in dups before update, got:\n{stdout}"
    );

    // ── 4. Delete one half of the duplicate pair from disk ────────────────
    std::fs::remove_file(env.img_dir.join("img_a_dup.png"))
        .expect("failed to delete img_a_dup.png");

    // ── 5. update --cleanup → missing file should be removed from DB ──────
    let out = env.cmd()
        .args(["update", "--cleanup"])
        .assert()
        .success();

    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();

    // The missing file is reported on stderr.
    assert!(
        stderr.contains("img_a_dup.png"),
        "expected img_a_dup.png in update stderr (file not found), got:\n{stderr}"
    );
    // Summary counts on stdout.
    assert!(
        stdout.contains("Missing:  1"),
        "expected 'Missing: 1' in update stdout, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Cleaned:  1"),
        "expected 'Cleaned: 1' in update stdout, got:\n{stdout}"
    );

    // ── 6. dups → no duplicates (img_a.png is now the sole owner of its hash) ──
    let out = env.cmd().arg("dups").assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("No duplicates found."),
        "expected 'No duplicates found.' after update --cleanup, got:\n{stdout}"
    );

    // ── 7. clean → wipe the entire DB ─────────────────────────────────────
    let out = env.cmd()
        .arg("clean")
        .write_stdin("y\n")
        .assert()
        .success();

    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("Database wiped."),
        "expected 'Database wiped.' after clean, got:\n{stdout}"
    );

    // ── 8. random → DB is now empty ───────────────────────────────────────
    let out = env.cmd().args(["random", "10"]).assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("No images in db."),
        "expected 'No images in db.' after clean, got:\n{stdout}"
    );
}
