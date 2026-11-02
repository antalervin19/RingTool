use super::ring::{polar, Ring, C, R1, R2, R3, R4};
use super::menu::Menu;
use tiny_skia::{FillRule, LineJoin, Paint, Path, PathBuilder, Pixmap, Stroke, Transform};
use fontdue::{Font, FontSettings};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const ACCENTS: [(u8, u8, u8); 3] = [
    (168, 85, 247),
    (245, 158, 11),
    (45, 156, 219),
];

const CATEGORY_COUNT: usize = 3;
const OPTION_COUNT: usize = 4;

static FONT: OnceLock<Font> = OnceLock::new();
type Glyph = (fontdue::Metrics, Vec<u8>);
static GLYPH_CACHE: OnceLock<Mutex<HashMap<(char, u32), Glyph>>> = OnceLock::new();

fn get_font() -> &'static Font {
    FONT.get_or_init(|| {
        Font::from_bytes(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/fonts/Montserrat-VariableFont_wght.ttf"
            )).as_slice(),
            FontSettings::default(),
        )
        .expect("Failed to load Montserrat font")
    })
}

fn draw_circle(pm: &mut Pixmap, radius: f32, color: (u8, u8, u8), alpha: u8) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.0, color.1, color.2, alpha);
    paint.anti_alias = true;

    let path = PathBuilder::from_circle(C, C, radius).unwrap();
    pm.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
}

fn cached_glyph(c: char, size: f32) -> Glyph {
    let mut cache = GLYPH_CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap();
    cache
        .entry((c, size.to_bits()))
        .or_insert_with(|| get_font().rasterize(c, size))
        .clone()
}

pub fn text_width(text: &str, size: f32) -> f32 {
    text.chars().map(|c| cached_glyph(c, size).0.advance_width).sum()
}

pub fn draw_text(pm: &mut Pixmap, text: &str, x: f32, y: f32, size: f32, color: (u8, u8, u8, u8)) {
    let width = pm.width() as i32;
    let height = pm.height() as i32;
    let pixels = pm.data_mut();
    let baseline = y + size * 0.35;
    let mut pen = x;

    for c in text.chars() {
        let (metrics, bitmap) = cached_glyph(c, size);
        let gx = (pen + metrics.xmin as f32).round() as i32;
        let gy = (baseline - metrics.ymin as f32 - metrics.height as f32).round() as i32;

        for sy in 0..metrics.height {
            for sx in 0..metrics.width {
                let coverage = bitmap[sy * metrics.width + sx] as u32;
                if coverage == 0 {
                    continue;
                }

                let px = gx + sx as i32;
                let py = gy + sy as i32;
                if px < 0 || py < 0 || px >= width || py >= height {
                    continue;
                }

                let a = coverage * color.3 as u32 / 255;
                let index = (py as usize * width as usize + px as usize) * 4;
                let src = [color.0, color.1, color.2];

                for channel in 0..3 {
                    let dst = pixels[index + channel] as u32;
                    pixels[index + channel] =
                        ((src[channel] as u32 * a + dst * (255 - a)) / 255).min(255) as u8;
                }
                let dst_a = pixels[index + 3] as u32;
                pixels[index + 3] = (a + dst_a * (255 - a) / 255).min(255) as u8;
            }
        }

        pen += metrics.advance_width;
    }
}

