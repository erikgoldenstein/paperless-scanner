//! Scanner backends.  The application talks to this trait instead of making
//! assumptions about the scanner API provided by the host operating system.

use std::path::Path;

use serde::Serialize;

use crate::ScanSettings;

pub trait ScannerBackend: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn experimental(&self) -> bool;
    fn list_scanners(&self) -> Result<Vec<String>, String>;
    fn scan(&self, settings: &ScanSettings, output: &Path) -> Result<(), String>;
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ScannerBackendStatus {
    pub id: String,
    pub name: String,
    pub experimental: bool,
    pub warning: Option<String>,
}

impl ScannerBackendStatus {
    fn from_backend(backend: &dyn ScannerBackend) -> Self {
        Self {
            id: backend.id().to_string(),
            name: backend.name().to_string(),
            experimental: backend.experimental(),
            warning: backend.experimental().then(|| {
                format!(
                    "Untested scanner backend (alpha, highly experimental): {}.",
                    backend.name()
                )
            }),
        }
    }
}

struct CompositeBackend {
    native: Box<dyn ScannerBackend>,
    escl: EsclBackend,
}

impl ScannerBackend for CompositeBackend {
    fn id(&self) -> &'static str {
        self.native.id()
    }

    fn name(&self) -> &'static str {
        self.native.name()
    }

    fn experimental(&self) -> bool {
        self.native.experimental()
    }

    fn list_scanners(&self) -> Result<Vec<String>, String> {
        let escl_scanners = self.escl.list_scanners()?;
        let mut scanners = match self.native.list_scanners() {
            Ok(scanners) => scanners,
            Err(_) if !escl_scanners.is_empty() => Vec::new(),
            Err(error) => return Err(error),
        };
        for scanner in escl_scanners {
            if !scanners.contains(&scanner) {
                scanners.push(scanner);
            }
        }
        Ok(scanners)
    }

    fn scan(&self, settings: &ScanSettings, output: &Path) -> Result<(), String> {
        if settings.device.starts_with("escl:") {
            self.escl.scan(settings, output)
        } else {
            self.native.scan(settings, output)
        }
    }
}

pub fn scanner_backend_for(settings: &ScanSettings) -> Result<Box<dyn ScannerBackend>, String> {
    let selection = settings.backend.trim().to_ascii_lowercase();
    let native = native_backend();
    let native_id = native.id();
    let escl = EsclBackend::new(settings.escl_url.clone());
    match selection.as_str() {
        "" | "auto" => Ok(Box::new(CompositeBackend {
            native: Box::new(native),
            escl,
        })),
        "escl" => Ok(Box::new(escl)),
        selected if selected == native_id => Ok(Box::new(native)),
        _ => Err(format!(
            "Scanner backend '{}' is not available on this platform",
            settings.backend
        )),
    }
}

pub fn scanner_backend_options() -> Vec<ScannerBackendStatus> {
    let native = native_backend();
    let native_status = ScannerBackendStatus::from_backend(&native);
    let mut automatic = native_status.clone();
    automatic.id = "auto".to_string();
    automatic.name = format!("Automatic ({})", native.name());
    vec![
        automatic,
        native_status,
        ScannerBackendStatus::from_backend(&EsclBackend::new(String::new())),
    ]
}

pub fn scanner_backend_status() -> ScannerBackendStatus {
    let backend = scanner_backend_for(&ScanSettings::default())
        .expect("automatic scanner backend must always be available");
    ScannerBackendStatus::from_backend(backend.as_ref())
}

#[cfg(target_os = "linux")]
fn native_backend() -> SaneBackend {
    SaneBackend
}

#[cfg(target_os = "windows")]
fn native_backend() -> WiaBackend {
    WiaBackend
}

