use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "icrop", about = "Image rotation and cropping utility")]
struct Cli {
    /// Input image file path
    input: PathBuf,

    /// Output image file path (overwrites input if not specified)
    output: Option<PathBuf>,

    /// Crop rectangle in format x,y,width,height (e.g. 100,50,400,300)
    #[arg(long, value_name = "X,Y,W,H")]
    crop: Option<String>,

    /// Rotation angle in degrees clockwise (e.g. 45 or -90)
    #[arg(long)]
    rotate: Option<f64>,
}

fn parse_crop(crop_str: &str) -> Result<(i32, i32, u32, u32), String> {
    let parts: Vec<&str> = crop_str.split(',').collect();
    if parts.len() != 4 {
        return Err("Crop rectangle must contain exactly 4 comma-separated values: X,Y,W,H".to_string());
    }
    let x = parts[0].trim().parse::<i32>().map_err(|e| format!("Invalid X value: {}", e))?;
    let y = parts[1].trim().parse::<i32>().map_err(|e| format!("Invalid Y value: {}", e))?;
    let w = parts[2].trim().parse::<u32>().map_err(|e| format!("Invalid width: {}", e))?;
    let h = parts[3].trim().parse::<u32>().map_err(|e| format!("Invalid height: {}", e))?;
    Ok((x, y, w, h))
}

fn main() {
    let cli = Cli::parse();
    
    let crop_rect = if let Some(crop_str) = cli.crop {
        match parse_crop(&crop_str) {
            Ok(rect) => Some(rect),
            Err(err) => {
                eprintln!("Error: {}", err);
                std::process::exit(1);
            }
        }
    } else {
        None
    };

    let output = cli.output.unwrap_or_else(|| cli.input.clone());

    match icrop::rotate_and_crop(&cli.input, &output, crop_rect, cli.rotate) {
        Ok(()) => {
            println!("Successfully processed image and saved to {:?}", output);
        }
        Err(err) => {
            eprintln!("Error processing image: {}", err);
            std::process::exit(1);
        }
    }
}
