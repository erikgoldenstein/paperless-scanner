fn main() {
    #[cfg(feature = "gui")]
    paperless_scanner_lib::run();

    #[cfg(not(feature = "gui"))]
    eprintln!("Build with --features gui to run the desktop application.");
}
