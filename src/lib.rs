use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "openjph-experiment")]
pub mod openjph_experiment;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    pub path: PathBuf,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct DocumentSession {
    pages: Vec<Page>,
    selected: Option<usize>,
}

impl DocumentSession {
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn add_page(&mut self, path: impl Into<PathBuf>) {
        self.pages.push(Page { path: path.into() });
        self.selected = Some(self.pages.len() - 1);
    }

    pub fn replace_selected(&mut self, path: impl Into<PathBuf>) -> Result<(), SessionError> {
        let index = self.selected.ok_or(SessionError::NoSelectedPage)?;
        self.pages[index] = Page { path: path.into() };
        Ok(())
    }

    pub fn select(&mut self, index: usize) -> Result<(), SessionError> {
        if index >= self.pages.len() {
            return Err(SessionError::PageOutOfRange);
        }
        self.selected = Some(index);
        Ok(())
    }

    pub fn reset(&mut self) -> Vec<Page> {
        self.selected = None;
        std::mem::take(&mut self.pages)
    }

    pub fn remove(&mut self, index: usize) -> Result<Page, SessionError> {
        if index >= self.pages.len() {
            return Err(SessionError::PageOutOfRange);
        }
        let removed = self.pages.remove(index);
        self.selected = match self.pages.len() {
            0 => None,
            length if self.selected.unwrap_or(0) >= length => Some(length - 1),
            _ => self.selected,
        };
        Ok(removed)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SessionError {
    #[error("no page is selected")]
    NoSelectedPage,
    #[error("page index is out of range")]
    PageOutOfRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanSettings {
    pub device: String,
    pub resolution: u32,
    pub mode: String,
}

impl Default for ScanSettings {
    fn default() -> Self {
        Self {
            device: String::new(),
            resolution: 300,
            mode: "Color".to_string(),
        }
    }
}

pub fn scanimage_args(settings: &ScanSettings) -> Vec<String> {
    let mut args = vec!["--format=png".to_string()];
    if !settings.device.trim().is_empty() {
        args.push(format!("--device-name={}", settings.device));
    }
    args.push(format!("--resolution={}", settings.resolution));
    args.push(format!("--mode={}", settings.mode));
    args
}

pub fn paperless_upload_url(base_url: &str) -> String {
    format!(
        "{}/api/documents/post_document/",
        base_url.trim_end_matches('/')
    )
}

pub fn paperless_authorization(token: &str) -> String {
    format!("Token {}", token.trim())
}

pub fn optional_upload_title(title: &str) -> Option<String> {
    let title = title.trim();
    (!title.is_empty()).then(|| title.to_string())
}

const FILE_IDENTIFIER_ALPHABET: &[u8; 62] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

fn short_base62_identifier(mut value: u64) -> String {
    let radix = FILE_IDENTIFIER_ALPHABET.len() as u64;
    let space = radix.pow(8);
    let mut identifier = [b'0'; 8];
    value %= space;
    for position in (0..identifier.len()).rev() {
        identifier[position] = FILE_IDENTIFIER_ALPHABET[(value % radix) as usize];
        value /= radix;
    }
    String::from_utf8(identifier.to_vec()).expect("identifier alphabet is valid UTF-8")
}

fn update_file_identifier_hash(hash: &mut u64, bytes: &[u8]) {
    // FNV-1a is small, deterministic, and sufficient for a short display/file
    // identifier. This is not intended as a security or deduplication hash.
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(1_099_511_628_211);
    }
}

fn upload_file_identifier(paths: &[String], rotations: &[u16]) -> Result<String, String> {
    if paths.len() != rotations.len() {
        return Err("Each page must have one rotation value".to_string());
    }

    let mut hash = 14_695_981_039_346_656_037_u64;
    for (index, (path, rotation)) in paths.iter().zip(rotations).enumerate() {
        update_file_identifier_hash(&mut hash, &(index as u64).to_le_bytes());
        update_file_identifier_hash(&mut hash, &rotation.to_le_bytes());
        let bytes = std::fs::read(path)
            .map_err(|error| format!("Could not read page for file identifier: {error}"))?;
        update_file_identifier_hash(&mut hash, &(bytes.len() as u64).to_le_bytes());
        update_file_identifier_hash(&mut hash, &bytes);
    }
    Ok(short_base62_identifier(hash))
}

fn upload_filename(title: &str, identifier: &str, hash_file_naming: bool) -> String {
    let source = optional_upload_title(title).unwrap_or_else(|| {
        if hash_file_naming {
            identifier.to_string()
        } else {
            "scan".to_string()
        }
    });
    let sanitized: String = source
        .chars()
        .map(|character| match character {
            '/' | '\\' | '\0'..='\u{1f}' | '\u{7f}' => '_',
            character => character,
        })
        .collect();
    let sanitized = sanitized.trim().trim_matches('.');
    let name = if sanitized.is_empty() {
        identifier
    } else {
        sanitized
    };
    if name.to_ascii_lowercase().ends_with(".pdf") {
        name.to_string()
    } else {
        format!("{name}.pdf")
    }
}

pub fn is_page_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("page-") && path.extension() == Some("png".as_ref()))
}

fn default_compression() -> u8 {
    85
}

fn default_ask_for_filename() -> bool {
    true
}

fn default_hash_file_naming() -> bool {
    true
}

