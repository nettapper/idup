use std::f64::consts::PI;
use std::path::Path;
use image::{DynamicImage, GenericImageView, ImageResult, Rgba, RgbaImage};

/// Rotates an image by an arbitrary angle in degrees clockwise around its center.
/// The resulting image dimensions are calculated to fit the entire rotated image.
/// Out-of-bounds/empty pixels are padded with solid black.
pub fn rotate_image(img: &DynamicImage, degrees: f64) -> RgbaImage {
    let radians = degrees * PI / 180.0;
    let cos_t = radians.cos();
    let sin_t = radians.sin();

    let (w_in, h_in) = img.dimensions();
    let cx_in = w_in as f64 / 2.0;
    let cy_in = h_in as f64 / 2.0;

    // Compute rotated corners to find the new bounding box
    let corners = [
        (0.0 - cx_in, 0.0 - cy_in),
        (w_in as f64 - cx_in, 0.0 - cy_in),
        (0.0 - cx_in, h_in as f64 - cy_in),
        (w_in as f64 - cx_in, h_in as f64 - cy_in),
    ];

    let mut min_x = f64::MAX;
    let mut max_x = f64::MIN;
    let mut min_y = f64::MAX;
    let mut max_y = f64::MIN;

    for &(x, y) in &corners {
        let rx = x * cos_t - y * sin_t;
        let ry = x * sin_t + y * cos_t;
        if rx < min_x { min_x = rx; }
        if rx > max_x { max_x = rx; }
        if ry < min_y { min_y = ry; }
        if ry > max_y { max_y = ry; }
    }

    let w_out = ((max_x - min_x) - 1e-9).ceil() as u32;
    let h_out = ((max_y - min_y) - 1e-9).ceil() as u32;
    let cx_out = w_out as f64 / 2.0;
    let cy_out = h_out as f64 / 2.0;

    let mut out_img = RgbaImage::new(w_out, h_out);
    let in_rgba = img.to_rgba8();

    // Backward mapping with bilinear interpolation
    for y_out in 0..h_out {
        for x_out in 0..w_out {
            // Center of the target pixel
            let dx = x_out as f64 - cx_out + 0.5;
            let dy = y_out as f64 - cy_out + 0.5;

            // Rotate backward to find source coordinates
            let src_x = (dx * cos_t + dy * sin_t) + cx_in - 0.5;
            let src_y = (-dx * sin_t + dy * cos_t) + cy_in - 0.5;

            // Check if the source point falls within the input image bounds (leaving 1px margin for bilinear interpolation)
            if src_x >= 0.0 && src_x < (w_in - 1) as f64 && src_y >= 0.0 && src_y < (h_in - 1) as f64 {
                let x0 = src_x.floor() as u32;
                let y0 = src_y.floor() as u32;
                let x1 = x0 + 1;
                let y1 = y0 + 1;

                let wx = src_x - x0 as f64;
                let wy = src_y - y0 as f64;

                let p00 = in_rgba.get_pixel(x0, y0);
                let p10 = in_rgba.get_pixel(x1, y0);
                let p01 = in_rgba.get_pixel(x0, y1);
                let p11 = in_rgba.get_pixel(x1, y1);

                let mut interpolated = [0u8; 4];
                for c in 0..4 {
                    let val = (p00[c] as f64) * (1.0 - wx) * (1.0 - wy)
                            + (p10[c] as f64) * wx * (1.0 - wy)
                            + (p01[c] as f64) * (1.0 - wx) * wy
                            + (p11[c] as f64) * wx * wy;
                    interpolated[c] = val.round().clamp(0.0, 255.0) as u8;
                }
                out_img.put_pixel(x_out, y_out, Rgba(interpolated));
            } else {
                // Out of bounds: pad with solid black
                out_img.put_pixel(x_out, y_out, Rgba([0, 0, 0, 255]));
            }
        }
    }

    out_img
}

