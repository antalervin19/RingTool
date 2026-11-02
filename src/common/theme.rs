use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

pub type Rgb = (u8, u8, u8);

#[derive(Clone, Debug)]
pub struct Theme {
    pub name: String,
    pub bg: Rgb,
    pub panel: Rgb,
    pub field: Rgb,
    pub edge: Rgb,
    pub text: Rgb,
    pub muted: Rgb,
    pub label: Rgb,
    pub button_text: Rgb,
    pub highlight: Rgb,
    pub accents: [Rgb; 4],
    pub ring_fill: Rgb,
    pub ring_text: Rgb,
}

impl Theme {
    pub fn text_rgba(&self) -> (u8, u8, u8, u8) {
        (self.text.0, self.text.1, self.text.2, 255)
    }

    pub fn muted_rgba(&self) -> (u8, u8, u8, u8) {
        (self.muted.0, self.muted.1, self.muted.2, 255)
    }

    pub fn label_rgba(&self) -> (u8, u8, u8, u8) {
        (self.label.0, self.label.1, self.label.2, 255)
    }

    pub fn button_text_rgba(&self) -> (u8, u8, u8, u8) {
        (self.button_text.0, self.button_text.1, self.button_text.2, 255)
    }

    fn to_text(&self) -> String {
        let hex = |c: Rgb| format!("#{:02X}{:02X}{:02X}", c.0, c.1, c.2);
        format!(
            "# RingTool theme. Edit the colours (#RRGGBB) and restart RingTool.\n\
             # Any missing colour falls back to the Midnight theme.\n\
             name = {}\n\n\
             background = {}\n\
             panel = {}\n\
             field = {}\n\
             border = {}\n\
             text = {}\n\
             muted = {}\n\
             label = {}\n\
             button_text = {}\n\
             highlight = {}\n\n\
             accent1 = {}\n\
             accent2 = {}\n\
             accent3 = {}\n\
             accent4 = {}\n\n\
             ring_fill = {}\n\
             ring_text = {}\n",
            self.name,
            hex(self.bg),
            hex(self.panel),
            hex(self.field),
            hex(self.edge),
            hex(self.text),
            hex(self.muted),
            hex(self.label),
            hex(self.button_text),
            hex(self.highlight),
            hex(self.accents[0]),
            hex(self.accents[1]),
            hex(self.accents[2]),
            hex(self.accents[3]),
            hex(self.ring_fill),
            hex(self.ring_text),
        )
    }

    fn parse(text: &str) -> Theme {
        let mut theme = midnight();

        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim().to_lowercase(), value.trim());

            if key == "name" {
                if !value.is_empty() {
                    theme.name = value.to_string();
                }
                continue;
            }

            let Some(color) = parse_hex(value) else {
                continue;
            };

            match key.as_str() {
                "background" => theme.bg = color,
                "panel" => theme.panel = color,
                "field" => theme.field = color,
                "border" => theme.edge = color,
                "text" => theme.text = color,
                "muted" => theme.muted = color,
                "label" => theme.label = color,
                "button_text" => theme.button_text = color,
                "highlight" => theme.highlight = color,
                "accent1" => theme.accents[0] = color,
                "accent2" => theme.accents[1] = color,
                "accent3" => theme.accents[2] = color,
                "accent4" => theme.accents[3] = color,
                "ring_fill" => theme.ring_fill = color,
                "ring_text" => theme.ring_text = color,
                _ => {}
            }
        }

        theme
    }
}

fn parse_hex(value: &str) -> Option<Rgb> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let channel = |range: std::ops::Range<usize>| u8::from_str_radix(&hex[range], 16).ok();
    Some((channel(0..2)?, channel(2..4)?, channel(4..6)?))
}

fn midnight() -> Theme {
    Theme {
        name: "Midnight".into(),
        bg: (14, 16, 21),
        panel: (30, 34, 43),
        field: (24, 27, 35),
        edge: (74, 81, 99),
        text: (248, 249, 252),
        muted: (176, 182, 196),
        label: (214, 218, 228),
        button_text: (14, 16, 21),
        highlight: (255, 255, 255),
        accents: [(168, 85, 247), (245, 158, 11), (45, 156, 219), (74, 222, 128)],
        ring_fill: (30, 34, 43),
        ring_text: (255, 255, 255),
    }
}

