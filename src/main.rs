#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod common;
mod platform;

use common::menu::create_test_menu;
use common::render::render_menu;
use tray_icon::{TrayIcon, TrayIconBuilder, Icon};
use tray_icon::menu::{Menu, MenuItem, MenuEvent};

pub const FFMPEG_PATH: &str = r"C:\Users\antalervin19\AppData\Local\Microsoft\WinGet\Packages\Gyan.FFmpeg.Shared_Microsoft.Winget.Source_8wekyb3d8bbwe\ffmpeg-9.0.2-full_build-shared\bin\ffmpeg.exe";
pub const BLENDER_PATH: &str = r"D:\SteamLibrary\steamapps\common\Blender\blender.exe";
pub const YTDLP_PATH: &str = r"C:\yt-dlp\yt-dlp.exe";

fn main() {
    common::settings::load();

    platform::windows::initialize_ole().expect("Failed to initialize OLE");
    platform::windows::register_app_id();

    if let Some(version) = common::update::updated_version() {
        platform::windows::notify_updated(&version);
    }
    common::update::spawn_check();

    let menu = create_test_menu();

    platform::windows::set_menu(menu.clone());

    let hwnd = platform::windows::create_window().unwrap();
    let pm = render_menu(&menu, None, None);

    platform::windows::present_pixmap(hwnd, &pm).unwrap();
    platform::windows::install_mouse_hook().unwrap();
    platform::windows::install_keyboard_hook().unwrap();

    let _tray_icon = create_tray_icon();

    platform::windows::run_message_loop();
    platform::windows::uninitialize_ole();
}

fn create_tray_icon() -> TrayIcon {
    let image = image::load_from_memory(
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/icon.ico"))
    )
    .expect("Failed to load icon.ico")
    .resize_exact(32, 32, image::imageops::FilterType::Lanczos3)
    .into_rgba8();

    let (width, height) = image.dimensions();
    let icon = Icon::from_rgba(image.into_raw(), width, height)
        .expect("Failed to create tray icon");

    let menu = Menu::new();

    let settings_item = MenuItem::new("Settings", true, None);
    let quit_item = MenuItem::new("Quit RingTool", true, None);

    menu.append(&settings_item).unwrap();
    menu.append(&quit_item).unwrap();

    let settings_id = settings_item.id().clone();
    let quit_id = quit_item.id().clone();

    MenuEvent::set_event_handler(Some(move |event: tray_icon::menu::MenuEvent| {
        if event.id == settings_id {
            platform::settings_window::open();
        } else if event.id == quit_id {
            platform::windows::request_quit();
        }
    }));

    TrayIconBuilder::new()
        .with_icon(icon)
        .with_tooltip("RingTool")
        .with_menu(Box::new(menu))
        .build()
        .expect("Failed to create tray icon")
}