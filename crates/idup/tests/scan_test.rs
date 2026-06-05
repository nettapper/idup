mod common;

use common::{image_gen, test_env::TestEnv};

/// Smoke test: scan a directory containing 3 unique images plus one exact
/// duplicate (same seed as image A, different filename), then verify that
/// `idup dups` reports exactly one duplicate group.
#[test]
fn scan_detects_exact_duplicate() {
    let env = TestEnv::new();

    // Write 3 unique images (distinct seeds → distinct pixel data → distinct hashes).
    image_gen::write_png(&env.img_dir, "img_a.png", 1);
    image_gen::write_png(&env.img_dir, "img_b.png", 2);
    image_gen::write_png(&env.img_dir, "img_c.png", 3);

    // Write a duplicate of img_a using the same seed.
    image_gen::write_png(&env.img_dir, "img_a_dup.png", 1);

    // Run `idup scan --recursive <img_dir>` — should exit 0.
    env.cmd()
        .args(["scan", "--recursive", env.img_dir.to_str().unwrap()])
        .assert()
        .success();

    // Run `idup dups` — should exit 0 and mention both paths that share a hash.
    let output = env.cmd().arg("dups").assert().success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Both the original and its duplicate must appear in the output.
    assert!(
        stdout.contains("img_a.png"),
        "expected img_a.png in dups output, got:\n{stdout}"
    );
    assert!(
        stdout.contains("img_a_dup.png"),
        "expected img_a_dup.png in dups output, got:\n{stdout}"
    );

    // The unique images must NOT appear (they have no duplicates).
    assert!(
        !stdout.contains("img_b.png"),
        "img_b.png should not appear in dups output, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("img_c.png"),
        "img_c.png should not appear in dups output, got:\n{stdout}"
    );
}

/// Smoke test: scanning an empty directory should succeed with zero images processed.
#[test]
fn scan_empty_dir_succeeds() {
    let env = TestEnv::new();

    env.cmd()
        .args(["scan", "--recursive", env.img_dir.to_str().unwrap()])
        .assert()
        .success();
}
