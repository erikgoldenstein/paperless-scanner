use std::hint::black_box;
use std::time::Instant;

use image::RgbImage;

const ITERATIONS: usize = 3;

fn measure<F>(mut encode: F) -> (std::time::Duration, usize)
where
    F: FnMut() -> Vec<u8>,
{
    let started = Instant::now();
    let mut size = 0;
    for _ in 0..ITERATIONS {
        size = black_box(encode()).len();
    }
    (started.elapsed(), size)
}

fn main() {
    let image = RgbImage::from_fn(1200, 1600, |x, y| {
        image::Rgb([(x % 251) as u8, (y % 241) as u8, ((x * y) % 239) as u8])
    });

    let (openjpeg_time, openjpeg_size) =
        measure(|| encode_openjpeg(&image, 85).expect("OpenJPEG encoding should succeed"));
    let (openjph_time, openjph_size) = measure(|| {
        paperless_scanner_lib::openjph_experiment::encode_rgb(&image, 85)
            .expect("OpenJPH encoding should succeed")
    });

    let openjpeg_avg = openjpeg_time / ITERATIONS as u32;
    let openjph_avg = openjph_time / ITERATIONS as u32;
    println!("JPEG2000 benchmark: {ITERATIONS} iterations, 1200x1600 RGB, quality 85");
    println!("OpenJPEG: {openjpeg_avg:?}/image, {openjpeg_size} bytes");
    println!("OpenJPH:  {openjph_avg:?}/image, {openjph_size} bytes");
    println!(
        "OpenJPH speedup: {:.2}x",
        openjpeg_avg.as_secs_f64() / openjph_avg.as_secs_f64()
    );
}

fn encode_openjpeg(image: &RgbImage, compression: u8) -> Result<Vec<u8>, String> {
    use openjpeg_sys as opj;
    use std::ffi::CString;
    use std::fs;

    let (width, height) = image.dimensions();
    let mut components = [opj::opj_image_cmptparm_t {
        dx: 1,
        dy: 1,
        w: width,
        h: height,
        x0: 0,
        y0: 0,
        prec: 8,
        bpp: 8,
        sgnd: 0,
    }; 3];
    let path = std::env::temp_dir().join(format!("paperless-jp2-bench-{}.jp2", std::process::id()));
    let path_string = CString::new(path.to_string_lossy().as_bytes()).unwrap();
    let result = (|| unsafe {
        let image_ptr = opj::opj_image_create(
            3,
            components.as_mut_ptr(),
            opj::COLOR_SPACE::OPJ_CLRSPC_SRGB,
        );
        if image_ptr.is_null() {
            return Err("OpenJPEG image allocation failed".to_string());
        }
        (*image_ptr).x1 = width;
        (*image_ptr).y1 = height;
        let component_len = (width * height) as usize;
        let image_components = std::slice::from_raw_parts_mut((*image_ptr).comps, 3);
        for channel in 0..3 {
            let data =
                std::slice::from_raw_parts_mut(image_components[channel].data, component_len);
            for (index, pixel) in image.pixels().enumerate() {
                data[index] = i32::from(pixel[channel]);
            }
        }
        let mut parameters = std::mem::zeroed::<opj::opj_cparameters_t>();
        opj::opj_set_default_encoder_parameters(&mut parameters);
        parameters.numresolution = 6;
        parameters.tcp_numlayers = 1;
        parameters.cp_disto_alloc = 1;
        parameters.tcp_rates[0] = 1.0 + (100.0 - compression.clamp(1, 100) as f32) * 0.12;
        parameters.irreversible = 1;
        let codec = opj::opj_create_compress(opj::CODEC_FORMAT::OPJ_CODEC_JP2);
        let stream = if codec.is_null() {
            std::ptr::null_mut()
        } else {
            opj::opj_stream_create_default_file_stream(path_string.as_ptr(), 0)
        };
        if codec.is_null() || stream.is_null() {
            if !codec.is_null() {
                opj::opj_destroy_codec(codec);
            }
            opj::opj_image_destroy(image_ptr);
            return Err("OpenJPEG codec allocation failed".to_string());
        }
        let setup_success = opj::opj_setup_encoder(codec, &mut parameters, image_ptr) != 0;
        if setup_success {
            let thread_count = std::thread::available_parallelism()
                .map(|parallelism| parallelism.get().min(8) as i32)
                .unwrap_or(1);
            let _ = opj::opj_codec_set_threads(codec, thread_count);
        }
        let success = setup_success
            && opj::opj_start_compress(codec, image_ptr, stream) != 0
            && opj::opj_encode(codec, stream) != 0
            && opj::opj_end_compress(codec, stream) != 0;
        opj::opj_stream_destroy(stream);
        opj::opj_destroy_codec(codec);
        opj::opj_image_destroy(image_ptr);
        if !success {
            return Err("OpenJPEG encoding failed".to_string());
        }
        fs::read(&path).map_err(|error| format!("Could not read OpenJPEG output: {error}"))
    })();
    let _ = fs::remove_file(path);
    result
}