fn default_max_upload_size_mb() -> u32 {
    10
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompressionFormat {
    Jpeg,
    Jpeg2000,
}

impl Default for CompressionFormat {
    fn default() -> Self {
        Self::Jpeg
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaperFormat {
    A4,
    #[serde(rename = "us-letter")]
    UsLetter,
    Legal,
    A5,
}

impl Default for PaperFormat {
    fn default() -> Self {
        Self::A4
    }
}

fn paper_size_points(format: PaperFormat) -> (f32, f32) {
    match format {
        PaperFormat::A4 => (595.28, 841.89),
        PaperFormat::UsLetter => (612.0, 792.0),
        PaperFormat::Legal => (612.0, 1008.0),
        PaperFormat::A5 => (419.53, 595.28),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub scanner: ScanSettings,
    pub paperless_url: String,
    pub paperless_token: String,
    #[serde(default = "default_compression")]
    pub compression: u8,
    #[serde(default)]
    pub compression_format: CompressionFormat,
    #[serde(default)]
    pub paper_format: PaperFormat,
    #[serde(default)]
    pub simple_mode: bool,
    #[serde(default = "default_ask_for_filename")]
    pub ask_for_filename: bool,
    #[serde(default = "default_hash_file_naming")]
    pub hash_file_naming: bool,
    #[serde(default)]
    pub debug_history: bool,
    #[serde(default = "default_max_upload_size_mb")]
    pub max_upload_size_mb: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            scanner: ScanSettings::default(),
            paperless_url: String::new(),
            paperless_token: String::new(),
            compression: default_compression(),
            compression_format: CompressionFormat::default(),
            paper_format: PaperFormat::default(),
            simple_mode: false,
            ask_for_filename: default_ask_for_filename(),
            hash_file_naming: default_hash_file_naming(),
            debug_history: false,
            max_upload_size_mb: default_max_upload_size_mb(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanResult {
    pub path: String,
    pub preview: String,
    pub thumbnail: String,
}

#[cfg(feature = "gui")]
pub async fn list_scanners() -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(list_scanners_sync)
        .await
        .map_err(|error| format!("Scanner lookup failed: {error}"))?
}

#[cfg(feature = "gui")]
fn list_scanners_sync() -> Result<Vec<String>, String> {
    let output = run_scanimage_capture(
        &["-L".to_string()],
        std::time::Duration::from_secs(5),
        "Scanner discovery timed out. Reconnect the scanner and try again.",
    )?;

    if !output.status.success() {
        return Err(command_error("scanimage -L", &output));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(parse_scanner_list(&format!("{stdout}\n{stderr}")))
}

pub fn parse_scanner_list(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            let start = line.find('`')? + 1;
            let end = line[start..].find('\'')? + start;
            Some(line[start..end].to_string())
        })
        .fold(Vec::new(), |mut scanners, scanner| {
            if !scanners.contains(&scanner) {
                scanners.push(scanner);
            }
            scanners
        })
}

pub fn default_scanner_device(scanners: &[String]) -> Option<String> {
    scanners
        .iter()
        .find(|scanner| !scanner.starts_with("v4l:"))
        .cloned()
}

fn remove_scan_option(args: &[String], option: &str) -> Vec<String> {
    args.iter()
        .filter(|arg| !(*arg == option || arg.starts_with(&format!("{option}="))))
        .cloned()
        .collect()
}

fn unsupported_scan_option(message: &str) -> Option<&'static str> {
    let option = message
        .split("unrecognized option '")
        .nth(1)
        .and_then(|rest| rest.split('\'').next())?;
    ["--resolution", "--mode"]
        .into_iter()
        .find(|candidate| option == *candidate || option.starts_with(&format!("{candidate}=")))
}

#[cfg(feature = "gui")]
fn scanner_probe_args(device: &str) -> Vec<String> {
    vec![format!("--device-name={device}"), "--dont-scan".to_string()]
}

#[cfg(feature = "gui")]
fn run_scanimage_capture(
    args: &[String],
    timeout: std::time::Duration,
    timeout_message: &str,
) -> Result<std::process::Output, String> {
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::Instant;

    let mut child = Command::new("scanimage")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not run scanimage: {error}"))?;
    let deadline = Instant::now() + timeout;

    loop {
        if child
            .try_wait()
            .map_err(|error| format!("Could not read scanner state: {error}"))?
            .is_some()
        {
            return child
                .wait_with_output()
                .map_err(|error| format!("Could not read scanner output: {error}"));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(timeout_message.to_string());
        }
        thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[cfg(feature = "gui")]
fn preflight_scanner(device: &str) -> Result<(), String> {
    let output = run_scanimage_capture(
        &scanner_probe_args(device),
        std::time::Duration::from_secs(5),
        "Scanner unavailable or disconnected: scanner did not respond within 5 seconds. Reconnect it and try again.",
    )?;
    if output.status.success() {
        Ok(())
    } else {
        Err(scanner_command_error(&output))
    }
}

fn jpeg2000_thread_count() -> i32 {
    std::thread::available_parallelism()
        .map(|parallelism| parallelism.get().min(8) as i32)
        .unwrap_or(1)
}

#[cfg(feature = "gui")]
pub async fn scan_page(settings: ScanSettings) -> Result<ScanResult, String> {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    tauri::async_runtime::spawn_blocking(move || {
        let mut settings = settings;
        if settings.device.trim().is_empty() {
            let scanners = list_scanners_sync()?;
            settings.device = default_scanner_device(&scanners).ok_or_else(|| {
                "No scanner detected. Reconnect the scanner and try again.".to_string()
            })?;
        }
        preflight_scanner(&settings.device)?;
        let directory = page_directory()?;
        fs::create_dir_all(&directory)
            .map_err(|error| format!("Could not create temporary scan directory: {error}"))?;

        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("Could not create scan filename: {error}"))?
            .as_nanos();
        let path = directory.join(format!("page-{id}.png"));
        let mut args = scanimage_args(&settings);
        let mut skipped_options = Vec::new();
        let output = loop {
            let output = match run_scanimage(&args, &path) {
                Ok(output) => output,
                Err(error) => {
                    let _ = fs::remove_file(&path);
                    return Err(error);
                }
            };
            if output.status.success() {
                break output;
            }
            if let Some(option) = unsupported_scan_option(&String::from_utf8_lossy(&output.stderr))
            {
                if !skipped_options.contains(&option) {
                    args = remove_scan_option(&args, option);
                    skipped_options.push(option);
                    let _ = fs::remove_file(&path);
                    continue;
                }
            }
            let _ = fs::remove_file(&path);
            return Err(scanner_command_error(&output));
        };

        match finalize_scan_output(&path, &output) {
            Ok(result) => Ok(result),
            Err(error) => {
                let _ = fs::remove_file(&path);
                Err(error)
            }
        }
    })
    .await
    .map_err(|error| format!("Scan failed: {error}"))?
}

#[cfg(feature = "gui")]
pub fn restore_pages() -> Result<Vec<ScanResult>, String> {
    use std::fs;

    let mut paths = fs::read_dir(page_directory()?)
        .map_err(|error| format!("Could not read saved pages: {error}"))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| is_page_file(path))
        .collect::<Vec<_>>();
    paths.sort();
    let mut pages = Vec::new();
    for path in paths {
        match scan_result_from_path(path.clone()) {
            Ok(page) => pages.push(page),
            Err(_) => {
                // Temporary scan files are disposable. Ignore and remove corrupt leftovers
                // from an interrupted or disconnected scan instead of breaking app startup.
                let _ = fs::remove_file(path);
            }
        }
    }
    Ok(pages)
}

#[cfg(feature = "gui")]
fn scan_result_from_path(path: PathBuf) -> Result<ScanResult, String> {
    use std::fs;

    let bytes = fs::read(&path).map_err(|error| format!("Could not read saved page: {error}"))?;
    let image = image::load_from_memory(&bytes)
        .map_err(|error| format!("Could not decode saved page: {error}"))?;
    Ok(ScanResult {
        path: path.to_string_lossy().into_owned(),
        preview: encode_preview(&image, 1600, 82)?,
        thumbnail: encode_preview(&image, 180, 70)?,
    })
}

#[cfg(feature = "gui")]
fn finalize_scan_output(path: &Path, output: &std::process::Output) -> Result<ScanResult, String> {
    use std::fs;

    if !output.status.success() {
        return Err(scanner_command_error(output));
    }
    if fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
        == 0
    {
        return Err("scanimage returned an empty page".to_string());
    }
    // scanimage streams stdout directly to `path`; do not write output.stdout here.
    scan_result_from_path(path.to_path_buf())
}

#[cfg(feature = "gui")]
fn encode_preview(
    image: &image::DynamicImage,
    max_dimension: u32,
    quality: u8,
) -> Result<String, String> {
    use base64::Engine;

    let resized = image.thumbnail(max_dimension, max_dimension).to_rgb8();
    let (bytes, _) = encode_pdf_image(&resized, quality, CompressionFormat::Jpeg)
        .map_err(|error| format!("Could not encode page preview: {error}"))?;
    Ok(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[cfg(feature = "gui")]
pub fn cleanup_pages(paths: Vec<String>) -> Result<(), String> {
    use std::fs;

    let directory = page_directory()?;
    for page in paths {
        let path = PathBuf::from(page);
        if path.parent() != Some(directory.as_path()) {
            continue;
        }
        if path.exists() {
            fs::remove_file(path).map_err(|error| format!("Could not remove scan: {error}"))?;
        }
    }
    Ok(())
}

#[cfg(feature = "gui")]
pub fn load_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    use std::fs;

    let path = settings_path(&app)?;
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let contents =
        fs::read_to_string(path).map_err(|error| format!("Could not read settings: {error}"))?;
    serde_json::from_str(&contents).map_err(|error| format!("Could not parse settings: {error}"))
}

#[cfg(feature = "gui")]
pub fn save_settings(app: tauri::AppHandle, settings: AppSettings) -> Result<(), String> {
    use std::fs;

    let path = settings_path(&app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create settings directory: {error}"))?;
    }
    let contents = serde_json::to_string_pretty(&settings)
        .map_err(|error| format!("Could not encode settings: {error}"))?;
    fs::write(path, contents).map_err(|error| format!("Could not save settings: {error}"))
}

#[cfg(feature = "gui")]
pub async fn upload_document(
    paths: Vec<String>,
    settings: AppSettings,
    title: String,
) -> Result<String, String> {
    upload_document_with_progress(paths, settings, title, |_| {}).await
}

#[cfg(feature = "gui")]
pub async fn upload_document_with_progress<F>(
    paths: Vec<String>,
    settings: AppSettings,
    title: String,
    progress: F,
) -> Result<String, String>
where
    F: Fn(&str) + Send + 'static,
{
    let rotations = vec![0; paths.len()];
    upload_document_with_rotations(paths, rotations, settings, title, progress).await
}

#[cfg(feature = "gui")]
pub async fn upload_document_with_rotations<F>(
    paths: Vec<String>,
    rotations: Vec<u16>,
    settings: AppSettings,
    title: String,
    progress: F,
) -> Result<String, String>
where
    F: Fn(&str) + Send + 'static,
{
    use std::fs;

    if paths.is_empty() {
        return Err("There are no pages to upload".to_string());
    }
    if rotations.len() != paths.len() {
        return Err("Each page must have one rotation value".to_string());
    }
    if settings.paperless_url.trim().is_empty() {
        return Err("Set the Paperless URL in Settings first".to_string());
    }
    if settings.paperless_token.trim().is_empty() {
        return Err("Set the Paperless token in Settings first".to_string());
    }

    let compression_format = settings.compression_format;
    let paper_format = settings.paper_format;
    let configured_quality = settings.compression.clamp(1, 100);
    progress("Estimating PDF size…");
    let estimate_paths = paths.clone();
    let estimated_size = tauri::async_runtime::spawn_blocking(move || {
        estimate_pdf_size(&estimate_paths, configured_quality, compression_format)
    })
    .await
    .map_err(|error| format!("PDF size estimation failed: {error}"))??;
    let max_upload_size_mb = settings.max_upload_size_mb.clamp(1, 10_000);
    let mut quality =
        initial_quality_for_estimate(estimated_size, configured_quality, max_upload_size_mb);
    let file_identifier = upload_file_identifier(&paths, &rotations)?;
    let client = reqwest::Client::new();

    progress(&format!(
        "Compressing pages at quality {quality}% (estimated PDF size: {})…",
        format_bytes(estimated_size)
    ));

    for attempt in 0..=8 {
        let max_dimension = match attempt {
            0..=4 => None,
            5 => Some(1800),
            6 => Some(1400),
            7 => Some(1100),
            _ => Some(900),
        };
        if attempt > 0 {
            quality = (u16::from(quality) * 65 / 100).max(1) as u8;
            if let Some(max_dimension) = max_dimension {
                progress(&format!(
                    "Upload was too large; recompressing at quality {quality}% and {max_dimension}px…"
                ));
            } else {
                progress(&format!(
                    "Upload was too large; recompressing at quality {quality}%…"
                ));
            }
        } else if quality != configured_quality {
            progress(&format!(
                "Compressing pages at quality {quality}% (estimated PDF size: {})…",
                format_bytes(estimated_size)
            ));
        }

        let paths_for_pdf = paths.clone();
        let rotations_for_pdf = rotations.clone();
        let pdf = tauri::async_runtime::spawn_blocking(move || {
            make_pdf(
                &paths_for_pdf,
                quality,
                compression_format,
                paper_format,
                &rotations_for_pdf,
                max_dimension,
            )
        })
        .await
        .map_err(|error| format!("PDF creation failed: {error}"))??;
        let bytes = fs::read(&pdf).map_err(|error| format!("Could not read PDF: {error}"))?;
        let _ = fs::remove_file(&pdf);

        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(upload_filename(
                &title,
                &file_identifier,
                settings.hash_file_naming,
            ))
            .mime_str("application/pdf")
            .map_err(|error| format!("Could not prepare PDF upload: {error}"))?;
        let mut form = reqwest::multipart::Form::new().part("document", part);
        if let Some(title) = optional_upload_title(&title) {
            form = form.text("title", title);
        }
        progress("Uploading to Paperless…");
        let response = client
            .post(paperless_upload_url(&settings.paperless_url))
            .header(
                reqwest::header::AUTHORIZATION,
                paperless_authorization(&settings.paperless_token),
            )
            .multipart(form)
            .send()
            .await
            .map_err(|error| format!("Could not connect to Paperless: {error}"))?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if status.is_success() {
            let _ = body;
            return Ok(file_identifier.clone());
        }
        if status != reqwest::StatusCode::PAYLOAD_TOO_LARGE || attempt == 8 {
            if status == reqwest::StatusCode::PAYLOAD_TOO_LARGE {
                return Err("Paperless rejected the upload (413 Payload Too Large) even after automatic quality and resolution reduction. Increase the Paperless/reverse-proxy upload limit".to_string());
            }
            return Err(format!("Paperless rejected the upload ({status})"));
        }
    }

    Err("Upload failed before a request could be sent".to_string())
}

fn initial_quality_for_estimate(
    estimated_bytes: u64,
    configured_quality: u8,
    max_upload_size_mb: u32,
) -> u8 {
    let configured_quality = configured_quality.clamp(1, 100);
    let target_bytes = u64::from(max_upload_size_mb.clamp(1, 10_000)).saturating_mul(900_000);
    if estimated_bytes <= target_bytes {
        return configured_quality;
    }
    (u64::from(configured_quality).saturating_mul(target_bytes) / estimated_bytes)
        .clamp(1, u64::from(configured_quality)) as u8
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!("{}.{:01} MB", bytes / 1_000_000, (bytes / 100_000) % 10)
    } else {
        format!("{} KB", bytes.div_ceil(1_000))
    }
}

#[cfg(any(feature = "gui", test))]
fn estimate_pdf_size(
    paths: &[String],
    compression: u8,
    compression_format: CompressionFormat,
) -> Result<u64, String> {
    use image::GenericImageView;

    const SAMPLE_MAX_DIMENSION: u32 = 800;
    const SAFETY_NUMERATOR: u64 = 5;
    const SAFETY_DENOMINATOR: u64 = 4;
    const PDF_PAGE_OVERHEAD: u64 = 4_096;

    let mut estimate = PDF_PAGE_OVERHEAD;
    for path in paths {
        let image = image::open(path).map_err(|error| format!("Could not open page: {error}"))?;
        let (width, height) = image.dimensions();
        let sample = image
            .thumbnail(SAMPLE_MAX_DIMENSION, SAMPLE_MAX_DIMENSION)
            .to_rgb8();
        let (compressed, _) = encode_pdf_image(&sample, compression, compression_format)?;
        let sample_pixels = u64::from(sample.width()) * u64::from(sample.height());
        let source_pixels = u64::from(width) * u64::from(height);
        let scaled = u64::try_from(compressed.len())
            .unwrap_or(u64::MAX)
            .saturating_mul(source_pixels)
            .checked_div(sample_pixels.max(1))
            .unwrap_or(u64::MAX);
        estimate = estimate
            .saturating_add(
                scaled
                    .saturating_mul(SAFETY_NUMERATOR)
                    .checked_div(SAFETY_DENOMINATOR)
                    .unwrap_or(u64::MAX),
            )
            .saturating_add(PDF_PAGE_OVERHEAD);
    }
    Ok(estimate)
}

#[cfg(any(feature = "gui", test))]
fn encode_pdf_image(
    rgb: &image::RgbImage,
    compression: u8,
    compression_format: CompressionFormat,
) -> Result<(Vec<u8>, &'static str), String> {
    #[cfg(feature = "jpeg-turbo")]
    {
        if compression_format == CompressionFormat::Jpeg {
            let jpeg = turbojpeg::compress_image(
                rgb,
                i32::from(compression.clamp(1, 100)),
                turbojpeg::Subsamp::Sub2x2,
            )
            .map_err(|error| format!("Could not compress page with TurboJPEG: {error}"))?
            .to_vec();
            return Ok((jpeg, "DCTDecode"));
        }
    }

    use image::codecs::jpeg::JpegEncoder;

    match compression_format {
        CompressionFormat::Jpeg => {
            let mut jpeg = Vec::new();
            JpegEncoder::new_with_quality(&mut jpeg, compression.clamp(1, 100))
                .encode_image(rgb)
                .map_err(|error| format!("Could not compress page: {error}"))?;
            Ok((jpeg, "DCTDecode"))
        }
        CompressionFormat::Jpeg2000 => Ok((encode_jpeg2000(rgb, compression)?, "JPXDecode")),
    }
}

#[cfg(any(feature = "gui", test))]
fn page_directory() -> Result<PathBuf, String> {
    let directory = std::env::temp_dir().join("paperless-scanner");
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("Could not create temporary scan directory: {error}"))?;
    Ok(directory)
}

#[cfg(feature = "gui")]
fn settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    Ok(app
        .path()
        .app_config_dir()
        .map_err(|error| format!("Could not find settings directory: {error}"))?
        .join("settings.json"))
}

#[cfg(feature = "gui")]
fn command_error(command: &str, output: &std::process::Output) -> String {
    let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if error.is_empty() {
        format!("{command} failed with status {}", output.status)
    } else {
        format!("{command} failed: {error}")
    }
}

#[cfg(feature = "gui")]
fn run_scanimage(args: &[String], output_path: &Path) -> Result<std::process::Output, String> {
    use std::fs::File;
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};

    let mut child = Command::new("scanimage")
        .args(args)
        .stdout(Stdio::from(File::create(output_path).map_err(|error| {
            format!("Could not create scan output: {error}")
        })?))
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not run scanimage: {error}"))?;
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        if child
            .try_wait()
            .map_err(|error| format!("Could not read scanner state: {error}"))?
            .is_some()
        {
            return child
                .wait_with_output()
                .map_err(|error| format!("Could not read scanner output: {error}"));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Scanner timed out. Reconnect the scanner and try again.".to_string());
        }
        thread::sleep(Duration::from_millis(100));
    }
}

