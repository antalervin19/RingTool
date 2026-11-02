#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
pub mod settings_window;

#[cfg(target_os = "linux")]
pub mod linux;