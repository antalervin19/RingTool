use crate::common::render::{draw_text, text_width};
use crate::common::settings;
use std::mem::size_of;
use std::sync::atomic::{AtomicI32, AtomicIsize, AtomicUsize, Ordering};
use tiny_skia::{FillRule, Paint, Path, PathBuilder, Pixmap, Stroke, Transform};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SetFocus, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, PWSTR, w};

const WM_MOUSELEAVE: u32 = 0x02A3;
const W: i32 = 560;
const H: i32 = 480;
const TITLE_H: f32 = 64.0;

const BG: (u8, u8, u8) = (14, 16, 21);
const PANEL: (u8, u8, u8) = (30, 34, 43);
const FIELD: (u8, u8, u8) = (24, 27, 35);
const PURPLE: (u8, u8, u8) = (168, 85, 247);
const AMBER: (u8, u8, u8) = (245, 158, 11);
const BLUE: (u8, u8, u8) = (45, 156, 219);
const TEXT: (u8, u8, u8, u8) = (248, 249, 252, 255);
const MUTED: (u8, u8, u8, u8) = (176, 182, 196, 255);
const LABEL: (u8, u8, u8, u8) = (214, 218, 228, 255);
const DARK_TEXT: (u8, u8, u8, u8) = (14, 16, 21, 255);
const EDGE: (u8, u8, u8) = (74, 81, 99);

static SETTINGS_HWND: AtomicIsize = AtomicIsize::new(0);
static HOVER: AtomicI32 = AtomicI32::new(-1);
static TRACKING: AtomicI32 = AtomicI32::new(0);

#[derive(Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Rect {
    fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Action {
    Close,
    Tab(usize),
    Browse(usize),
    Toggle,
    Format(usize, usize),
    Step(usize, i32),
}

static TAB: AtomicUsize = AtomicUsize::new(0);

const TABS: [(&str, (u8, u8, u8)); 3] = [("General", PURPLE), ("Formats", AMBER), ("Quality", BLUE)];
const TAB_H: f32 = 44.0;
const CONTENT_Y: f32 = TITLE_H + TAB_H + 16.0;
const GREEN: (u8, u8, u8) = (74, 222, 128);
const GROUP_ACCENTS: [(u8, u8, u8); 4] = [PURPLE, AMBER, BLUE, GREEN];
const CHIP_W: f32 = 54.0;

const TOOLS: [(&str, (u8, u8, u8)); 3] = [
    ("FFmpeg", PURPLE),
    ("Blender", AMBER),
    ("yt-dlp", BLUE),
];

const QUALITY_ROWS: [(&str, &str); 4] = [
    ("Audio bitrate", "MP3, AAC, OGG, OPUS and M4A"),
    ("Video quality", "Lower CRF means better quality and bigger files"),
    ("Image quality", "JPG and WebP"),
    ("ICO size", "Largest icon dimension"),
];

fn row_y(i: usize) -> f32 {
    CONTENT_Y + i as f32 * 84.0
}

fn chip_rect(group: usize, index: usize) -> Rect {
    Rect {
        x: 28.0 + index as f32 * (CHIP_W + 8.0),
        y: CONTENT_Y + group as f32 * 78.0 + 24.0,
        w: CHIP_W,
        h: 32.0,
    }
}

fn quality_card(row: usize) -> Rect {
    Rect { x: 28.0, y: CONTENT_Y + row as f32 * 76.0, w: W as f32 - 56.0, h: 64.0 }
}

fn step_rects(row: usize) -> (Rect, Rect) {
    let card = quality_card(row);
    let plus = Rect { x: card.x + card.w - 14.0 - 36.0, y: card.y + 14.0, w: 36.0, h: 36.0 };
    let minus = Rect { x: plus.x - 84.0 - 36.0, y: plus.y, w: 36.0, h: 36.0 };
    (minus, plus)
}

fn quality_list(kind: usize) -> &'static [u32] {
    match kind {
        0 => &settings::AUDIO_KBPS[..],
        1 => &settings::VIDEO_CRF[..],
        2 => &settings::IMAGE_QUALITY[..],
        _ => &settings::ICO_SIZES[..],
    }
}

