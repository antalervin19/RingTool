
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct MenuOption {
    pub id: String,
    pub label: String,
    pub output_format: String,
}

#[derive(Clone, Debug)]
pub struct MenuCategory {
    pub id: String,
    pub label: String,
    pub color: (u8, u8, u8),
    pub options: Vec<MenuOption>,
}

#[derive(Clone, Debug)]
pub enum Menu {
    SingleRing {
        options: Vec<MenuOption>,
    },
    MultiRing {
        categories: Vec<MenuCategory>,
    },
}

fn make_options(prefix: &str, formats: &[&str], source: &str) -> Vec<MenuOption> {
    formats
        .iter()
        .filter(|format| **format != source && super::settings::format_enabled(format))
        .map(|format| MenuOption {
            id: format!("{prefix}_{format}"),
            label: format.to_uppercase(),
            output_format: (*format).to_string(),
        })
        .collect()
}

fn make_category(
    id: &str,
    label: &str,
    color: (u8, u8, u8),
    formats: &[&str],
    source: &str,
) -> MenuCategory {
    MenuCategory {
        id: id.into(),
        label: label.into(),
        color,
        options: make_options(id, formats, source),
    }
}

pub const IMAGE_FORMATS: [&str; 7] = ["png", "jpg", "webp", "bmp", "gif", "tiff", "ico"];
pub const AUDIO_FORMATS: [&str; 7] = ["mp3", "wav", "flac", "aac", "ogg", "opus", "m4a"];

fn extension_of(path: &Path) -> Option<String> {
    Some(path.extension()?.to_str()?.to_ascii_lowercase())
}

pub fn split_image_audio(paths: &[PathBuf]) -> Option<(&PathBuf, &PathBuf)> {
    if paths.len() != 2 {
        return None;
    }

    let kind = |p: &PathBuf| {
        let ext = extension_of(p)?;
        if IMAGE_FORMATS.contains(&ext.as_str()) {
            Some(true)
        } else if AUDIO_FORMATS.contains(&ext.as_str()) {
            Some(false)
        } else {
            None
        }
    };

    match (kind(&paths[0])?, kind(&paths[1])?) {
        (true, false) => Some((&paths[0], &paths[1])),
        (false, true) => Some((&paths[1], &paths[0])),
        _ => None,
    }
}

pub fn menu_for_url() -> Menu {
    let video = ["mp4", "mkv", "webm"];
    let audio = ["mp3", "m4a", "opus", "wav", "flac"];

    Menu::MultiRing {
        categories: vec![
            make_category("url_video", "Video", (45, 156, 219), &video, ""),
            make_category("url_audio", "Audio", (245, 158, 11), &audio, ""),
        ],
    }
}

pub fn menu_for_files(paths: &[PathBuf]) -> Option<Menu> {
    match paths {
        [] => None,
        [single] => menu_for_file(single),
        _ => {
            split_image_audio(paths)?;

            let options = ["mp4", "mkv", "mov"]
                .iter()
                .filter(|format| super::settings::format_enabled(format))
                .map(|format| MenuOption {
                    id: format!("slideshow_{format}"),
                    label: format.to_uppercase(),
                    output_format: (*format).to_string(),
                })
                .collect();

            Some(Menu::SingleRing { options })
        }
    }
}

pub fn menu_for_file(path: &Path) -> Option<Menu> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();

    let image_formats = ["png", "jpg", "webp", "bmp", "gif", "tiff", "ico"];
    let audio_formats = ["mp3", "wav", "flac", "aac", "ogg", "opus", "m4a"];
    let video_formats = ["mp4", "mkv", "webm", "mov", "avi", "gif"];
    let model_formats = ["glb", "gltf", "fbx", "obj", "stl", "dae", "3mf", "ply"];

    if image_formats.contains(&extension.as_str()) {
        return Some(Menu::SingleRing {
            options: make_options("image", &image_formats, &extension),
        });
    }

    if audio_formats.contains(&extension.as_str()) {
        return Some(Menu::SingleRing {
            options: make_options("audio", &audio_formats, &extension),
        });
    }

    if model_formats.contains(&extension.as_str()) {
        return Some(Menu::SingleRing {
            options: make_options("model", &model_formats, &extension),
        });
    }

    if video_formats.contains(&extension.as_str()) {
        let categories = vec![
            make_category(
                "images",
                "Images · First Frame",
                (168, 85, 247),
                &image_formats,
                "",
            ),
            make_category(
                "audio",
                "Audio · Extract",
                (245, 158, 11),
                &audio_formats,
                "",
            ),
            make_category(
                "video",
                "Video",
                (45, 156, 219),
                &video_formats,
                &extension,
            ),
        ];

        return Some(Menu::MultiRing { categories });
    }

    None
}

pub fn create_test_menu() -> Menu {
    menu_for_file(Path::new("test.mp4")).unwrap()
}