fn daylight() -> Theme {
    Theme {
        name: "Daylight".into(),
        bg: (243, 244, 247),
        panel: (255, 255, 255),
        field: (234, 236, 241),
        edge: (160, 166, 180),
        text: (20, 23, 31),
        muted: (88, 95, 112),
        label: (45, 50, 64),
        button_text: (255, 255, 255),
        highlight: (20, 23, 31),
        accents: [(124, 58, 237), (217, 119, 6), (2, 132, 199), (22, 163, 74)],
        ring_fill: (250, 250, 252),
        ring_text: (20, 23, 31),
    }
}

fn ember() -> Theme {
    Theme {
        name: "Ember".into(),
        bg: (20, 12, 12),
        panel: (42, 26, 24),
        field: (31, 19, 18),
        edge: (110, 74, 66),
        text: (255, 244, 238),
        muted: (205, 170, 156),
        label: (240, 210, 196),
        button_text: (24, 12, 10),
        highlight: (255, 238, 220),
        accents: [(244, 63, 94), (251, 146, 60), (250, 204, 21), (190, 242, 100)],
        ring_fill: (42, 26, 24),
        ring_text: (255, 244, 238),
    }
}

fn forest() -> Theme {
    Theme {
        name: "Forest".into(),
        bg: (11, 18, 15),
        panel: (22, 36, 30),
        field: (16, 27, 22),
        edge: (62, 92, 78),
        text: (240, 250, 244),
        muted: (160, 192, 175),
        label: (200, 226, 210),
        button_text: (8, 16, 12),
        highlight: (230, 255, 240),
        accents: [(52, 211, 153), (163, 230, 53), (45, 212, 191), (134, 239, 172)],
        ring_fill: (22, 36, 30),
        ring_text: (240, 250, 244),
    }
}

fn mono() -> Theme {
    Theme {
        name: "Mono".into(),
        bg: (10, 10, 10),
        panel: (28, 28, 28),
        field: (20, 20, 20),
        edge: (96, 96, 96),
        text: (245, 245, 245),
        muted: (170, 170, 170),
        label: (210, 210, 210),
        button_text: (10, 10, 10),
        highlight: (255, 255, 255),
        accents: [(235, 235, 235), (190, 190, 190), (145, 145, 145), (215, 215, 215)],
        ring_fill: (28, 28, 28),
        ring_text: (245, 245, 245),
    }
}

fn builtins() -> [(&'static str, Theme); 5] {
    [
        ("midnight", midnight()),
        ("daylight", daylight()),
        ("ember", ember()),
        ("forest", forest()),
        ("mono", mono()),
    ]
}

pub fn dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("RingTool").join("themes"))
}

fn write_builtins() {
    let Some(dir) = dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);

    for (stem, theme) in builtins() {
        let path = dir.join(format!("{stem}.ini"));
        if !path.exists() {
            let _ = std::fs::write(path, theme.to_text());
        }
    }
}

pub fn list() -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = Vec::new();

    if let Some(entries) = dir().and_then(|d| std::fs::read_dir(d).ok()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ini") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let name = std::fs::read_to_string(&path)
                .map(|text| Theme::parse(&text).name)
                .unwrap_or_else(|_| stem.to_string());
            found.push((stem.to_string(), name));
        }
    }

    if found.is_empty() {
        found.push(("midnight".into(), "Midnight".into()));
    }
    found.sort_by_key(|(stem, _)| stem.to_lowercase());
    found
}

static THEME: OnceLock<RwLock<Theme>> = OnceLock::new();

fn store() -> &'static RwLock<Theme> {
    THEME.get_or_init(|| RwLock::new(midnight()))
}

pub fn get() -> Theme {
    store().read().unwrap().clone()
}

pub fn load(stem: &str) {
    write_builtins();

    let theme = dir()
        .and_then(|d| std::fs::read_to_string(d.join(format!("{stem}.ini"))).ok())
        .map(|text| Theme::parse(&text))
        .unwrap_or_else(midnight);

    *store().write().unwrap() = theme;
}
