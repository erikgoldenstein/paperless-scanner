use image::RgbImage;
use openjph_core::codestream::Codestream;
use openjph_core::file::MemOutfile;
use openjph_core::types::{Point, Size};

pub fn encode_rgb(image: &RgbImage, quality: u8) -> Result<Vec<u8>, String> {
    let (width, height) = image.dimensions();
    let pixel_count = (width * height) as usize;
    let mut components = vec![Vec::with_capacity(pixel_count); 3];
    for pixel in image.pixels() {
        components[0].push(i32::from(pixel[0]));
        components[1].push(i32::from(pixel[1]));
        components[2].push(i32::from(pixel[2]));
    }

    let mut codestream = Codestream::new();
    let siz = codestream.access_siz_mut();
    siz.set_image_extent(Point::new(width, height));
    siz.set_tile_size(Size::new(width, height));
    siz.set_num_components(3);
    for component in 0..3 {
        siz.set_comp_info(component, Point::new(1, 1), 8, false);
    }

    let cod = codestream.access_cod_mut();
    cod.set_reversible(false);
    cod.set_color_transform(true);
    cod.set_num_decomposition(decomposition_levels(width, height));
    cod.set_block_dims(64, 64);
    codestream.access_qcd_mut().set_delta(quality_step(quality));
    codestream.set_planar(0);

    let mut output = MemOutfile::new();
    codestream
        .write_headers(&mut output, &[])
        .map_err(|error| format!("OpenJPH header encoding failed: {error}"))?;
    for y in 0..height as usize {
        let start = y * width as usize;
        let end = start + width as usize;
        for (component, values) in components.iter().enumerate() {
            codestream
                .exchange(&values[start..end], component as u32)
                .map_err(|error| format!("OpenJPH image encoding failed: {error}"))?;
        }
    }
    codestream
        .flush(&mut output)
        .map_err(|error| format!("OpenJPH finalization failed: {error}"))?;
    Ok(output.get_data().to_vec())
}

fn decomposition_levels(width: u32, height: u32) -> u32 {
    let mut levels = 0;
    let mut smallest = width.min(height);
    while smallest > 1 && levels < 5 {
        smallest /= 2;
        levels += 1;
    }
    levels
}

fn quality_step(quality: u8) -> f32 {
    let quality = quality.clamp(1, 100);
    0.0001 * 2.0_f32.powf((100 - u32::from(quality)) as f32 / 10.0)
}

#[cfg(test)]
mod tests {
    use super::encode_rgb;

    #[test]
    fn openjph_produces_a_jpeg2000_codestream() {
        let image = image::RgbImage::from_pixel(32, 24, image::Rgb([20, 40, 80]));
        let encoded = encode_rgb(&image, 85).unwrap();

        assert_eq!(&encoded[..2], &[0xff, 0x4f]);
        assert_eq!(&encoded[encoded.len() - 2..], &[0xff, 0xd9]);
    }
}