pub fn scanner_error(message: &str) -> String {
    let detail = message.trim();
    let lower = detail.to_ascii_lowercase();
    let unavailable = [
        "no such device",
        "device i/o",
        "could not open",
        "failed to open",
        "open of device",
        "not found",
        "disconnected",
    ];
    if unavailable.iter().any(|marker| lower.contains(marker)) {
        format!("Scanner unavailable or disconnected: {detail}")
    } else if lower.contains("busy") {
        format!("Scanner is busy: {detail}")
    } else if detail.is_empty() {
        "Scanner failed without an error message".to_string()
    } else {
        format!("scanimage failed: {detail}")
    }
}

#[cfg(feature = "gui")]
fn scanner_command_error(output: &std::process::Output) -> String {
    scanner_error(&String::from_utf8_lossy(&output.stderr))
}

#[cfg(any(feature = "gui", test))]
fn make_pdf(
    paths: &[String],
    compression: u8,
    compression_format: CompressionFormat,
    paper_format: PaperFormat,
    rotations: &[u16],
    max_dimension: Option<u32>,
) -> Result<PathBuf, String> {
    use image::GenericImageView;
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Document, Object, Stream};

    if paths.is_empty() {
        return Err("Cannot create a PDF without pages".to_string());
    }

    let mut document = Document::with_version("1.5");
    let pages_id = document.new_object_id();
    let mut page_ids = Vec::with_capacity(paths.len());

    if rotations.len() != paths.len() {
        return Err("Each page must have one rotation value".to_string());
    }

    for (index, path) in paths.iter().enumerate() {
        let image = image::open(path).map_err(|error| format!("Could not open page: {error}"))?;
        let image = match rotations[index] % 360 {
            0 => image,
            90 => image::DynamicImage::ImageRgba8(image::imageops::rotate90(&image)),
            180 => image::DynamicImage::ImageRgba8(image::imageops::rotate180(&image)),
            270 => image::DynamicImage::ImageRgba8(image::imageops::rotate270(&image)),
            _ => return Err("Rotation must be a multiple of 90 degrees".to_string()),
        };
        let (source_width, source_height) = image.dimensions();
        let image = max_dimension
            .map(|max| image.thumbnail(max, max))
            .unwrap_or(image);
        let (width, height) = image.dimensions();
        let rgb = image.to_rgb8();
        let (compressed, filter) = encode_pdf_image(&rgb, compression, compression_format)?;

        let (paper_width, paper_height) = paper_size_points(paper_format);
        let (paper_width, paper_height) = if width > height {
            (paper_height, paper_width)
        } else {
            (paper_width, paper_height)
        };
        let margin = 18.0_f32;
        let image_width_points = source_width as f32 * 72.0 / 300.0;
        let image_height_points = source_height as f32 * 72.0 / 300.0;
        let scale = ((paper_width - 2.0 * margin) / image_width_points)
            .min((paper_height - 2.0 * margin) / image_height_points);
        let draw_width = image_width_points * scale;
        let draw_height = image_height_points * scale;
        let draw_x = (paper_width - draw_width) / 2.0;
        let draw_y = (paper_height - draw_height) / 2.0;

        let image_id = document.new_object_id();
        document.objects.insert(
            image_id,
            Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => width as i64,
                    "Height" => height as i64,
                    "ColorSpace" => "DeviceRGB",
                    "BitsPerComponent" => 8,
                    "Filter" => filter,
                },
                compressed,
            )
            .into(),
        );

        let content = Content {
            operations: vec![
                Operation::new("q", vec![]),
                Operation::new(
                    "cm",
                    vec![
                        Object::Real(draw_width),
                        Object::Integer(0),
                        Object::Integer(0),
                        Object::Real(draw_height),
                        Object::Real(draw_x),
                        Object::Real(draw_y),
                    ],
                ),
                Operation::new("Do", vec![Object::Name(b"Im0".to_vec())]),
                Operation::new("Q", vec![]),
            ],
        }
        .encode()
        .map_err(|error| format!("Could not create PDF page: {error}"))?;
        let content_id = document.new_object_id();
        document
            .objects
            .insert(content_id, Stream::new(dictionary! {}, content).into());

        let page_id = document.new_object_id();
        document.objects.insert(
            page_id,
            dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Real(paper_width),
                    Object::Real(paper_height),
                ],
                "Resources" => dictionary! {
                    "XObject" => dictionary! { "Im0" => image_id },
                },
                "Contents" => content_id,
            }
            .into(),
        );
        page_ids.push(page_id);
    }

    document.objects.insert(
        pages_id,
        dictionary! {
            "Type" => "Pages",
            "Kids" => page_ids.into_iter().map(Object::Reference).collect::<Vec<_>>(),
            "Count" => paths.len() as i64,
        }
        .into(),
    );
    let catalog_id = document.new_object_id();
    document.objects.insert(
        catalog_id,
        dictionary! { "Type" => "Catalog", "Pages" => pages_id }.into(),
    );
    document.trailer.set("Root", catalog_id);
    document.compress();

    let output = page_directory()?.join(format!("upload-{}.pdf", unique_id()));
    document
        .save(&output)
        .map_err(|error| format!("Could not write PDF: {error}"))?;
    Ok(output)
}

