use std::io::Cursor;

use image::{ImageFormat, Rgba, RgbaImage};

use crate::auth::Error;

pub struct Skin {
    pixels: RgbaImage,
    palette_indices: Option<Vec<u8>>,
}

pub fn decode_png(data: &[u8]) -> Result<Skin, Error> {
    let pixels = image::load_from_memory_with_format(data, ImageFormat::Png)
        .map_err(|_| Error::Material)?
        .into_rgba8();
    let mut reader = png::Decoder::new(Cursor::new(data))
        .read_info()
        .map_err(|_| Error::Material)?;
    let palette_indices = if reader.info().color_type == png::ColorType::Indexed {
        // Go compares palette indices when identifying the model, even if two
        // entries have identical RGBA colors. Keep the original packed indices.
        let bits = reader.info().bit_depth as usize;
        let mut packed = vec![0; reader.output_buffer_size().ok_or(Error::Material)?];
        let output = reader
            .next_frame(&mut packed)
            .map_err(|_| Error::Material)?;
        let mut indices = Vec::with_capacity((output.width * output.height) as usize);
        for y in 0..output.height as usize {
            for x in 0..output.width as usize {
                let byte = packed[y * output.line_size + x * bits / 8];
                indices.push((byte >> (8 - bits - (x * bits % 8))) & ((1_u16 << bits) - 1) as u8);
            }
        }
        Some(indices)
    } else {
        None
    };
    Ok(Skin {
        pixels,
        palette_indices,
    })
}

pub fn model_type(skin: &Skin) -> &'static str {
    if skin.pixels.width() != 64 || !matches!(skin.pixels.height(), 32 | 64) {
        return "unknown";
    }
    if pixel_identity(skin, 54, 20) == pixel_identity(skin, 54, 36) {
        "slim"
    } else {
        "default"
    }
}

fn pixel_identity(skin: &Skin, x: u32, y: u32) -> u32 {
    if x >= skin.pixels.width() || y >= skin.pixels.height() {
        return 0;
    }
    if let Some(indices) = &skin.palette_indices {
        return u32::from(indices[(y * skin.pixels.width() + x) as usize]);
    }
    let pixel = skin.pixels.get_pixel(x, y).0;
    let alpha = 127 - u32::from(pixel[3] >> 1);
    alpha << 24 | u32::from(pixel[0]) << 16 | u32::from(pixel[1]) << 8 | u32::from(pixel[2])
}

pub fn avatar(skin: &Skin, size: u32) -> Result<Vec<u8>, Error> {
    if size == 0 {
        return Err(Error::Material);
    }
    let length = (size as usize)
        .checked_mul(size as usize)
        .and_then(|value| value.checked_mul(4))
        .filter(|value| *value <= isize::MAX as usize)
        .ok_or(Error::Material)?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(length)
        .map_err(|_| Error::Material)?;
    pixels.resize(length, 0);
    let mut output = RgbaImage::from_raw(size, size, pixels).ok_or(Error::Material)?;
    for y in 0..size {
        for x in 0..size {
            let mut pixel = [0_u8, 0, 0, 255];
            for origin in [(8, 8), (40, 8)] {
                let (red, green, blue, transparency) = sample(skin, origin, x, y, size);
                let opacity = 127 - transparency;
                pixel[0] = ((red * opacity + u32::from(pixel[0]) * transparency) / 127) as u8;
                pixel[1] = ((green * opacity + u32::from(pixel[1]) * transparency) / 127) as u8;
                pixel[2] = ((blue * opacity + u32::from(pixel[2]) * transparency) / 127) as u8;
            }
            output.put_pixel(x, y, Rgba(pixel));
        }
    }
    let mut encoded = Cursor::new(Vec::new());
    output
        .write_to(&mut encoded, ImageFormat::Png)
        .map_err(|_| Error::Material)?;
    Ok(encoded.into_inner())
}

// Integrate the source area and retain GD's 7-bit alpha and rounding behavior.
// Bilinear resizing and algebraic cancellation of the divisions change pixels.
fn sample(skin: &Skin, origin: (u32, u32), x: u32, y: u32, size: u32) -> (u32, u32, u32, u32) {
    let left = f64::from(x) * 8.0 / f64::from(size);
    let right = f64::from(x + 1) * 8.0 / f64::from(size);
    let top = f64::from(y) * 8.0 / f64::from(size);
    let bottom = f64::from(y + 1) * 8.0 / f64::from(size);
    let (mut red, mut green, mut blue, mut transparent, mut opacity_weight, mut area) =
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    let mut source_y = top.floor() as u32;
    while f64::from(source_y) < bottom {
        let height = f64::from(source_y + 1).min(bottom) - f64::from(source_y).max(top);
        let mut source_x = left.floor() as u32;
        while f64::from(source_x) < right {
            let width = f64::from(source_x + 1).min(right) - f64::from(source_x).max(left);
            let coverage = width * height;
            let (sx, sy) = (source_x + origin.0, source_y + origin.1);
            let pixel = if sx < skin.pixels.width() && sy < skin.pixels.height() {
                skin.pixels.get_pixel(sx, sy).0
            } else {
                [0, 0, 0, 255]
            };
            let alpha = f64::from(127 - (pixel[3] >> 1));
            let weight = (127.0 - alpha) * coverage;
            red += f64::from(pixel[0]) * weight;
            green += f64::from(pixel[1]) * weight;
            blue += f64::from(pixel[2]) * weight;
            transparent += alpha * coverage;
            opacity_weight += weight;
            area += coverage;
            source_x += 1;
        }
        source_y += 1;
    }
    if area != 0.0 {
        red /= area;
        green /= area;
        blue /= area;
        transparent /= area;
    }
    if opacity_weight != 0.0 {
        let average_opacity = opacity_weight / area;
        red /= average_opacity;
        green /= average_opacity;
        blue /= average_opacity;
    }
    (
        (red + 0.5).min(255.0) as u32,
        (green + 0.5).min(255.0) as u32,
        (blue + 0.5).min(255.0) as u32,
        (transparent + 0.5).min(127.0) as u32,
    )
}