/// Crops an image to a specified rectangle.
/// If the rectangle falls outside of the source image boundary, the out-of-bounds area is filled with black.
pub fn crop_image(img: &RgbaImage, x: i32, y: i32, w: u32, h: u32) -> RgbaImage {
    let mut out_img = RgbaImage::new(w, h);
    let (src_w, src_h) = img.dimensions();

    for dest_y in 0..h {
        for dest_x in 0..w {
            let src_x = x + dest_x as i32;
            let src_y = y + dest_y as i32;

            if src_x >= 0 && src_x < src_w as i32 && src_y >= 0 && src_y < src_h as i32 {
                let pixel = img.get_pixel(src_x as u32, src_y as u32);
                out_img.put_pixel(dest_x, dest_y, *pixel);
            } else {
                // Out of bounds: fill with solid black
                out_img.put_pixel(dest_x, dest_y, Rgba([0, 0, 0, 255]));
            }
        }
    }

    out_img
}

/// Orchestrates rotating and cropping an image from input path to output path.
pub fn rotate_and_crop(
    input_path: &Path,
    output_path: &Path,
    crop_rect: Option<(i32, i32, u32, u32)>,
    rotate_deg: Option<f64>,
) -> ImageResult<()> {
    let img = image::open(input_path)?;

    // 1. Rotate if specified and non-trivial
    let rotated_img = if let Some(deg) = rotate_deg {
        if deg.abs() > 1e-5 {
            DynamicImage::ImageRgba8(rotate_image(&img, deg))
        } else {
            img
        }
    } else {
        img
    };

    // 2. Crop if specified
    let cropped_img = if let Some((x, y, w, h)) = crop_rect {
        DynamicImage::ImageRgba8(crop_image(&rotated_img.to_rgba8(), x, y, w, h))
    } else {
        rotated_img
    };

    // 3. Save
    let format = image::ImageFormat::from_path(output_path).ok();
    if format == Some(image::ImageFormat::Jpeg) {
        let rgb_img = cropped_img.to_rgb8();
        rgb_img.save(output_path)?;
    } else {
        cropped_img.save(output_path)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crop_image() {
        // Create a 4x4 red image
        let mut img = RgbaImage::new(4, 4);
        for y in 0..4 {
            for x in 0..4 {
                img.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }

        // Crop a 2x2 area from (1, 1)
        let cropped = crop_image(&img, 1, 1, 2, 2);
        assert_eq!(cropped.dimensions(), (2, 2));
        assert_eq!(cropped.get_pixel(0, 0), &Rgba([255, 0, 0, 255]));

        // Crop an area that goes out of bounds: 2x2 area from (3, 3)
        // (3,3) is inside, but the rest (4,3), (3,4), (4,4) are out of bounds and should be black
        let cropped_out = crop_image(&img, 3, 3, 2, 2);
        assert_eq!(cropped_out.dimensions(), (2, 2));
        assert_eq!(cropped_out.get_pixel(0, 0), &Rgba([255, 0, 0, 255])); // (3,3) inside bounds
        assert_eq!(cropped_out.get_pixel(1, 0), &Rgba([0, 0, 0, 255]));   // (4,3) out of bounds -> black
        assert_eq!(cropped_out.get_pixel(0, 1), &Rgba([0, 0, 0, 255]));   // (3,4) out of bounds -> black
        assert_eq!(cropped_out.get_pixel(1, 1), &Rgba([0, 0, 0, 255]));   // (4,4) out of bounds -> black
    }

    #[test]
    fn test_rotate_image_90() {
        // Create a 2x4 image:
        // Row 0: Red, Green
        // Row 1: Blue, White
        // Row 2: Black, Yellow
        // Row 3: Purple, Cyan
        let mut img = RgbaImage::new(2, 4);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));   // Red
        img.put_pixel(1, 0, Rgba([0, 255, 0, 255]));   // Green
        img.put_pixel(0, 1, Rgba([0, 0, 255, 255]));   // Blue
        img.put_pixel(1, 1, Rgba([255, 255, 255, 255])); // White
        img.put_pixel(0, 2, Rgba([0, 0, 0, 255]));     // Black
        img.put_pixel(1, 2, Rgba([255, 255, 0, 255])); // Yellow
        img.put_pixel(0, 3, Rgba([128, 0, 128, 255])); // Purple
        img.put_pixel(1, 3, Rgba([0, 255, 255, 255])); // Cyan

        let dyn_img = DynamicImage::ImageRgba8(img);
        // Rotate by 90 degrees
        let rotated = rotate_image(&dyn_img, 90.0);
        // The output dimensions for 90 deg rotation of 2x4 should be 4x2
        assert_eq!(rotated.dimensions(), (4, 2));
    }
}