fn draw_label(pm: &mut Pixmap, text: &str, angle: f32, radius: f32, max_width: f32, font_size: f32) {
    if text.is_empty() || max_width <= 0.0 {
        return;
    }

    let font = get_font();
    let chars: Vec<char> = text.chars().collect();

    let glyphs: Vec<_> = {
        let mut cache = GLYPH_CACHE
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap();
        chars
            .iter()
            .map(|&c| {
                cache
                    .entry((c, font_size.to_bits()))
                    .or_insert_with(|| font.rasterize(c, font_size))
                    .clone()
            })
            .collect()
    };

    let total_width: f32 = glyphs.iter()
        .map(|(metrics, _)| metrics.advance_width)
        .sum();

    if total_width <= 0.0 {
        return;
    }

    let scale = (max_width / total_width).min(1.0);
    let direction = if angle.rem_euclid(360.0) > 90.0 && angle.rem_euclid(360.0) < 270.0 {
        -1.0
    } else {
        1.0
    };

    let mut offset = -total_width * scale * 0.5;
    let width = pm.width() as usize;
    let height = pm.height() as usize;
    let pixels = pm.data_mut();

    for (metrics, bitmap) in &glyphs {
        let advance = metrics.advance_width * scale;
        let char_distance = offset + advance * 0.5;
        let char_angle = angle + direction * (char_distance / radius).to_degrees();
        let rotation = (char_angle + if direction < 0.0 { 180.0 } else { 0.0 }).to_radians();

        let center_x = C + radius * char_angle.to_radians().sin();
        let center_y = C - radius * char_angle.to_radians().cos();

        let cos = rotation.cos();
        let sin = rotation.sin();

        let origin_x = metrics.xmin as f32 - metrics.advance_width * 0.5;
        let origin_y = -(metrics.ymin as f32 + metrics.height as f32);

        for sy in 0..metrics.height {
            for sx in 0..metrics.width {
                let coverage = bitmap[sy * metrics.width + sx];
                if coverage == 0 {
                    continue;
                }

                let local_x = origin_x + sx as f32;
                let local_y = origin_y + sy as f32;

                let x = (center_x + local_x * cos - local_y * sin).round() as i32;
                let y = (center_y + local_x * sin + local_y * cos).round() as i32;

                if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
                    continue;
                }

                let index = (y as usize * width + x as usize) * 4;
                let alpha = coverage as u32;

                for channel in 0..4 {
                    let dst = pixels[index + channel] as u32;
                    pixels[index + channel] = (alpha + dst * (255 - alpha) / 255).min(255) as u8;
                }
            }
        }

        offset += advance;
    }
}

fn sector_path(r1: f32, r2: f32, a0: f32, a1: f32) -> Path {
    let steps = (((a1 - a0).abs() / 2.0).ceil() as usize).max(2);
    let angle_at = |k: usize| a0 + (a1 - a0) * k as f32 / steps as f32;

    let mut pb = PathBuilder::new();

    let (x, y) = polar(r2, a0);
    pb.move_to(x, y);

    for k in 1..=steps {
        let (x, y) = polar(r2, angle_at(k));
        pb.line_to(x, y);
    }

    for k in (0..=steps).rev() {
        let (x, y) = polar(r1, angle_at(k));
        pb.line_to(x, y);
    }

    pb.close();
    pb.finish().expect("valid sector")
}

pub fn draw_segment(pm: &mut Pixmap, ring: &Ring, i: usize, accent: (u8, u8, u8), lit: bool) {
    let mid = ring.center_angle(i);

    let path = sector_path(
        ring.r1,
        ring.r2,
        mid - ring.step / 2.0 + 0.8,
        mid + ring.step / 2.0 - 0.8,
    );

    let mut fill = Paint::default();

    if lit {
        fill.set_color_rgba8(accent.0, accent.1, accent.2, 242);
    } else {
        fill.set_color_rgba8(30, 34, 43, 216);
    }

    pm.fill_path(&path, &fill, FillRule::Winding, Transform::identity(), None);

    let mut line = Paint::default();
    line.set_color_rgba8(accent.0, accent.1, accent.2, 176);

    let stroke = Stroke {
        width: 1.5,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };

    pm.stroke_path(&path, &line, &stroke, Transform::identity(), None);
}

