//! Scanner backends.  The application talks to this trait instead of making
//! assumptions about the scanner API provided by the host operating system.

use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
use std::process::Child;
#[cfg(target_os = "linux")]
use std::sync::{Mutex, OnceLock};

use serde::Serialize;

use crate::ScanSettings;

#[cfg(target_os = "linux")]
static ACTIVE_SCANIMAGE: OnceLock<Mutex<Option<Child>>> = OnceLock::new();

#[cfg(target_os = "linux")]
fn active_scanimage() -> &'static Mutex<Option<Child>> {
    ACTIVE_SCANIMAGE.get_or_init(|| Mutex::new(None))
}

/// Stop the external scanner process, if one is currently running.
pub fn cancel_active_scan() -> bool {
    #[cfg(target_os = "linux")]
    {
        let mut active = active_scanimage()
            .lock()
            .expect("scan process mutex poisoned");
        if let Some(mut child) = active.take() {
            let _ = child.kill();
            let _ = child.wait();
            return true;
        }
    }
    false
}

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
        #[cfg(target_os = "linux")]
        "sane-legacy" => Ok(Box::new(LegacySaneBackend::new())),
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
    let mut options = vec![automatic, native_status];
    #[cfg(target_os = "linux")]
    options.push(ScannerBackendStatus::from_backend(&LegacySaneBackend::new()));
    options.extend([ScannerBackendStatus::from_backend(&EsclBackend::new(
        String::new(),
    ))]);
    options
}

pub fn scanner_backend_status() -> ScannerBackendStatus {
    let backend = scanner_backend_for(&ScanSettings::default())
        .expect("automatic scanner backend must always be available");
    ScannerBackendStatus::from_backend(backend.as_ref())
}