fn quality_value(s: &settings::Settings, kind: usize) -> u32 {
    match kind {
        0 => s.audio_kbps,
        1 => s.video_crf,
        2 => s.image_quality,
        _ => s.ico_size,
    }
}

fn quality_text(kind: usize, value: u32) -> String {
    match kind {
        0 => format!("{value} kbps"),
        1 => format!("CRF {value}"),
        2 => format!("{value}%"),
        _ => format!("{value} px"),
    }
}

fn quality_index(kind: usize, value: u32) -> usize {
    quality_list(kind)
        .iter()
        .position(|v| *v >= value)
        .unwrap_or(quality_list(kind).len() - 1)
}

fn step_quality(kind: usize, dir: i32) {
    settings::update(|s| {
        let list = quality_list(kind);
        let index = quality_index(kind, quality_value(s, kind)) as i32 + dir;
        let value = list[index.clamp(0, list.len() as i32 - 1) as usize];

        match kind {
            0 => s.audio_kbps = value,
            1 => s.video_crf = value,
            2 => s.image_quality = value,
            _ => s.ico_size = value,
        }
    });
}

fn toggle_format(group: usize, index: usize) {
    let name = settings::FORMAT_GROUPS[group].1[index];

    settings::update(|s| {
        if let Some(pos) = s.disabled_formats.iter().position(|f| f == name) {
            s.disabled_formats.remove(pos);
        } else {
            s.disabled_formats.push(name.to_string());
        }
    });
}

fn layout() -> Vec<(Rect, Action)> {
    let mut items = vec![(
        Rect { x: W as f32 - 52.0, y: 16.0, w: 32.0, h: 32.0 },
        Action::Close,
    )];

    for i in 0..TABS.len() {
        items.push((
            Rect { x: i as f32 * W as f32 / 3.0, y: TITLE_H, w: W as f32 / 3.0, h: TAB_H },
            Action::Tab(i),
        ));
    }

    match TAB.load(Ordering::Relaxed) {
        0 => {
            for i in 0..TOOLS.len() {
                items.push((
                    Rect { x: W as f32 - 28.0 - 96.0, y: row_y(i) + 28.0, w: 96.0, h: 40.0 },
                    Action::Browse(i),
                ));
            }

            items.push((
                Rect { x: 28.0, y: row_y(3) + 4.0, w: W as f32 - 56.0, h: 52.0 },
                Action::Toggle,
            ));
        }

        1 => {
            for (group, (_, formats)) in settings::FORMAT_GROUPS.iter().enumerate() {
                for index in 0..formats.len() {
                    items.push((chip_rect(group, index), Action::Format(group, index)));
                }
            }
        }

        _ => {
            for row in 0..QUALITY_ROWS.len() {
                let (minus, plus) = step_rects(row);
                items.push((minus, Action::Step(row, -1)));
                items.push((plus, Action::Step(row, 1)));
            }
        }
    }

    items
}

fn round_rect(r: Rect, radius: f32) -> Path {
    let (x, y, w, h) = (r.x, r.y, r.w, r.h);
    let k = radius.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + k, y);
    pb.line_to(x + w - k, y);
    pb.quad_to(x + w, y, x + w, y + k);
    pb.line_to(x + w, y + h - k);
    pb.quad_to(x + w, y + h, x + w - k, y + h);
    pb.line_to(x + k, y + h);
    pb.quad_to(x, y + h, x, y + h - k);
    pb.line_to(x, y + k);
    pb.quad_to(x, y, x + k, y);
    pb.close();
    pb.finish().unwrap()
}

fn fill(pm: &mut Pixmap, path: &Path, color: (u8, u8, u8), alpha: u8) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.0, color.1, color.2, alpha);
    paint.anti_alias = true;
    pm.fill_path(path, &paint, FillRule::Winding, Transform::identity(), None);
}

fn stroke(pm: &mut Pixmap, path: &Path, color: (u8, u8, u8), alpha: u8, width: f32) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.0, color.1, color.2, alpha);
    paint.anti_alias = true;
    let stroke = Stroke { width, ..Stroke::default() };
    pm.stroke_path(path, &paint, &stroke, Transform::identity(), None);
}

