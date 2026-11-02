use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

pub const FORMAT_GROUPS: [(&str, &[&str]); 4] = [
    ("IMAGES", &["png", "jpg", "webp", "bmp", "gif", "tiff", "ico"]),
    ("AUDIO", &["mp3", "wav", "flac", "aac", "ogg", "opus", "m4a"]),
    ("VIDEO", &["mp4", "mkv", "webm", "mov", "avi"]),
    ("3D MODELS", &["glb", "gltf", "fbx", "obj", "stl", "dae", "3mf", "ply"]),
];

pub const AUDIO_KBPS: [u32; 7] = [64, 96, 128, 160, 192, 256, 320];
pub const VIDEO_CRF: [u32; 7] = [16, 18, 20, 23, 26, 28, 32];
pub const IMAGE_QUALITY: [u32; 8] = [50, 60, 70, 80, 85, 90, 95, 100];
pub const ICO_SIZES: [u32; 6] = [16, 32, 48, 64, 128, 256];

#[derive(Clone, Debug)]
pub struct Settings {
    pub ffmpeg: String,
    pub blender: String,
    pub ytdlp: String,
    pub use_explorer_folder: bool,
    pub disabled_formats: Vec<String>,
    pub audio_kbps: u32,
    pub video_crf: u32,
    pub image_quality: u32,
    pub ico_size: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ffmpeg: crate::FFMPEG_PATH.into(),
            blender: crate::BLENDER_PATH.into(),
            ytdlp: crate::YTDLP_PATH.into(),
            use_explorer_folder: true,
            disabled_formats: Vec::new(),
            audio_kbps: 192,
            video_crf: 23,
            image_quality: 90,
            ico_size: 256,
        }
    }
}

impl Settings {
    pub fn format_enabled(&self, format: &str) -> bool {
        !self.disabled_formats.iter().any(|f| f == format)
    }
}

static SETTINGS: OnceLock<RwLock<Settings>> = OnceLock::new();

fn store() -> &'static RwLock<Settings> {
    SETTINGS.get_or_init(|| RwLock::new(Settings::default()))
}

fn config_path() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("RingTool").join("settings.ini"))
}

pub fn get() -> Settings {
    store().read().unwrap().clone()
}

pub fn format_enabled(format: &str) -> bool {
    store().read().unwrap().format_enabled(format)
}

pub fn load() {
    let Some(text) = config_path().and_then(|p| std::fs::read_to_string(p).ok()) else {
        return;
    };

    let mut settings = Settings::default();

    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().to_string();
        let number = value.parse::<u32>().ok();

        match key.trim() {
            "ffmpeg" if !value.is_empty() => settings.ffmpeg = value,
            "blender" if !value.is_empty() => settings.blender = value,
            "ytdlp" if !value.is_empty() => settings.ytdlp = value,
            "use_explorer_folder" => settings.use_explorer_folder = value == "true",
            "disabled_formats" => {
                settings.disabled_formats = value
                    .split(',')
                    .map(str::trim)
                    .filter(|f| !f.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            "audio_kbps" => settings.audio_kbps = number.unwrap_or(settings.audio_kbps),
            "video_crf" => settings.video_crf = number.unwrap_or(settings.video_crf),
            "image_quality" => settings.image_quality = number.unwrap_or(settings.image_quality),
            "ico_size" => settings.ico_size = number.unwrap_or(settings.ico_size),
            _ => {}
        }
    }

    *store().write().unwrap() = settings;
}

pub fn update(change: impl FnOnce(&mut Settings)) {
    let snapshot = {
        let mut guard = store().write().unwrap();
        change(&mut guard);
        guard.clone()
    };

    let Some(path) = config_path() else {
        return;
    };

    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }

    let text = format!(
        "ffmpeg={}\nblender={}\nytdlp={}\nuse_explorer_folder={}\ndisabled_formats={}\naudio_kbps={}\nvideo_crf={}\nimage_quality={}\nico_size={}\n",
        snapshot.ffmpeg,
        snapshot.blender,
        snapshot.ytdlp,
        snapshot.use_explorer_folder,
        snapshot.disabled_formats.join(","),
        snapshot.audio_kbps,
        snapshot.video_crf,
        snapshot.image_quality,
        snapshot.ico_size,
    );
    let _ = std::fs::write(path, text);
}