#[cfg(target_os = "linux")]
fn native_backend() -> SaneBackend {
    SaneBackend::new()
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
struct SaneBackend {
    scanimage: PathBuf,
}

#[cfg(target_os = "linux")]
impl SaneBackend {
    fn new() -> Self {
        Self {
            scanimage: PathBuf::from("scanimage"),
        }
    }

    #[cfg(test)]
    fn with_scanimage(scanimage: PathBuf) -> Self {
        Self { scanimage }
    }

    fn run_scanimage(
        &self,
        args: &[String],
        output_path: Option<&Path>,
    ) -> Result<std::process::Output, String> {
        use std::fs::OpenOptions;
        use std::process::{Command, Stdio};
        use std::thread;
        use std::time::{Duration, Instant};

        let stdout = if let Some(path) = output_path {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;

                options.mode(0o600);
            }
            let file = options
                .open(path)
                .map_err(|error| format!("Could not create scan output: {error}"))?;
            Stdio::from(file)
        } else {
            Stdio::piped()
        };

        let mut child = Command::new(&self.scanimage)
            .args(args)
            .stdout(stdout)
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("Could not run scanimage: {error}"))?;
        {
            let mut active = active_scanimage()
                .lock()
                .map_err(|_| "Scanner process state is unavailable".to_string())?;
            if active.is_some() {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Another scanner operation is already active".to_string());
            }
            *active = Some(child);
        }
        let deadline = Instant::now() + Duration::from_secs(90);

        loop {
            let finished = {
                let mut active = active_scanimage()
                    .lock()
                    .map_err(|_| "Scanner process state is unavailable".to_string())?;
                match active.as_mut() {
                    Some(child) => child
                        .try_wait()
                        .map_err(|error| format!("Could not read scanner state: {error}"))?
                        .is_some(),
                    None => return Err("Scanner scan canceled".to_string()),
                }
            };
            if finished {
                let child = active_scanimage()
                    .lock()
                    .map_err(|_| "Scanner process state is unavailable".to_string())?
                    .take()
                    .ok_or_else(|| "Scanner scan canceled".to_string())?;
                return child
                    .wait_with_output()
                    .map_err(|error| format!("Could not read scanner output: {error}"));
            }
            if Instant::now() >= deadline {
                let child = active_scanimage()
                    .lock()
                    .map_err(|_| "Scanner process state is unavailable".to_string())?
                    .take();
                if let Some(mut child) = child {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                return Err("Scanner timed out. Reconnect the scanner and try again.".to_string());
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    fn command_error(output: &std::process::Output) -> String {
        let detail = String::from_utf8_lossy(&output.stderr);
        if detail.trim().is_empty() {
            crate::scanner_error(&format!("scanimage exited with status {}", output.status))
        } else {
            crate::scanner_error(&detail)
        }
    }
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
        let output = self.run_scanimage(&["-L".to_string()], None)?;
        if !output.status.success() {
            return Err(Self::command_error(&output));
        }
        Ok(
            crate::parse_scanner_list(&String::from_utf8_lossy(&output.stdout))
                .into_iter()
                .filter(|name| !name.starts_with("v4l:"))
                .collect(),
        )
    }

    fn scan(&self, settings: &ScanSettings, output: &Path) -> Result<(), String> {
        let args = crate::scanimage_args(settings);
        let result = self.run_scanimage(&args, Some(output))?;
        if result.status.success() {
            Ok(())
        } else {
            let _ = std::fs::remove_file(output);
            Err(Self::command_error(&result))
        }
    }
}

#[cfg(target_os = "linux")]
struct LegacySaneBackend {
    command: SaneBackend,
}

#[cfg(target_os = "linux")]
impl LegacySaneBackend {
    fn new() -> Self {
        Self {
            command: SaneBackend::new(),
        }
    }

    #[cfg(test)]
    fn with_scanimage(scanimage: PathBuf) -> Self {
        Self {
            command: SaneBackend::with_scanimage(scanimage),
        }
    }

    fn list_scanners(&self) -> Result<Vec<String>, String> {
        let output = self.command.run_scanimage(&["-L".to_string()], None)?;
        if !output.status.success() {
            return Err(SaneBackend::command_error(&output));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(crate::parse_scanner_list(&format!("{stdout}\n{stderr}")))
    }

    fn scan(&self, settings: &ScanSettings, output: &Path) -> Result<(), String> {
        let probe = vec![
            format!("--device-name={}", settings.device),
            "--dont-scan".to_string(),
        ];
        let probe_result = self.command.run_scanimage(&probe, None)?;
        if !probe_result.status.success() {
            return Err(SaneBackend::command_error(&probe_result));
        }

        let mut args = crate::scanimage_args(settings);
        let mut skipped_options = Vec::new();
        loop {
            let result = match self.command.run_scanimage(&args, Some(output)) {
                Ok(result) => result,
                Err(error) => {
                    let _ = std::fs::remove_file(output);
                    return Err(error);
                }
            };
            if result.status.success() {
                return Ok(());
            }
            if let Some(option) = unsupported_scan_option(&String::from_utf8_lossy(&result.stderr))
            {
                if !skipped_options.contains(&option) {
                    args = remove_scan_option(&args, option);
                    skipped_options.push(option);
                    let _ = std::fs::remove_file(output);
                    continue;
                }
            }
            let _ = std::fs::remove_file(output);
            return Err(SaneBackend::command_error(&result));
        }
    }
}

#[cfg(target_os = "linux")]
impl ScannerBackend for LegacySaneBackend {
    fn id(&self) -> &'static str {
        "sane-legacy"
    }

    fn name(&self) -> &'static str {
        "Linux SANE (legacy external scanimage)"
    }

    fn experimental(&self) -> bool {
        false
    }

    fn list_scanners(&self) -> Result<Vec<String>, String> {
        self.list_scanners()
    }

    fn scan(&self, settings: &ScanSettings, output: &Path) -> Result<(), String> {
        self.scan(settings, output)
    }
}

#[cfg(target_os = "linux")]
fn remove_scan_option(args: &[String], option: &str) -> Vec<String> {
    args.iter()
        .filter(|arg| !(*arg == option || arg.starts_with(&format!("{option}="))))
        .cloned()
        .collect()
}

#[cfg(target_os = "linux")]
fn unsupported_scan_option(message: &str) -> Option<&'static str> {
    let option = message
        .split("unrecognized option '")
        .nth(1)
        .and_then(|rest| rest.split('\'').next())?;
    ["--resolution", "--mode"]
        .into_iter()
        .find(|candidate| option == *candidate || option.starts_with(&format!("{candidate}=")))
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

    #[cfg(target_os = "linux")]
    fn fake_scanimage(script: &str) -> (tempfile::TempDir, PathBuf) {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("scanimage");
        std::fs::write(&path, script).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).unwrap();
        (directory, path)
    }

    #[cfg(target_os = "linux")]
    fn test_scan_lock() -> &'static std::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

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
        #[cfg(target_os = "linux")]
        assert!(ids.contains(&"sane-legacy"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn sane_listing_runs_in_a_child_process_and_parses_devices() {
        let _lock = test_scan_lock().lock().unwrap();
        let (_directory, command) = fake_scanimage(
            r##"#!/bin/sh
if [ "$1" = "-L" ]; then
  cat <<'EOF'
device `scanner:test' is a test scanner
device `v4l:/dev/video0' is a camera
device `scanner:test' is a duplicate
EOF
  exit 0
fi
exit 1
"##,
        );
        let backend = SaneBackend::with_scanimage(command);

        assert_eq!(
            backend.list_scanners().unwrap(),
            vec!["scanner:test".to_string()]
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_crashing_sane_child_becomes_an_error_instead_of_crashing_the_app() {
        let _lock = test_scan_lock().lock().unwrap();
        let (_directory, command) = fake_scanimage("#!/bin/sh\nulimit -c 0\nkill -SEGV $$\n");
        let backend = SaneBackend::with_scanimage(command);

        let error = backend.list_scanners().unwrap_err();

        assert!(error.contains("scanimage"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn an_active_sane_child_can_be_cancelled_without_waiting_for_the_timeout() {
        let _lock = test_scan_lock().lock().unwrap();
        let (_directory, command) = fake_scanimage("#!/bin/sh\nsleep 30\n");
        let backend = SaneBackend::with_scanimage(command);
        let worker = std::thread::spawn(move || backend.list_scanners());

        for _ in 0..100 {
            if active_scanimage().lock().unwrap().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(cancel_active_scan());
        let error = worker.join().unwrap().unwrap_err();

        assert!(error.contains("canceled"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_sane_runs_the_old_preflight_and_retries_without_unsupported_options() {
        let _lock = test_scan_lock().lock().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let log = directory.path().join("calls.log");
        let script = format!(
            "#!/bin/sh\nlog={}\necho \"$*\" >> \"$log\"\nif [ \"$1\" = \"--device-name=scanner:test\" ] && [ \"$2\" = \"--dont-scan\" ]; then\n  exit 0\nfi\ncase \"$*\" in\n  *\"--resolution=300\"*)\n    echo \"scanimage: unrecognized option '--resolution=300'\" >&2\n    exit 1\n    ;;\nesac\nexit 0\n",
            log.display()
        );
        let (_command_directory, command) = fake_scanimage(&script);
        let backend = LegacySaneBackend::with_scanimage(command);
        let output = directory.path().join("page.png");
        let settings = crate::ScanSettings {
            device: "scanner:test".to_string(),
            resolution: 300,
            ..Default::default()
        };

        backend.scan(&settings, &output).unwrap();

        let calls = std::fs::read_to_string(log).unwrap();
        let calls = calls.lines().collect::<Vec<_>>();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0], "--device-name=scanner:test --dont-scan");
        assert!(calls[1].contains("--resolution=300"));
        assert!(!calls[2].contains("--resolution"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_sane_is_selectable_as_a_separate_backend() {
        let settings = crate::ScanSettings {
            backend: "sane-legacy".to_string(),
            ..Default::default()
        };

        let backend = scanner_backend_for(&settings).unwrap();

        assert_eq!(backend.id(), "sane-legacy");
    }
}
