use std::hint::black_box;
use std::time::{Duration, Instant};

use image::codecs::jpeg::JpegEncoder;
use image::RgbImage;

const ITERATIONS: usize = 5;

fn baseline(image: &RgbImage, quality: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, quality)
        .encode_image(image)
        .expect("baseline JPEG encoding should succeed");
    bytes
}

fn turbo(image: &RgbImage, quality: u8, subsamp: turbojpeg::Subsamp) -> Vec<u8> {
    turbojpeg::compress_image(image, i32::from(quality), subsamp)
        .expect("TurboJPEG encoding should succeed")
        .to_vec()
}

fn measure<F>(mut encode: F, iterations: usize) -> (Duration, usize)
where
    F: FnMut() -> Vec<u8>,
{
    let started = Instant::now();
    let mut size = 0;
    for _ in 0..iterations {
        size = black_box(encode()).len();
    }
    (started.elapsed(), size)
}

fn main() {
    let image = RgbImage::from_fn(2480, 3508, |x, y| {
        image::Rgb([(x % 251) as u8, (y % 241) as u8, ((x * y) % 239) as u8])
    });
    let quality = 85;

    let (baseline_time, baseline_size) = measure(|| baseline(&image, quality), ITERATIONS);
    let (turbo_444_time, turbo_444_size) = measure(
        || turbo(&image, quality, turbojpeg::Subsamp::None),
        ITERATIONS,
    );
    let (turbo_420_time, turbo_420_size) = measure(
        || turbo(&image, quality, turbojpeg::Subsamp::Sub2x2),
        ITERATIONS,
    );
    let baseline_avg = baseline_time / ITERATIONS as u32;
    let turbo_444_avg = turbo_444_time / ITERATIONS as u32;
    let turbo_420_avg = turbo_420_time / ITERATIONS as u32;

    println!("JPEG benchmark: {ITERATIONS} iterations, 2480x3508 RGB, quality {quality}");
    println!("image::JpegEncoder: {baseline_avg:?}/image, {baseline_size} bytes");
    println!("TurboJPEG 4:4:4:    {turbo_444_avg:?}/image, {turbo_444_size} bytes");
    println!("TurboJPEG 4:2:0:    {turbo_420_avg:?}/image, {turbo_420_size} bytes");
    println!(
        "speedup (4:4:4):    {:.2}x",
        baseline_avg.as_secs_f64() / turbo_444_avg.as_secs_f64()
    );
    println!(
        "speedup (4:2:0):    {:.2}x",
        baseline_avg.as_secs_f64() / turbo_420_avg.as_secs_f64()
    );
}
