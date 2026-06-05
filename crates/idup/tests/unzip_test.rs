mod common;

use common::{image_gen, test_env::TestEnv};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

fn create_zip_with_images(
    zip_path: &Path,
    image_data: &[(&str, Vec<u8>)],
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(zip_path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    for (name, data) in image_data {
        zip.start_file(*name, options)?;
        zip.write_all(data)?;
    }

    zip.finish()?;
    Ok(())
}

#[test]
fn scan_unzip_extracts_and_detects_duplicates() {
    let env = TestEnv::new();

    // 1. Generate image bytes
    let temp_dir = tempfile::tempdir().unwrap();
    
    let img_a_path = image_gen::write_png(temp_dir.path(), "img_a.png", 101);
    let img_a_dup_path = image_gen::write_png(temp_dir.path(), "img_a_dup.png", 101);
    let img_b_path = image_gen::write_png(temp_dir.path(), "img_b.png", 202);

    let mut img_a_bytes = Vec::new();
    File::open(&img_a_path).unwrap().read_to_end(&mut img_a_bytes).unwrap();

    let mut img_a_dup_bytes = Vec::new();
    File::open(&img_a_dup_path).unwrap().read_to_end(&mut img_a_dup_bytes).unwrap();

    let mut img_b_bytes = Vec::new();
    File::open(&img_b_path).unwrap().read_to_end(&mut img_b_bytes).unwrap();

    // 2. Write zip file to env.img_dir
    let zip_path = env.img_dir.join("images.zip");
    create_zip_with_images(
        &zip_path,
        &[
            ("img_a.png", img_a_bytes),
            ("img_a_dup.png", img_a_dup_bytes),
            ("img_b.png", img_b_bytes),
        ],
    )
    .expect("failed to create test zip file");

    // Ensure the zip exists and is the only file in the directory initially
    assert!(zip_path.exists());

    // 3. Run scan with --unzip option
    env.cmd()
        .args(["scan", "--unzip", "--recursive", env.img_dir.to_str().unwrap()])
        .assert()
        .success();

    // 4. Verify unzipped sibling directory exists and contains extracted files
    let expected_dir = env.img_dir.join("images");
    assert!(expected_dir.exists(), "extracted sibling directory should exist");
    assert!(expected_dir.is_dir());

    assert!(expected_dir.join("img_a.png").exists());
    assert!(expected_dir.join("img_a_dup.png").exists());
    assert!(expected_dir.join("img_b.png").exists());

    // 5. Run dups command and verify duplicates are detected
    let out = env.cmd().arg("dups").assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();

    assert!(
        stdout.contains("img_a.png"),
        "expected img_a.png in dups output, got:\n{stdout}"
    );
    assert!(
        stdout.contains("img_a_dup.png"),
        "expected img_a_dup.png in dups output, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("img_b.png"),
        "img_b.png should not be in dups output, got:\n{stdout}"
    );
}

#[test]
fn scan_unzip_avoids_collisions() {
    let env = TestEnv::new();
    
    // Create an existing folder 'images' in the image directory
    let existing_dir = env.img_dir.join("images");
    std::fs::create_dir_all(&existing_dir).unwrap();

    // Write a zip file
    let zip_path = env.img_dir.join("images.zip");
    create_zip_with_images(&zip_path, &[]).expect("failed to create zip");

    // Run scan with --unzip
    env.cmd()
        .args(["scan", "--unzip", "--recursive", env.img_dir.to_str().unwrap()])
        .assert()
        .success();

    // Verify it extracted to 'images_1' to avoid collision
    let expected_collided_dir = env.img_dir.join("images_1");
    assert!(expected_collided_dir.exists(), "extracted colliding zip should go to images_1");
    assert!(expected_collided_dir.is_dir());
}

#[test]
fn scan_unzip_handles_single_root_dir() {
    let env = TestEnv::new();

    // 1. Generate image bytes
    let temp_dir = tempfile::tempdir().unwrap();
    let img_a_path = image_gen::write_png(temp_dir.path(), "img_a.png", 101);
    let mut img_a_bytes = Vec::new();
    File::open(&img_a_path).unwrap().read_to_end(&mut img_a_bytes).unwrap();

    // 2. Write zip file where all files are nested under a single root directory "nested_folder"
    let zip_path = env.img_dir.join("images.zip");
    create_zip_with_images(
        &zip_path,
        &[
            ("nested_folder/img_a.png", img_a_bytes),
        ],
    )
    .expect("failed to create test zip file");

    // 3. Run scan with --unzip option
    env.cmd()
        .args(["scan", "--unzip", "--recursive", env.img_dir.to_str().unwrap()])
        .assert()
        .success();

    // 4. Verify unzipped sibling directory exists and contains the file directly (no double nesting)
    let expected_dir = env.img_dir.join("images");
    assert!(expected_dir.exists(), "extracted sibling directory should exist");
    assert!(expected_dir.is_dir());

    // It should exist directly inside "images", i.e. "images/img_a.png" instead of "images/nested_folder/img_a.png"
    let direct_file = expected_dir.join("img_a.png");
    let nested_file = expected_dir.join("nested_folder").join("img_a.png");
    
    assert!(direct_file.exists(), "expected file to be unzipped directly (without double nesting)");
    assert!(!nested_file.exists(), "should not be nested twice under the zip root folder");
}