pub fn render_ring(active_category: Option<usize>, hovered_option: Option<usize>) -> Pixmap {
    let mut pm = Pixmap::new((2.0 * C) as u32, (2.0 * C) as u32).unwrap();
    draw_circle(&mut pm, R1, (30, 34, 43), 230);

    let category_step = 360.0 / CATEGORY_COUNT as f32;

    for i in 0..CATEGORY_COUNT {
        let ring = Ring {
            r1: R1,
            r2: R2,
            first: i as f32 * category_step,
            step: category_step,
            count: 1,
        };

        draw_segment(
            &mut pm,
            &ring,
            0,
            ACCENTS[i],
            active_category == Some(i),
        );
    }

    if let Some(category) = active_category {
        let step = 36.0_f32.min(category_step / OPTION_COUNT as f32);
        let mid = category as f32 * category_step;

        let options = Ring {
            r1: R3,
            r2: R4,
            first: mid - (OPTION_COUNT as f32 - 1.0) * step / 2.0,
            step,
            count: OPTION_COUNT,
        };

        for i in 0..OPTION_COUNT {
            draw_segment(
                &mut pm,
                &options,
                i,
                ACCENTS[category],
                hovered_option == Some(i),
            );
        }
    }

    pm
}

pub fn render_menu(
    menu: &Menu,
    active_category: Option<usize>,
    hovered_option: Option<usize>,
) -> Pixmap {
    let mut pm = Pixmap::new((2.0 * C) as u32, (2.0 * C) as u32).unwrap();

    match menu {
        Menu::SingleRing { options } => {
            if options.is_empty() {
                return pm;
            }

            let step = 360.0 / options.len() as f32;

            let ring = Ring {
                r1: R1,
                r2: R2,
                first: 0.0,
                step,
                count: options.len(),
            };

            for i in 0..options.len() {
                let lit = hovered_option == Some(i);

                draw_segment(
                    &mut pm,
                    &ring,
                    i,
                    (168, 85, 247),
                    lit,
                );

                let radius = (R1 + R2) * 0.5;
                let max_width = 2.0 * radius * (step.to_radians() * 0.5).sin() - 8.0;

                draw_label(
                    &mut pm,
                    &options[i].label,
                    ring.center_angle(i),
                    radius,
                    max_width,
                    12.0,
                );
            }
        }

        Menu::MultiRing { categories } => {
            if categories.is_empty() {
                return pm;
            }

            let category_step = 360.0 / categories.len() as f32;

            let category_ring = Ring {
                r1: R1,
                r2: R2,
                first: 0.0,
                step: category_step,
                count: categories.len(),
            };

            for i in 0..categories.len() {
                let lit = active_category == Some(i);

                draw_segment(
                    &mut pm,
                    &category_ring,
                    i,
                    categories[i].color,
                    lit,
                );

                let radius = (R1 + R2) * 0.5;
                let max_width = 2.0 * radius * (category_step.to_radians() * 0.5).sin() - 8.0;

                draw_label(
                    &mut pm,
                    &categories[i].label,
                    category_ring.center_angle(i),
                    radius,
                    max_width,
                    13.0,
                );
            }

            if let Some(category_index) = active_category {
                if let Some(category) = categories.get(category_index) {
                    let options = &category.options;

                    if !options.is_empty() {
                        let option_step = category_step / options.len() as f32;
                        let category_center = category_index as f32 * category_step;

                        let option_ring = Ring {
                            r1: R3,
                            r2: R4,
                            first: category_center - category_step / 2.0 + option_step / 2.0,
                            step: option_step,
                            count: options.len(),
                        };

                        for i in 0..options.len() {
                            let lit = hovered_option == Some(i);

                            draw_segment(
                                &mut pm,
                                &option_ring,
                                i,
                                category.color,
                                lit,
                            );

                            let radius = (R3 + R4) * 0.5;
                            let max_width = 2.0 * radius * (option_step.to_radians() * 0.5).sin() - 8.0;

                            draw_label(
                                &mut pm,
                                &options[i].label,
                                option_ring.center_angle(i),
                                radius,
                                max_width,
                                12.0,
                            );
                        }
                    }
                }
            }
        }
    }

    pm
}