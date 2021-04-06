extern crate image;

use image::GenericImageView;

fn main() {
    println!("Hello, world!");

    // Use the open function to load an image from a Path.
    // `open` returns a `DynamicImage` on success.
    let img = image::open("test.jpg").expect("test image could not be opened");

    println!("original dimensions {:?}", img.dimensions());
    println!("original color {:?}", img.color());

    let img = img.resize(8, 8, image::imageops::FilterType::Gaussian).into_luma8();
    let (w, h) = img.dimensions();
    println!("new dimensions {:?}", (w, h));

    let mut total: u32 = 0;
    for p in img.iter() {
        total = total.checked_add(*p as u32).expect("overflow when calculating total");
    }
    println!("total {:?}", total);

    let avg = total / (w * h);
    println!("average {:?}", avg);

    assert!(w * h == 64);
    // SAFETY this will overflow if width * height is anything over 64 bits
    let mut average_hash: u64 = 0;
    for p in img.iter() {
        // println!("{} {:#b}", *p, average_hash);
        if *p as u32 > avg {
            average_hash += 1;
        }
        average_hash <<= 1;  // shift one from least to most signifigant
    }
    println!("{}", average_hash);

    // img.save("out.jpg").expect("could not save new image");
}