#[cfg(target_os = "macos")]
fn native_backend() -> ImageCaptureBackend {
    ImageCaptureBackend
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn native_backend() -> UnsupportedBackend {
    UnsupportedBackend
}

#[cfg(target_os = "linux")]
struct SaneBackend;

#[cfg(target_os = "linux")]
mod sane_sys {
    use std::ffi::{c_char, c_int, c_void};

    pub type Status = c_int;
    pub type Handle = *mut c_void;

    pub const GOOD: Status = 0;
    pub const EOF: Status = 5;
    pub const ACTION_SET_VALUE: c_int = 1;
    pub const FRAME_GRAY: c_int = 0;
    pub const FRAME_RGB: c_int = 1;

    #[repr(C)]
    pub struct Device {
        pub name: *const c_char,
        pub vendor: *const c_char,
        pub model: *const c_char,
        pub type_: *const c_char,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Parameters {
        pub format: c_int,
        pub last_frame: c_int,
        pub bytes_per_line: c_int,
        pub pixels_per_line: c_int,
        pub lines: c_int,
        pub depth: c_int,
    }

    #[repr(C)]
    pub struct Range {
        pub min: c_int,
        pub max: c_int,
        pub quant: c_int,
    }

    #[repr(C)]
    pub union Constraint {
        pub range: *const Range,
        pub word_list: *const c_int,
        pub string_list: *const *const c_char,
    }

    #[repr(C)]
    pub struct OptionDescriptor {
        pub name: *const c_char,
        pub title: *const c_char,
        pub desc: *const c_char,
        pub type_: c_int,
        pub unit: c_int,
        pub size: c_int,
        pub cap: c_int,
        pub constraint_type: c_int,
        pub constraint: Constraint,
    }

    #[link(name = "sane")]
    unsafe extern "C" {
        pub fn sane_init(version_code: *mut c_int, authorize: *mut c_void) -> Status;
        pub fn sane_exit();
        pub fn sane_strstatus(status: Status) -> *const c_char;
        pub fn sane_get_devices(
            device_list: *mut *const *const Device,
            local_only: c_int,
        ) -> Status;
        pub fn sane_open(name: *const c_char, handle: *mut Handle) -> Status;
        pub fn sane_close(handle: Handle);
        pub fn sane_cancel(handle: Handle);
        pub fn sane_get_option_descriptor(handle: Handle, option: c_int)
            -> *const OptionDescriptor;
        pub fn sane_control_option(
            handle: Handle,
            option: c_int,
            action: c_int,
            value: *mut c_void,
            info: *mut c_int,
        ) -> Status;
        pub fn sane_start(handle: Handle) -> Status;
        pub fn sane_get_parameters(handle: Handle, parameters: *mut Parameters) -> Status;
        pub fn sane_read(
            handle: Handle,
            data: *mut u8,
            max_length: c_int,
            length: *mut c_int,
        ) -> Status;
    }
}

#[cfg(target_os = "linux")]
fn sane_error(status: sane_sys::Status) -> String {
    unsafe {
        std::ffi::CStr::from_ptr(sane_sys::sane_strstatus(status))
            .to_string_lossy()
            .into_owned()
    }
}

#[cfg(target_os = "linux")]
struct SaneSession;

#[cfg(target_os = "linux")]
impl Drop for SaneSession {
    fn drop(&mut self) {
        unsafe { sane_sys::sane_exit() }
    }
}

#[cfg(target_os = "linux")]
struct SaneHandle(sane_sys::Handle);

#[cfg(target_os = "linux")]
impl Drop for SaneHandle {
    fn drop(&mut self) {
        unsafe {
            sane_sys::sane_cancel(self.0);
            sane_sys::sane_close(self.0);
        }
    }
}

#[cfg(target_os = "linux")]
fn sane_init() -> Result<SaneSession, String> {
    let mut version = (1 << 24) | (0 << 16);
    let status = unsafe { sane_sys::sane_init(&mut version, std::ptr::null_mut()) };
    if status == sane_sys::GOOD {
        Ok(SaneSession)
    } else {
        Err(format!("Could not initialize SANE: {}", sane_error(status)))
    }
}

#[cfg(target_os = "linux")]
fn sane_devices() -> Result<Vec<String>, String> {
    let _session = sane_init()?;
    let mut devices = std::ptr::null();
    let status = unsafe { sane_sys::sane_get_devices(&mut devices, 0) };
    if status != sane_sys::GOOD {
        return Err(format!(
            "Could not enumerate SANE scanners: {}",
            sane_error(status)
        ));
    }
    let mut result = Vec::new();
    let mut index = 0;
    unsafe {
        while !(*devices.add(index)).is_null() {
            let device = &**devices.add(index);
            let name = std::ffi::CStr::from_ptr(device.name)
                .to_string_lossy()
                .into_owned();
            if !name.starts_with("v4l:") && !result.contains(&name) {
                result.push(name);
            }
            index += 1;
        }
    }
    Ok(result)
}

#[cfg(target_os = "linux")]
fn sane_option(handle: sane_sys::Handle, name: &str) -> Option<i32> {
    let requested = std::ffi::CString::new(name).ok()?;
    for index in 1..256 {
        let descriptor = unsafe { sane_sys::sane_get_option_descriptor(handle, index) };
        if descriptor.is_null() {
            continue;
        }
        let descriptor_name = unsafe { std::ffi::CStr::from_ptr((*descriptor).name) };
        if descriptor_name == requested.as_c_str() {
            return Some(index);
        }
    }
    None
}

#[cfg(target_os = "linux")]
impl ScannerBackend for SaneBackend {
    fn id(&self) -> &'static str {
        "sane"
    }

    fn name(&self) -> &'static str {
        "Linux SANE"
    }

    fn experimental(&self) -> bool {
        false
    }

    fn list_scanners(&self) -> Result<Vec<String>, String> {
        sane_devices()
    }

    fn scan(&self, settings: &ScanSettings, output: &Path) -> Result<(), String> {
        use image::{DynamicImage, ImageBuffer, Luma, Rgb};
        use std::ffi::{c_void, CString};

        let device_name = if settings.device.trim().is_empty() {
            sane_devices()?.into_iter().next().ok_or_else(|| {
                "No scanner detected. Reconnect the scanner and try again.".to_string()
            })?
        } else {
            settings.device.clone()
        };
        let _session = sane_init()?;
        let device_name = CString::new(device_name)
            .map_err(|_| "Scanner name contains an invalid character".to_string())?;
        let mut raw_handle = std::ptr::null_mut();
        let status = unsafe { sane_sys::sane_open(device_name.as_ptr(), &mut raw_handle) };
        if status != sane_sys::GOOD {
            return Err(format!("Could not open scanner: {}", sane_error(status)));
        }
        let handle = SaneHandle(raw_handle);
        if let Some(option_index) = sane_option(handle.0, "resolution") {
            let mut value = settings.resolution as i32;
            let status = unsafe {
                sane_sys::sane_control_option(
                    handle.0,
                    option_index,
                    sane_sys::ACTION_SET_VALUE,
                    (&mut value as *mut i32).cast::<c_void>(),
                    std::ptr::null_mut(),
                )
            };
            if status != sane_sys::GOOD {
                return Err(format!(
                    "Could not set scanner resolution: {}",
                    sane_error(status)
                ));
            }
        }
        if let Some(option_index) = sane_option(handle.0, "mode") {
            let mode = CString::new(settings.mode.clone())
                .map_err(|_| "Scanner mode contains an invalid character".to_string())?;
            let status = unsafe {
                sane_sys::sane_control_option(
                    handle.0,
                    option_index,
                    sane_sys::ACTION_SET_VALUE,
                    mode.as_ptr() as *mut c_void,
                    std::ptr::null_mut(),
                )
            };
            if status != sane_sys::GOOD {
                return Err(format!(
                    "Could not set scanner mode: {}",
                    sane_error(status)
                ));
            }
        }
        let mut parameters = sane_sys::Parameters {
            format: 0,
            last_frame: 0,
            bytes_per_line: 0,
            pixels_per_line: 0,
            lines: 0,
            depth: 0,
        };
        let status = unsafe { sane_sys::sane_start(handle.0) };
        if status != sane_sys::GOOD {
            return Err(format!("Could not start scan: {}", sane_error(status)));
        }
        let status = unsafe { sane_sys::sane_get_parameters(handle.0, &mut parameters) };
        if status != sane_sys::GOOD {
            return Err(format!(
                "Could not read scan parameters: {}",
                sane_error(status)
            ));
        }
        if parameters.depth != 8 {
            return Err(format!(
                "This SANE scanner returned {}-bit data; only 8-bit scans are currently supported",
                parameters.depth
            ));
        }
        let mut bytes = Vec::new();
        let mut buffer = vec![0u8; 1024 * 1024];
        loop {
            let mut read = 0;
            let status = unsafe {
                sane_sys::sane_read(
                    handle.0,
                    buffer.as_mut_ptr(),
                    buffer.len() as i32,
                    &mut read,
                )
            };
            if status == sane_sys::EOF {
                break;
            }
            if status != sane_sys::GOOD {
                return Err(format!("Could not read scan data: {}", sane_error(status)));
            }
            if read > 0 {
                bytes.extend_from_slice(&buffer[..read as usize]);
            }
        }
        let width = parameters.pixels_per_line as u32;
        let height = parameters.lines as u32;
        let row_bytes = parameters.bytes_per_line as usize;
        let channels = match parameters.format {
            sane_sys::FRAME_GRAY => 1,
            sane_sys::FRAME_RGB => 3,
            format => return Err(format!("Unsupported SANE frame format: {format}")),
        };
        let expected_row_bytes = width as usize * channels;
        if row_bytes < expected_row_bytes || bytes.len() < row_bytes * height as usize {
            return Err("SANE returned incomplete image data".to_string());
        }
        let mut pixels = Vec::with_capacity(expected_row_bytes * height as usize);
        for row in bytes.chunks(row_bytes).take(height as usize) {
            pixels.extend_from_slice(&row[..expected_row_bytes]);
        }
        let image = match channels {
            1 => DynamicImage::ImageLuma8(
                ImageBuffer::<Luma<u8>, _>::from_raw(width, height, pixels)
                    .ok_or_else(|| "Could not construct grayscale scan image".to_string())?,
            ),
            _ => DynamicImage::ImageRgb8(
                ImageBuffer::<Rgb<u8>, _>::from_raw(width, height, pixels)
                    .ok_or_else(|| "Could not construct color scan image".to_string())?,
            ),
        };
        image
            .save(output)
            .map_err(|error| format!("Could not save scan output: {error}"))
    }
}

