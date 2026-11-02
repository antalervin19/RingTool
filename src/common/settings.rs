use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

#[derive(Clone, Debug)]
pub struct Settings {
    pub ffmpeg: String,
    pub blender: String,
    pub ytdlp: String,
    pub use_explorer_folder: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ffmpeg: crate::FFMPEG_PATH.into(),
            blender: crate::BLENDER_PATH.into(),
            ytdlp: crate::YTDLP_PATH.into(),
            use_explorer_folder: true,
        }
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

        match key.trim() {
            "ffmpeg" if !value.is_empty() => settings.ffmpeg = value,
            "blender" if !value.is_empty() => settings.blender = value,
            "ytdlp" if !value.is_empty() => settings.ytdlp = value,
            "use_explorer_folder" => settings.use_explorer_folder = value == "true",
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
        "ffmpeg={}\nblender={}\nytdlp={}\nuse_explorer_folder={}\n",
        snapshot.ffmpeg, snapshot.blender, snapshot.ytdlp, snapshot.use_explorer_folder
    );
    let _ = std::fs::write(path, text);
}