#[cfg(any(feature = "gui", test))]
fn encode_jpeg2000(rgb: &image::RgbImage, compression: u8) -> Result<Vec<u8>, String> {
    use openjpeg_sys as opj;
    use std::ffi::CString;
    use std::fs;

    let (width, height) = rgb.dimensions();
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
    let output = page_directory()?.join(format!("upload-{}.jp2", unique_id()));
    let output_string = CString::new(output.to_string_lossy().as_bytes())
        .map_err(|_| "Could not create JPEG 2000 output path".to_string())?;
    let result = (|| unsafe {
        let image = opj::opj_image_create(
            3,
            components.as_mut_ptr(),
            opj::COLOR_SPACE::OPJ_CLRSPC_SRGB,
        );
        if image.is_null() {
            return Err("Could not allocate JPEG 2000 image".to_string());
        }
        (*image).x1 = width;
        (*image).y1 = height;

        let pixels = rgb.as_raw();
        let component_len = (width * height) as usize;
        let image_components = std::slice::from_raw_parts_mut((*image).comps, 3);
        for channel in 0..3 {
            let data =
                std::slice::from_raw_parts_mut(image_components[channel].data, component_len);
            for (index, pixel) in pixels.chunks_exact(3).enumerate() {
                data[index] = i32::from(pixel[channel]);
            }
        }

        let mut parameters = std::mem::zeroed::<opj::opj_cparameters_t>();
        opj::opj_set_default_encoder_parameters(&mut parameters);
        let mut resolution_count = 1;
        let mut smallest_dimension = width.min(height);
        while smallest_dimension > 1 && resolution_count < 6 {
            smallest_dimension /= 2;
            resolution_count += 1;
        }
        parameters.numresolution = resolution_count;
        parameters.tcp_numlayers = 1;
        parameters.cp_disto_alloc = 1;
        parameters.tcp_rates[0] = 1.0 + (100.0 - compression.clamp(1, 100) as f32) * 0.12;
        parameters.irreversible = 1;

        let codec = opj::opj_create_compress(opj::CODEC_FORMAT::OPJ_CODEC_JP2);
        if codec.is_null() {
            opj::opj_image_destroy(image);
            return Err("Could not create JPEG 2000 encoder".to_string());
        }
        let stream = opj::opj_stream_create_default_file_stream(output_string.as_ptr(), 0);
        if stream.is_null() {
            opj::opj_destroy_codec(codec);
            opj::opj_image_destroy(image);
            return Err("Could not create JPEG 2000 output stream".to_string());
        }

        let setup_success = opj::opj_setup_encoder(codec, &mut parameters, image) != 0;
        if setup_success {
            // The OpenJPEG build has thread support, but defaults to one worker unless this is
            // explicitly configured (or OPJ_NUM_THREADS is set in the environment).
            let _ = opj::opj_codec_set_threads(codec, jpeg2000_thread_count());
        }
        let success = setup_success
            && opj::opj_start_compress(codec, image, stream) != 0
            && opj::opj_encode(codec, stream) != 0
            && opj::opj_end_compress(codec, stream) != 0;
        opj::opj_stream_destroy(stream);
        opj::opj_destroy_codec(codec);
        opj::opj_image_destroy(image);
        if !success {
            return Err("JPEG 2000 encoder failed".to_string());
        }
        fs::read(&output).map_err(|error| format!("Could not read JPEG 2000 output: {error}"))
    })();
    let _ = fs::remove_file(output);
    result
}

