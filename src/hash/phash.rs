use super::ImgHash;
use super::ImgHashKind;
use image::{DynamicImage, ImageError, ImageReader};
use std::path::Path;

pub fn hash_path(path: &Path) -> Result<ImgHash, ImageError> {
    let img = ImageReader::open(path)?.with_guessed_format()?.decode()?;
    Ok(ImgHash {
        path: path.to_path_buf(),
        kind: ImgHashKind::Phash,
        hash: hash(img).to_string(),
    })
}

pub fn hash(img: DynamicImage) -> u64 {
    // trace!("original dimensions {:?}", img.dimensions());
    // trace!("original color {:?}", img.color());

    let img = img
        .resize_exact(8, 8, image::imageops::FilterType::Gaussian)
        .into_luma8();
    let (w, h) = img.dimensions();
    // trace!("new dimensions {:?}", (w, h));

    let mut total: u32 = 0;
    for p in img.iter() {
        total = total.checked_add(*p as u32).expect("overflow when calculating total");
    }
    // trace!("total {:?}", total);

    let avg = total / (w * h);
    // trace!("average {:?}", avg);

    assert!(w * h == 64);
    // SAFETY this will overflow if width * height is anything over 64 bits
    let mut average_hash: u64 = 0;
    for p in img.iter() {
        // trace!("{} {:#b}", *p, average_hash);
        if *p as u32 > avg {
            average_hash += 1;
        }
        average_hash <<= 1; // shift one from least to most significant
    }
    average_hash
}
