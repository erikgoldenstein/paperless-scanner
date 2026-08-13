fn main() {
    #[cfg(feature = "gui")]
    {
        #[cfg(target_os = "linux")]
        {
            // WebKitGTK's DMA-BUF renderer can crash in Mesa EGL teardown on
            // some Wayland/driver combinations. Set the upstream workaround
            // before Tauri creates the WebView, while allowing an explicit
            // deployment override.
            if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
                std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
            }
        }

        paperless_scanner_lib::run();
    }

    #[cfg(not(feature = "gui"))]
    eprintln!("Build with --features gui to run the desktop application.");
}