#[cfg(any(feature = "gui", test))]
fn unique_id() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

#[cfg(feature = "gui")]
mod commands {
    use super::{AppSettings, ScanResult, ScanSettings};
    use tauri::AppHandle;

    #[tauri::command]
    pub async fn list_scanners() -> Result<Vec<String>, String> {
        super::list_scanners().await
    }

    #[tauri::command]
    pub async fn scan_page(settings: ScanSettings) -> Result<ScanResult, String> {
        super::scan_page(settings).await
    }

    #[tauri::command]
    pub fn restore_pages() -> Result<Vec<ScanResult>, String> {
        super::restore_pages()
    }

    #[tauri::command]
    pub fn cleanup_pages(paths: Vec<String>) -> Result<(), String> {
        super::cleanup_pages(paths)
    }

    #[tauri::command]
    pub fn load_settings(app: AppHandle) -> Result<AppSettings, String> {
        super::load_settings(app)
    }

    #[tauri::command]
    pub fn save_settings(app: AppHandle, settings: AppSettings) -> Result<(), String> {
        super::save_settings(app, settings)
    }

    #[tauri::command]
    pub async fn upload_document(
        app: AppHandle,
        paths: Vec<String>,
        rotations: Option<Vec<u16>>,
        settings: AppSettings,
        title: String,
        job_id: String,
    ) -> Result<String, String> {
        use tauri::Emitter;

        let rotations = rotations.unwrap_or_else(|| vec![0; paths.len()]);
        super::upload_document_with_rotations(paths, rotations, settings, title, move |stage| {
            let _ = app.emit(
                "upload-progress",
                serde_json::json!({ "job_id": job_id.clone(), "stage": stage }),
            );
        })
        .await
    }
}