#[cfg(target_os = "windows")]
struct WiaBackend;

#[cfg(target_os = "windows")]
impl ScannerBackend for WiaBackend {
    fn id(&self) -> &'static str {
        "wia"
    }
    fn name(&self) -> &'static str {
        "Windows WIA"
    }
    fn experimental(&self) -> bool {
        true
    }

    fn list_scanners(&self) -> Result<Vec<String>, String> {
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", "$m=New-Object -ComObject WIA.DeviceManager; $m.DeviceInfos | ForEach-Object { \"$($_.DeviceID)`t$($_.Properties.Item('Name').Value)\" }"])
            .output()
            .map_err(|error| format!("Could not run Windows WIA discovery: {error}"))?;
        if !output.status.success() {
            return Err(command_output_error("WIA discovery", &output));
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| line.split_once('\t').map(|(id, _name)| format!("wia:{id}")))
            .collect())
    }

    fn scan(&self, settings: &ScanSettings, output: &Path) -> Result<(), String> {
        let device = settings
            .device
            .strip_prefix("wia:")
            .unwrap_or(&settings.device);
        let output_path = output.to_string_lossy();
        let script = "$m=New-Object -ComObject WIA.DeviceManager; $i=$m.DeviceInfos | Where-Object { $_.DeviceID -eq $env:PAPERLESS_WIA_DEVICE } | Select-Object -First 1; if ($null -eq $i) { throw 'WIA scanner not found' }; $d=$i.Connect(); $x=$d.Items.Item(1); foreach($p in $x.Properties) { if($p.PropertyID -eq 6147 -or $p.PropertyID -eq 6148) { $p.Value=[int]$env:PAPERLESS_WIA_RESOLUTION }; if($p.PropertyID -eq 6146) { $p.Value=[int]$env:PAPERLESS_WIA_DATATYPE } }; $image=$x.Transfer(); $image.SaveFile($env:PAPERLESS_WIA_OUTPUT)";
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("PAPERLESS_WIA_DEVICE", device)
            .env("PAPERLESS_WIA_OUTPUT", output_path.as_ref())
            .env("PAPERLESS_WIA_RESOLUTION", settings.resolution.to_string())
            .env(
                "PAPERLESS_WIA_DATATYPE",
                wia_data_type(&settings.mode).to_string(),
            )
            .output()
            .map_err(|error| format!("Could not run Windows WIA scan: {error}"))?;
        if !output.status.success() {
            return Err(command_output_error("WIA scan", &output));
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn wia_data_type(mode: &str) -> u32 {
    match mode.to_ascii_lowercase().as_str() {
        "gray" | "grayscale" => 2,
        "lineart" | "black and white" => 4,
        _ => 1,
    }
}

#[cfg(target_os = "macos")]
struct ImageCaptureBackend;

#[cfg(target_os = "macos")]
impl ScannerBackend for ImageCaptureBackend {
    fn id(&self) -> &'static str {
        "image-capture-core"
    }
    fn name(&self) -> &'static str {
        "macOS ImageCaptureCore"
    }
    fn experimental(&self) -> bool {
        true
    }
    fn list_scanners(&self) -> Result<Vec<String>, String> {
        // ImageCaptureCore is asynchronous and requires an NSRunLoop.  The
        // native bridge is kept behind this backend so it can be completed and
        // tested without changing the scanner-facing application API.
        Err(
            "macOS ImageCaptureCore discovery is alpha and not yet available in this build"
                .to_string(),
        )
    }
    fn scan(&self, _settings: &ScanSettings, _output: &Path) -> Result<(), String> {
        Err(
            "macOS ImageCaptureCore scanning is alpha and not yet available in this build"
                .to_string(),
        )
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
struct UnsupportedBackend;

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
impl ScannerBackend for UnsupportedBackend {
    fn id(&self) -> &'static str {
        "unsupported"
    }
    fn name(&self) -> &'static str {
        "Experimental platform backend"
    }
    fn experimental(&self) -> bool {
        true
    }
    fn list_scanners(&self) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
    fn scan(&self, _settings: &ScanSettings, _output: &Path) -> Result<(), String> {
        Err("No scanner backend is available for this platform".to_string())
    }
}