fn bold(pm: &mut Pixmap, text: &str, x: f32, y: f32, size: f32, color: (u8, u8, u8, u8)) {
    draw_text(pm, text, x, y, size, color);
    draw_text(pm, text, x + 0.7, y, size, color);
}

fn fit_text(text: &str, size: f32, max_width: f32) -> String {
    if text_width(text, size) <= max_width {
        return text.to_string();
    }

    let chars: Vec<char> = text.chars().collect();
    for skip in 1..chars.len() {
        let candidate: String = std::iter::once('…')
            .chain(chars[skip..].iter().copied())
            .collect();
        if text_width(&candidate, size) <= max_width {
            return candidate;
        }
    }
    text.to_string()
}

fn render(hover: i32) -> Pixmap {
    let mut pm = Pixmap::new(W as u32, H as u32).unwrap();
    let full = Rect { x: 0.0, y: 0.0, w: W as f32, h: H as f32 };
    fill(&mut pm, &round_rect(full, 0.0), BG, 255);

    let bar = Rect { x: 0.0, y: 0.0, w: W as f32, h: TITLE_H };
    fill(&mut pm, &round_rect(bar, 0.0), PANEL, 255);
    bold(&mut pm, "RingTool", 28.0, 32.0, 22.0, TEXT);
    let title_w = text_width("RingTool", 22.0);
    draw_text(&mut pm, "Settings", 28.0 + title_w + 10.0, 33.0, 15.0, MUTED);

    let tab = TAB.load(Ordering::Relaxed);
    let tab_bar = Rect { x: 0.0, y: TITLE_H, w: W as f32, h: TAB_H };
    fill(&mut pm, &round_rect(tab_bar, 0.0), PANEL, 255);
    let rule = Rect { x: 0.0, y: TITLE_H + TAB_H - 1.0, w: W as f32, h: 1.0 };
    fill(&mut pm, &round_rect(rule, 0.0), EDGE, 255);

    if tab == 1 {
        for (group, (name, _)) in settings::FORMAT_GROUPS.iter().enumerate() {
            let y = CONTENT_Y + group as f32 * 78.0 + 6.0;
            bold(&mut pm, name, 28.0, y, 12.0, LABEL);
        }
    }

    let items = layout();
    let settings = settings::get();
    let paths = [&settings.ffmpeg, &settings.blender, &settings.ytdlp];

    for (idx, (rect, action)) in items.iter().enumerate() {
        let hovered = hover == idx as i32;

        match *action {
            Action::Tab(i) => {
                let active = tab == i;
                let label = TABS[i].0;
                let lw = text_width(label, 14.0);
                let color = if active || hovered { TEXT } else { MUTED };
                bold(&mut pm, label, rect.x + (rect.w - lw) / 2.0, rect.y + rect.h / 2.0 - 1.0, 14.0, color);

                if active {
                    let underline = Rect { x: rect.x, y: rect.y + rect.h - 3.0, w: rect.w, h: 3.0 };
                    fill(&mut pm, &round_rect(underline, 0.0), TABS[i].1, 255);
                }
            }

            Action::Format(group, index) => {
                let name = settings::FORMAT_GROUPS[group].1[index];
                let on = settings.format_enabled(name);
                let accent = GROUP_ACCENTS[group];
                let chip = round_rect(*rect, 10.0);
                let label = name.to_uppercase();
                let lw = text_width(&label, 12.0);
                let (tx, ty) = (rect.x + (rect.w - lw) / 2.0, rect.y + rect.h / 2.0);

                if on {
                    fill(&mut pm, &chip, accent, 255);
                    bold(&mut pm, &label, tx, ty, 12.0, DARK_TEXT);
                } else {
                    fill(&mut pm, &chip, FIELD, 255);
                    stroke(&mut pm, &chip, EDGE, 255, 1.5);
                    draw_text(&mut pm, &label, tx, ty, 12.0, MUTED);
                    let mut pb = PathBuilder::new();
                    pb.move_to(tx - 2.0, ty);
                    pb.line_to(tx + lw + 2.0, ty);
                    stroke(&mut pm, &pb.finish().unwrap(), MUTED_RGB, 255, 1.2);
                }

                if hovered {
                    stroke(&mut pm, &chip, (255, 255, 255), 255, 2.0);
                }
            }

            Action::Step(row, dir) => {
                let list = quality_list(row);
                let index = quality_index(row, quality_value(&settings, row));
                let at_edge = (dir < 0 && index == 0) || (dir > 0 && index + 1 == list.len());
                let accent = [PURPLE, BLUE, AMBER, GREEN][row];

                if dir < 0 {
                    let card_rect = quality_card(row);
                    let card = round_rect(card_rect, 14.0);
                    fill(&mut pm, &card, PANEL, 255);
                    stroke(&mut pm, &card, EDGE, 255, 1.5);

                    bold(&mut pm, QUALITY_ROWS[row].0, card_rect.x + 18.0, card_rect.y + 24.0, 13.5, TEXT);
                    draw_text(&mut pm, QUALITY_ROWS[row].1, card_rect.x + 18.0, card_rect.y + 44.0, 11.0, MUTED);

                    let value = quality_text(row, list[index]);
                    let vw = text_width(&value, 14.0);
                    bold(&mut pm, &value, rect.x + rect.w + (84.0 - vw) / 2.0, rect.y + rect.h / 2.0, 14.0, TEXT);
                }

                let button = round_rect(*rect, 10.0);
                fill(&mut pm, &button, if at_edge { FIELD } else { accent }, 255);
                if at_edge {
                    stroke(&mut pm, &button, EDGE, 255, 1.5);
                } else if hovered {
                    stroke(&mut pm, &button, (255, 255, 255), 255, 2.0);
                }

                let color = if at_edge { MUTED_RGB } else { (14, 16, 21) };
                let (cx, cy) = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
                let mut pb = PathBuilder::new();
                pb.move_to(cx - 6.0, cy);
                pb.line_to(cx + 6.0, cy);
                if dir > 0 {
                    pb.move_to(cx, cy - 6.0);
                    pb.line_to(cx, cy + 6.0);
                }
                stroke(&mut pm, &pb.finish().unwrap(), color, 255, 2.2);
            }
            Action::Close => {
                let color = if hovered { (239, 68, 68) } else { MUTED_RGB };
                let (cx, cy) = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
                if hovered {
                    fill(&mut pm, &PathBuilder::from_circle(cx, cy, 16.0).unwrap(), (239, 68, 68), 60);
                }
                let mut pb = PathBuilder::new();
                pb.move_to(cx - 6.0, cy - 6.0);
                pb.line_to(cx + 6.0, cy + 6.0);
                pb.move_to(cx + 6.0, cy - 6.0);
                pb.line_to(cx - 6.0, cy + 6.0);
                stroke(&mut pm, &pb.finish().unwrap(), color, 255, 2.0);
            }

            Action::Browse(i) => {
                let accent = TOOLS[i].1;
                let y = row_y(i);

                bold(&mut pm, &TOOLS[i].0.to_uppercase(), 28.0, y + 12.0, 12.0, LABEL);

                let exists = std::path::Path::new(paths[i]).is_file();
                let dot = if exists { (74, 222, 128) } else { (248, 113, 113) };
                let label_w = text_width(&TOOLS[i].0.to_uppercase(), 12.0);
                fill(&mut pm, &PathBuilder::from_circle(28.0 + label_w + 12.0, y + 12.0, 4.0).unwrap(), dot, 255);
                draw_text(
                    &mut pm,
                    if exists { "found" } else { "not found" },
                    28.0 + label_w + 22.0,
                    y + 12.0,
                    11.0,
                    MUTED,
                );

                let field = Rect { x: 28.0, y: y + 28.0, w: W as f32 - 56.0 - 96.0 - 12.0, h: 40.0 };
                let path = round_rect(field, 12.0);
                fill(&mut pm, &path, FIELD, 255);
                stroke(&mut pm, &path, EDGE, 255, 1.5);
                let shown = fit_text(paths[i], 13.0, field.w - 28.0);
                draw_text(&mut pm, &shown, field.x + 14.0, field.y + field.h / 2.0, 13.0, TEXT);

                let button = round_rect(*rect, 12.0);
                fill(&mut pm, &button, accent, 255);
                if hovered {
                    stroke(&mut pm, &button, (255, 255, 255), 255, 2.0);
                }
                let label = "Browse";
                let lw = text_width(label, 13.0);
                bold(&mut pm, label, rect.x + (rect.w - lw) / 2.0, rect.y + rect.h / 2.0, 13.0, DARK_TEXT);
            }

            Action::Toggle => {
                let card = round_rect(*rect, 14.0);
                fill(&mut pm, &card, PANEL, 255);
                stroke(&mut pm, &card, if hovered { (255, 255, 255) } else { EDGE }, 255, 1.5);

                bold(&mut pm, "Save downloaded links to the open Explorer folder", rect.x + 18.0, rect.y + 19.0, 13.5, TEXT);
                draw_text(&mut pm, "Otherwise they go to your Downloads folder", rect.x + 18.0, rect.y + 37.0, 11.0, MUTED);

                let track = Rect { x: rect.x + rect.w - 18.0 - 48.0, y: rect.y + 14.0, w: 48.0, h: 24.0 };
                let on = settings.use_explorer_folder;
                fill(&mut pm, &round_rect(track, 12.0), if on { AMBER } else { (84, 91, 110) }, 255);
                let knob_x = if on { track.x + track.w - 12.0 } else { track.x + 12.0 };
                fill(&mut pm, &PathBuilder::from_circle(knob_x, track.y + 12.0, 9.0).unwrap(), (255, 255, 255), 255);
            }
        }
    }

    let footer = match tab {
        1 => "Hidden formats won't appear on the ring",
        2 => "Applies to new conversions",
        _ => "Changes are saved automatically",
    };
    draw_text(&mut pm, footer, 28.0, H as f32 - 24.0, 11.0, MUTED);

    stroke(&mut pm, &round_rect(Rect { x: 0.75, y: 0.75, w: W as f32 - 1.5, h: H as f32 - 1.5 }, 0.0), (74, 81, 99), 255, 1.5);

    pm
}