#[cfg(feature = "gui")]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::list_scanners,
            commands::scan_page,
            commands::restore_pages,
            commands::cleanup_pages,
            commands::load_settings,
            commands::save_settings,
            commands::upload_document,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Paperless Scanner");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_pages_appends_and_selects_the_new_page() {
        let mut session = DocumentSession::default();

        session.add_page("page-1.png");
        session.add_page("page-2.png");

        assert_eq!(
            session.pages(),
            &[
                Page {
                    path: PathBuf::from("page-1.png")
                },
                Page {
                    path: PathBuf::from("page-2.png")
                },
            ]
        );
        assert_eq!(session.selected(), Some(1));
    }

    #[test]
    fn rescanning_replaces_only_the_selected_page() {
        let mut session = DocumentSession::default();
        session.add_page("page-1.png");
        session.add_page("bad-page-2.png");
        session.select(1).unwrap();

        session.replace_selected("page-2.png").unwrap();

        assert_eq!(session.pages()[0].path, PathBuf::from("page-1.png"));
        assert_eq!(session.pages()[1].path, PathBuf::from("page-2.png"));
        assert_eq!(session.selected(), Some(1));
    }

    #[test]
    fn reset_returns_pages_to_clean_them_up_and_clears_selection() {
        let mut session = DocumentSession::default();
        session.add_page("page-1.png");

        let removed = session.reset();

        assert_eq!(
            removed,
            vec![Page {
                path: PathBuf::from("page-1.png")
            }]
        );
        assert!(session.pages().is_empty());
        assert_eq!(session.selected(), None);
    }

    #[test]
    fn removing_a_page_keeps_the_remaining_pages_selectable() {
        let mut session = DocumentSession::default();
        session.add_page("page-1.png");
        session.add_page("page-2.png");
        session.add_page("page-3.png");
        session.select(1).unwrap();

        let removed = session.remove(1).unwrap();

        assert_eq!(removed.path, PathBuf::from("page-2.png"));
        assert_eq!(session.pages().len(), 2);
        assert_eq!(session.selected(), Some(1));
    }

    #[test]
    fn rescanning_without_a_page_is_rejected() {
        let mut session = DocumentSession::default();

        assert_eq!(
            session.replace_selected("page.png"),
            Err(SessionError::NoSelectedPage)
        );
    }

    #[test]
    fn scan_arguments_are_safe_for_device_names_with_spaces() {
        let settings = ScanSettings {
            device: "airscan:escl:Office Scanner".to_string(),
            resolution: 300,
            mode: "Color".to_string(),
        };
        let args = scanimage_args(&settings);

        assert_eq!(
            args,
            vec![
                "--format=png",
                "--device-name=airscan:escl:Office Scanner",
                "--resolution=300",
                "--mode=Color",
            ]
        );
    }

    #[test]
    fn default_scanner_prefers_a_real_scanner_over_a_camera() {
        let scanners = vec![
            "v4l:/dev/video0".to_string(),
            "epsonds:libusb:001:024".to_string(),
        ];

        assert_eq!(
            default_scanner_device(&scanners),
            Some("epsonds:libusb:001:024".to_string())
        );
    }

    #[test]
    fn default_scanner_does_not_fall_back_to_a_camera() {
        assert_eq!(
            default_scanner_device(&["v4l:/dev/video0".to_string()]),
            None
        );
    }

    #[test]
    fn unsupported_scan_option_can_be_removed_without_dropping_other_options() {
        let settings = ScanSettings::default();
        let args = scanimage_args(&settings);
        let filtered = remove_scan_option(&args, "--resolution");

        assert!(!filtered.iter().any(|arg| arg.starts_with("--resolution")));
        assert!(filtered.iter().any(|arg| arg == "--format=png"));
        assert!(filtered.iter().any(|arg| arg == "--mode=Color"));
    }

    #[test]
    fn scanner_errors_identify_unsupported_options() {
        assert_eq!(
            unsupported_scan_option("scanimage: unrecognized option '--resolution'"),
            Some("--resolution")
        );
        assert_eq!(
            unsupported_scan_option("scanimage: unrecognized option '--resolution=300'"),
            Some("--resolution")
        );
        assert_eq!(unsupported_scan_option("scanimage failed"), None);
    }

    #[test]
    fn jpeg2000_thread_count_is_small_but_parallel() {
        let count = jpeg2000_thread_count();

        assert!((1..=8).contains(&count));
    }

    #[test]
    fn paperless_url_is_joined_without_duplicate_slashes() {
        assert_eq!(
            paperless_upload_url("https://paperless.example/"),
            "https://paperless.example/api/documents/post_document/"
        );
    }

    #[test]
    fn paperless_uses_token_authentication() {
        assert_eq!(
            paperless_authorization("  secret-token  "),
            "Token secret-token"
        );
    }

    #[test]
    fn empty_upload_title_is_omitted_but_text_is_kept() {
        assert_eq!(optional_upload_title("  "), None);
        assert_eq!(
            optional_upload_title("  invoices  "),
            Some("invoices".to_string())
        );
    }

    #[test]
    fn upload_file_identifier_is_eight_base62_characters_and_is_stable() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first.png");
        let second = directory.path().join("second.png");
        std::fs::write(&first, b"first page").unwrap();
        std::fs::write(&second, b"second page").unwrap();
        let paths = vec![
            first.to_string_lossy().into_owned(),
            second.to_string_lossy().into_owned(),
        ];

        let identifier = upload_file_identifier(&paths, &[0, 90]).unwrap();
        assert_eq!(identifier.len(), 8);
        assert!(identifier.bytes().all(|byte| {
            byte.is_ascii_digit() || byte.is_ascii_uppercase() || byte.is_ascii_lowercase()
        }));
        assert_eq!(
            identifier,
            upload_file_identifier(&paths, &[0, 90]).unwrap()
        );
    }

    #[test]
    fn upload_file_identifier_changes_when_page_state_changes() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first.png");
        let second = directory.path().join("second.png");
        std::fs::write(&first, b"first page").unwrap();
        std::fs::write(&second, b"second page").unwrap();
        let first_path = first.to_string_lossy().into_owned();
        let second_path = second.to_string_lossy().into_owned();

        let original =
            upload_file_identifier(&[first_path.clone(), second_path.clone()], &[0, 0]).unwrap();
        let rotated =
            upload_file_identifier(&[first_path.clone(), second_path.clone()], &[90, 0]).unwrap();
        let reordered = upload_file_identifier(&[second_path, first_path], &[0, 0]).unwrap();

        assert_ne!(original, rotated);
        assert_ne!(original, reordered);
    }

    #[test]
    fn upload_filename_respects_hash_naming_setting_and_custom_names() {
        assert_eq!(upload_filename("  ", "aB12cD34", true), "aB12cD34.pdf");
        assert_eq!(upload_filename("  ", "aB12cD34", false), "scan.pdf");
        assert_eq!(upload_filename("invoice", "aB12cD34", false), "invoice.pdf");
        assert_eq!(
            upload_filename("invoice.pdf", "aB12cD34", true),
            "invoice.pdf"
        );
    }

    #[test]
    fn simple_mode_is_off_by_default() {
        assert!(!AppSettings::default().simple_mode);
    }

    #[test]
    fn filename_prompt_is_on_by_default() {
        assert!(AppSettings::default().ask_for_filename);
    }

    #[test]
    fn hash_based_file_naming_is_on_by_default() {
        assert!(AppSettings::default().hash_file_naming);
    }

    #[test]
    fn paper_format_defaults_to_a4() {
        assert_eq!(PaperFormat::default(), PaperFormat::A4);
        assert_eq!(paper_size_points(PaperFormat::UsLetter), (612.0, 792.0));
    }

    #[test]
    fn scanner_listing_parser_finds_sane_devices() {
        let output = "device `epsonds:libusb:001:019' is a Epson EW-052A Series ESC/I-2\n";

        assert_eq!(
            parse_scanner_list(output),
            vec!["epsonds:libusb:001:019".to_string()]
        );
    }

    #[test]
    fn scanner_errors_explain_disconnects_and_busy_devices() {
        assert!(scanner_error("Error during device I/O").contains("unavailable or disconnected"));
        assert!(
            scanner_error("open of device epsonds:libusb:001:024 failed: Invalid argument")
                .contains("unavailable or disconnected")
        );
        assert!(scanner_error("Device is busy").starts_with("Scanner is busy"));
    }

    #[cfg(feature = "gui")]
    #[test]
    fn scanner_probe_does_not_use_slow_scan_options() {
        let args = scanner_probe_args("epsonds:libusb:001:024");

        assert_eq!(
            args,
            vec!["--device-name=epsonds:libusb:001:024", "--dont-scan"]
        );
        assert!(!args.iter().any(|arg| arg.starts_with("--resolution")));
        assert!(!args.iter().any(|arg| arg.starts_with("--mode")));
    }

    #[cfg(feature = "gui")]
    #[test]
    fn restoring_pages_ignores_empty_scan_leftovers() {
        let path = page_directory()
            .unwrap()
            .join(format!("page-empty-test-{}.png", unique_id()));
        std::fs::write(&path, []).unwrap();

        let pages = restore_pages().unwrap();

        assert!(!pages.iter().any(|page| page.path == path.to_string_lossy()));
        assert!(!path.exists());
    }

    #[cfg(feature = "gui")]
    #[test]
    fn streamed_scan_output_is_not_overwritten_by_empty_stdout() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("page-streamed.png");
        image::RgbImage::from_pixel(20, 30, image::Rgb([255, 255, 255]))
            .save(&path)
            .unwrap();
        let output = std::process::Command::new("true").output().unwrap();

        finalize_scan_output(&path, &output).unwrap();

        let result = scan_result_from_path(path).unwrap();
        assert!(result.preview.starts_with("data:image/jpeg;base64,"));
    }

    #[test]
    fn initial_quality_uses_a_conservative_size_estimate() {
        assert_eq!(initial_quality_for_estimate(8_000_000, 85, 10), 85);
        assert_eq!(initial_quality_for_estimate(20_000_000, 85, 10), 38);
        assert_eq!(initial_quality_for_estimate(200_000_000, 40, 10), 1);
    }

    #[test]
    fn max_upload_size_defaults_to_ten_megabytes() {
        assert_eq!(AppSettings::default().max_upload_size_mb, 10);
    }

    #[test]
    fn resume_filter_ignores_old_upload_pdfs() {
        assert!(is_page_file(Path::new("/tmp/page-123.png")));
        assert!(!is_page_file(Path::new("/tmp/upload-123.pdf")));
        assert!(!is_page_file(Path::new("/tmp/page-123.jpg")));
    }

    #[cfg(feature = "gui")]
    #[test]
    fn scan_previews_are_bounded_for_fast_page_switching() {
        use base64::Engine;

        let directory = tempfile::tempdir().unwrap();
        let page = directory.path().join("page-preview.png");
        image::RgbImage::from_fn(2400, 1800, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x * y) % 256) as u8])
        })
        .save(&page)
        .unwrap();

        let result = scan_result_from_path(page).unwrap();
        let preview = base64::engine::general_purpose::STANDARD
            .decode(result.preview.split(',').nth(1).unwrap())
            .unwrap();
        let thumbnail = base64::engine::general_purpose::STANDARD
            .decode(result.thumbnail.split(',').nth(1).unwrap())
            .unwrap();
        let preview = image::load_from_memory(&preview).unwrap();
        let thumbnail = image::load_from_memory(&thumbnail).unwrap();

        assert_eq!((preview.width(), preview.height()), (1600, 1200));
        assert_eq!((thumbnail.width(), thumbnail.height()), (180, 135));
    }

    #[test]
    fn pdf_contains_all_pages() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first.png");
        let second = directory.path().join("second.png");
        image::RgbImage::from_pixel(20, 30, image::Rgb([255, 255, 255]))
            .save(&first)
            .unwrap();
        image::RgbImage::from_pixel(20, 30, image::Rgb([0, 0, 0]))
            .save(&second)
            .unwrap();

        let pdf = make_pdf(
            &[
                first.to_string_lossy().into_owned(),
                second.to_string_lossy().into_owned(),
            ],
            85,
            CompressionFormat::Jpeg,
            PaperFormat::default(),
            &[0, 90],
            None,
        )
        .unwrap();
        let bytes = std::fs::read(&pdf).unwrap();

        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.len() > 100);
        assert!(bytes
            .windows(b"/DCTDecode".len())
            .any(|window| window == b"/DCTDecode"));
        assert!(bytes
            .windows(b"/Width 30".len())
            .any(|window| window == b"/Width 30"));
        std::fs::remove_file(pdf).unwrap();
    }

    #[test]
    fn jpeg2000_pdf_uses_jpxdecode() {
        let directory = tempfile::tempdir().unwrap();
        let page = directory.path().join("page.png");
        image::RgbImage::from_pixel(20, 30, image::Rgb([100, 150, 200]))
            .save(&page)
            .unwrap();

        let pdf = make_pdf(
            &[page.to_string_lossy().into_owned()],
            85,
            CompressionFormat::Jpeg2000,
            PaperFormat::default(),
            &[0],
            None,
        )
        .unwrap();
        let bytes = std::fs::read(&pdf).unwrap();

        assert!(bytes
            .windows(b"/JPXDecode".len())
            .any(|window| window == b"/JPXDecode"));
        std::fs::remove_file(pdf).unwrap();
    }

    #[test]
    fn pdf_fallback_can_reduce_image_dimensions() {
        let directory = tempfile::tempdir().unwrap();
        let page = directory.path().join("large-page.png");
        image::RgbImage::from_pixel(1200, 800, image::Rgb([100, 150, 200]))
            .save(&page)
            .unwrap();

        let pdf = make_pdf(
            &[page.to_string_lossy().into_owned()],
            10,
            CompressionFormat::Jpeg,
            PaperFormat::default(),
            &[0],
            Some(600),
        )
        .unwrap();
        let bytes = std::fs::read(&pdf).unwrap();

        assert!(bytes
            .windows(b"/Width 600".len())
            .any(|window| window == b"/Width 600"));
        std::fs::remove_file(pdf).unwrap();
    }

    #[cfg(feature = "openjph-experiment")]
    #[test]
    fn openjph_codestream_can_be_rendered_from_a_pdf() {
        use lopdf::content::{Content, Operation};
        use lopdf::{dictionary, Document, Object, Stream};
        use std::process::Command;

        if Command::new("pdftoppm").arg("-v").output().is_err() {
            return;
        }

        let directory = tempfile::tempdir().unwrap();
        let image = image::RgbImage::from_pixel(32, 24, image::Rgb([20, 40, 80]));
        let encoded = crate::openjph_experiment::encode_rgb(&image, 85).unwrap();
        let mut document = Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let page_id = document.new_object_id();
        let image_id = document.new_object_id();
        let content_id = document.new_object_id();
        document.objects.insert(
            image_id,
            Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 32,
                    "Height" => 24,
                    "ColorSpace" => "DeviceRGB",
                    "BitsPerComponent" => 8,
                    "Filter" => "JPXDecode",
                },
                encoded,
            )
            .into(),
        );
        let content = Content {
            operations: vec![
                Operation::new("q", vec![]),
                Operation::new(
                    "cm",
                    vec![
                        Object::Integer(32),
                        Object::Integer(0),
                        Object::Integer(0),
                        Object::Integer(24),
                        Object::Integer(0),
                        Object::Integer(0),
                    ],
                ),
                Operation::new("Do", vec![Object::Name(b"Im0".to_vec())]),
                Operation::new("Q", vec![]),
            ],
        }
        .encode()
        .unwrap();
        document
            .objects
            .insert(content_id, Stream::new(dictionary! {}, content).into());
        document.objects.insert(
            page_id,
            dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(32),
                    Object::Integer(24),
                ],
                "Resources" => dictionary! { "XObject" => dictionary! { "Im0" => image_id } },
                "Contents" => content_id,
            }
            .into(),
        );
        document.objects.insert(
            pages_id,
            dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => 1,
            }
            .into(),
        );
        let catalog_id = document.new_object_id();
        document.objects.insert(
            catalog_id,
            dictionary! { "Type" => "Catalog", "Pages" => pages_id }.into(),
        );
        document.trailer.set("Root", catalog_id);
        let pdf = directory.path().join("openjph.pdf");
        document.save(&pdf).unwrap();
        let rendered = directory.path().join("rendered");
        let output = Command::new("pdftoppm")
            .args(["-f", "1", "-singlefile", "-png"])
            .arg(&pdf)
            .arg(&rendered)
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "Poppler could not render OpenJPH PDF: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(rendered.with_extension("png").exists());
    }

    #[cfg(feature = "gui")]
    #[test]
    fn upload_document_posts_a_pdf_to_paperless() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::{Arc, Mutex};
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let request_sizes = Arc::new(Mutex::new(Vec::new()));
        let sizes_for_server = Arc::clone(&request_sizes);
        let server = thread::spawn(move || {
            for attempt in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = Vec::new();
                let mut buffer = [0_u8; 8192];
                let header_end = loop {
                    let count = stream.read(&mut buffer).unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&buffer[..count]);
                    if let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                        break end + 4;
                    }
                };
                let headers = String::from_utf8_lossy(&request[..header_end]).to_ascii_lowercase();
                let content_length = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .unwrap()
                    .trim()
                    .parse::<usize>()
                    .unwrap();
                while request.len() < header_end + content_length {
                    let count = stream.read(&mut buffer).unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&buffer[..count]);
                }
                sizes_for_server.lock().unwrap().push(content_length);
                assert!(headers.contains("post /api/documents/post_document/"));
                assert!(headers.contains("authorization: token test-token"));
                assert!(request
                    .windows(b"application/pdf".len())
                    .any(|window| { window == b"application/pdf" }));
                let response = if attempt == 0 {
                    b"HTTP/1.1 413 Payload Too Large\r\nContent-Length: 0\r\n\r\n".as_slice()
                } else {
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK".as_slice()
                };
                stream.write_all(response).unwrap();
            }
        });

        let directory = tempfile::tempdir().unwrap();
        let page = directory.path().join("page.png");
        image::RgbImage::from_fn(600, 600, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x * y) % 256) as u8])
        })
        .save(&page)
        .unwrap();
        let settings = AppSettings {
            paperless_url: format!("http://{address}"),
            paperless_token: "test-token".to_string(),
            ..AppSettings::default()
        };
        let stages = Arc::new(Mutex::new(Vec::new()));
        let stages_for_upload = Arc::clone(&stages);

        let result = tauri::async_runtime::block_on(upload_document_with_progress(
            vec![page.to_string_lossy().into_owned()],
            settings,
            "test document".to_string(),
            move |stage| stages_for_upload.lock().unwrap().push(stage.to_string()),
        ));

        let identifier = result.unwrap();
        assert_eq!(identifier.len(), 8);
        assert!(identifier.bytes().all(|byte| {
            byte.is_ascii_digit() || byte.is_ascii_uppercase() || byte.is_ascii_lowercase()
        }));
        let stages = stages.lock().unwrap();
        assert_eq!(stages[0], "Estimating PDF size…");
        assert!(stages[1].starts_with("Compressing pages at quality 85% (estimated PDF size: "));
        assert_eq!(
            &stages[2..],
            [
                "Uploading to Paperless…",
                "Upload was too large; recompressing at quality 55%…",
                "Uploading to Paperless…"
            ]
        );
        let sizes = request_sizes.lock().unwrap();
        assert_eq!(sizes.len(), 2);
        assert!(sizes[1] < sizes[0]);
        server.join().unwrap();
    }
}