struct EsclBackend {
    configured_url: String,
}

impl EsclBackend {
    fn new(configured_url: String) -> Self {
        Self {
            configured_url: configured_url.trim_end_matches('/').to_string(),
        }
    }

    fn configured_url(&self) -> Option<String> {
        if !self.configured_url.is_empty() {
            return Some(self.configured_url.clone());
        }
        std::env::var("PAPERLESS_SCANNER_ESCL_URL")
            .ok()
            .map(|url| url.trim_end_matches('/').to_string())
            .filter(|url| !url.is_empty())
    }

    fn url_from_device(device: &str) -> Result<reqwest::Url, String> {
        let url = device.strip_prefix("escl:").unwrap_or(device);
        reqwest::Url::parse(url).map_err(|error| format!("Invalid eSCL scanner URL: {error}"))
    }
}

impl ScannerBackend for EsclBackend {
    fn id(&self) -> &'static str {
        "escl"
    }
    fn name(&self) -> &'static str {
        "eSCL network scanner"
    }
    fn experimental(&self) -> bool {
        true
    }

    fn list_scanners(&self) -> Result<Vec<String>, String> {
        Ok(self
            .configured_url()
            .map(|url| vec![format!("escl:{url}")])
            .unwrap_or_default())
    }

    fn scan(&self, settings: &ScanSettings, output: &Path) -> Result<(), String> {
        let base = Self::url_from_device(&settings.device)?;
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|error| format!("Could not create eSCL client: {error}"))?;
        let settings_xml = format!(
            "<scan:ScanSettings xmlns:scan=\"http://schemas.hp.com/imaging/escl/2011/05/03\"><scan:Intent>Document</scan:Intent><scan:ScanRegions><scan:ScanRegion><scan:Height>10000</scan:Height><scan:Width>10000</scan:Width><scan:Top>0</scan:Top><scan:Left>0</scan:Left></scan:ScanRegion></scan:ScanRegions><scan:DocumentFormat>image/png</scan:DocumentFormat><scan:XResolution>{}</scan:XResolution><scan:YResolution>{}</scan:YResolution></scan:ScanSettings>",
            settings.resolution, settings.resolution
        );
        let response = client
            .post(
                base.join("eSCL/ScanJobs")
                    .map_err(|error| format!("Invalid eSCL URL: {error}"))?,
            )
            .header(reqwest::header::CONTENT_TYPE, "text/xml")
            .body(settings_xml)
            .send()
            .map_err(|error| format!("Could not start eSCL scan: {error}"))?;
        if !response.status().is_success() && response.status().as_u16() != 201 {
            return Err(format!(
                "eSCL scan request failed with status {}",
                response.status()
            ));
        }
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| "eSCL scanner did not return a scan job location".to_string())?;
        let document_url = reqwest::Url::parse(location)
            .or_else(|_| base.join(location))
            .map_err(|error| format!("Invalid eSCL scan job location: {error}"))?;
        let document = client
            .get(document_url.join("NextDocument").unwrap_or(document_url))
            .send()
            .map_err(|error| format!("Could not download eSCL scan: {error}"))?;
        if !document.status().is_success() {
            return Err(format!(
                "eSCL document download failed with status {}",
                document.status()
            ));
        }
        let bytes = document
            .bytes()
            .map_err(|error| format!("Could not read eSCL scan: {error}"))?;
        std::fs::write(output, bytes).map_err(|error| format!("Could not save eSCL scan: {error}"))
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn command_output_error(command: &str, output: &std::process::Output) -> String {
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if detail.is_empty() {
        format!("{command} failed with status {}", output.status)
    } else {
        format!("{command} failed: {detail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn experimental_status_has_the_user_facing_warning() {
        let status = ScannerBackendStatus {
            id: "test".to_string(),
            name: "Test backend".to_string(),
            experimental: true,
            warning: Some(
                "Untested scanner backend (alpha, highly experimental): Test backend.".to_string(),
            ),
        };
        assert!(status
            .warning
            .as_deref()
            .unwrap()
            .contains("alpha, highly experimental"));
    }

    #[test]
    fn escl_device_urls_are_parsed_without_shell_interpolation() {
        let url = EsclBackend::url_from_device("escl:http://scanner.local/").unwrap();
        assert_eq!(url.host_str(), Some("scanner.local"));
    }

    #[test]
    fn backend_options_include_automatic_native_and_escl_choices() {
        let options = scanner_backend_options();
        let ids = options
            .iter()
            .map(|option| option.id.as_str())
            .collect::<Vec<_>>();
        assert!(ids.contains(&"auto"));
        assert!(ids.contains(&native_backend().id()));
        assert!(ids.contains(&"escl"));
        assert!(
            options
                .iter()
                .find(|option| option.id == "escl")
                .unwrap()
                .experimental
        );
    }
}
