#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    use_linux_nvidia_webkit_workaround();

    ryo_wallet_next::run();
}

#[cfg(target_os = "linux")]
fn use_linux_nvidia_webkit_workaround() {
    // WebKitGTK's DMABUF renderer can display an empty window or trigger
    // Wayland protocol error 71 with the NVIDIA driver. Respect an explicit
    // user setting and leave other Linux graphics stacks unchanged.
    if std::path::Path::new("/proc/driver/nvidia/version").is_file()
        && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none()
    {
        // SAFETY: main runs before Tauri, WebKitGTK, or service threads start.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
}