const MUTED_RGB: (u8, u8, u8) = (176, 182, 196);

fn hit_index(x: f32, y: f32) -> i32 {
    layout()
        .iter()
        .position(|(rect, _)| rect.contains(x, y))
        .map_or(-1, |i| i as i32)
}

fn browse(hwnd: HWND, index: usize) {
    unsafe {
        let mut file = [0u16; 1024];
        let current = match index {
            0 => settings::get().ffmpeg,
            1 => settings::get().blender,
            _ => settings::get().ytdlp,
        };
        for (slot, unit) in file.iter_mut().zip(current.encode_utf16().take(1023)) {
            *slot = unit;
        }

        let filter: Vec<u16> = "Executables\0*.exe\0All files\0*.*\0\0".encode_utf16().collect();
        let title: Vec<u16> = format!("Select {}\0", TOOLS[index].0).encode_utf16().collect();

        let mut ofn = OPENFILENAMEW {
            lStructSize: size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: hwnd,
            lpstrFilter: PCWSTR(filter.as_ptr()),
            lpstrFile: PWSTR(file.as_mut_ptr()),
            nMaxFile: file.len() as u32,
            lpstrTitle: PCWSTR(title.as_ptr()),
            Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST,
            ..Default::default()
        };

        if GetOpenFileNameW(&mut ofn).as_bool() {
            let end = file.iter().position(|&c| c == 0).unwrap_or(file.len());
            let chosen = String::from_utf16_lossy(&file[..end]);

            settings::update(|s| match index {
                0 => s.ffmpeg = chosen,
                1 => s.blender = chosen,
                _ => s.ytdlp = chosen,
            });
        }
    }
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let mouse = |lparam: LPARAM| {
            (
                (lparam.0 as u16 as i16) as f32,
                ((lparam.0 >> 16) as u16 as i16) as f32,
            )
        };

        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                let pm = render(HOVER.load(Ordering::Relaxed));
                let mut bgra = pm.data().to_vec();
                for px in bgra.chunks_exact_mut(4) {
                    px.swap(0, 2);
                }

                let mut bmi = BITMAPINFO::default();
                bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
                bmi.bmiHeader.biWidth = W;
                bmi.bmiHeader.biHeight = -H;
                bmi.bmiHeader.biPlanes = 1;
                bmi.bmiHeader.biBitCount = 32;
                bmi.bmiHeader.biCompression = BI_RGB.0;

                SetDIBitsToDevice(
                    hdc, 0, 0, W as u32, H as u32, 0, 0, 0, H as u32,
                    bgra.as_ptr() as *const _, &bmi, DIB_RGB_COLORS,
                );

                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }

            WM_ERASEBKGND => LRESULT(1),

            WM_NCHITTEST => {
                let mut point = POINT {
                    x: (lparam.0 as u16 as i16) as i32,
                    y: ((lparam.0 >> 16) as u16 as i16) as i32,
                };
                let _ = ScreenToClient(hwnd, &mut point);
                let (x, y) = (point.x as f32, point.y as f32);

                if y >= 0.0 && y < TITLE_H && x >= 0.0 && x < W as f32 && hit_index(x, y) < 0 {
                    LRESULT(HTCAPTION as isize)
                } else {
                    LRESULT(HTCLIENT as isize)
                }
            }

            WM_MOUSEMOVE => {
                let (x, y) = mouse(lparam);
                let index = hit_index(x, y);

                if TRACKING.swap(1, Ordering::Relaxed) == 0 {
                    let mut track = TRACKMOUSEEVENT {
                        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    let _ = TrackMouseEvent(&mut track);
                }

                if HOVER.swap(index, Ordering::Relaxed) != index {
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                LRESULT(0)
            }

            WM_MOUSELEAVE => {
                TRACKING.store(0, Ordering::Relaxed);
                HOVER.store(-1, Ordering::Relaxed);
                let _ = InvalidateRect(Some(hwnd), None, false);
                LRESULT(0)
            }

            WM_SETCURSOR => {
                let id = if HOVER.load(Ordering::Relaxed) >= 0 { IDC_HAND } else { IDC_ARROW };
                if let Ok(cursor) = LoadCursorW(None, id) {
                    SetCursor(Some(cursor));
                }
                LRESULT(1)
            }

            WM_LBUTTONUP => {
                let (x, y) = mouse(lparam);

                if let Some((_, action)) = layout().into_iter().find(|(r, _)| r.contains(x, y)) {
                    match action {
                        Action::Close => {
                            let _ = DestroyWindow(hwnd);
                            return LRESULT(0);
                        }
                        Action::Browse(i) => browse(hwnd, i),
                        Action::Toggle => settings::update(|s| s.use_explorer_folder = !s.use_explorer_folder),
                        Action::Tab(i) => {
                            TAB.store(i, Ordering::Relaxed);
                            HOVER.store(hit_index(x, y), Ordering::Relaxed);
                        }
                        Action::Format(group, index) => toggle_format(group, index),
                        Action::Step(row, dir) => step_quality(row, dir),
                    }
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                LRESULT(0)
            }

            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }

            WM_DESTROY => {
                SETTINGS_HWND.store(0, Ordering::Relaxed);
                LRESULT(0)
            }

            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

pub fn open() {
    unsafe {
        let existing = SETTINGS_HWND.load(Ordering::Relaxed);
        if existing != 0 {
            let hwnd = HWND(existing as *mut _);
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
            return;
        }

        let Ok(instance) = GetModuleHandleW(None) else {
            return;
        };
        let instance: HINSTANCE = instance.into();

        let icon = LoadIconW(Some(instance), PCWSTR(1 as *const u16)).unwrap_or_default();

        let class = WNDCLASSW {
            hInstance: instance,
            lpszClassName: w!("RingToolSettings"),
            lpfnWndProc: Some(proc),
            hIcon: icon,
            ..Default::default()
        };
        RegisterClassW(&class);

        let x = (GetSystemMetrics(SM_CXSCREEN) - W) / 2;
        let y = (GetSystemMetrics(SM_CYSCREEN) - H) / 2;

        let hwnd = match CreateWindowExW(
            WS_EX_APPWINDOW,
            w!("RingToolSettings"),
            w!("RingTool Settings"),
            WS_POPUP | WS_MINIMIZEBOX,
            x,
            y,
            W,
            H,
            None,
            None,
            Some(instance),
            None,
        ) {
            Ok(hwnd) => hwnd,
            Err(error) => {
                eprintln!("Failed to create settings window: {error}");
                return;
            }
        };

        SETTINGS_HWND.store(hwnd.0 as isize, Ordering::Relaxed);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(hwnd));
    }
}
