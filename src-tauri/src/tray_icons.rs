use tauri::image::Image;

use crate::tray_presentation::TrayIconKey;

const ICON_SIZE: u32 = 32;
const SUPERSAMPLE: u32 = 4;

#[derive(Clone, Copy)]
struct Color {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl Color {
    const TRANSPARENT: Self = Self {
        red: 0,
        green: 0,
        blue: 0,
        alpha: 0,
    };
    const OUTLINE: Self = Self {
        red: 5,
        green: 12,
        blue: 18,
        alpha: 235,
    };
    const GLYPH: Self = Self {
        red: 247,
        green: 250,
        blue: 252,
        alpha: 255,
    };
}

pub fn image_for(key: TrayIconKey) -> Image<'static> {
    Image::new_owned(render_rgba(key), ICON_SIZE, ICON_SIZE)
}

fn render_rgba(key: TrayIconKey) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((ICON_SIZE * ICON_SIZE * 4) as usize);
    let samples_per_pixel = SUPERSAMPLE * SUPERSAMPLE;

    for pixel_y in 0..ICON_SIZE {
        for pixel_x in 0..ICON_SIZE {
            let mut alpha_sum = 0_u32;
            let mut red_sum = 0_u32;
            let mut green_sum = 0_u32;
            let mut blue_sum = 0_u32;

            for sample_y in 0..SUPERSAMPLE {
                for sample_x in 0..SUPERSAMPLE {
                    let x = pixel_x as f32 + (sample_x as f32 + 0.5) / SUPERSAMPLE as f32;
                    let y = pixel_y as f32 + (sample_y as f32 + 0.5) / SUPERSAMPLE as f32;
                    let color = sample_color(key, x, y);
                    let alpha = u32::from(color.alpha);
                    alpha_sum += alpha;
                    red_sum += u32::from(color.red) * alpha;
                    green_sum += u32::from(color.green) * alpha;
                    blue_sum += u32::from(color.blue) * alpha;
                }
            }

            if alpha_sum == 0 {
                pixels.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                pixels.extend_from_slice(&[
                    (red_sum / alpha_sum) as u8,
                    (green_sum / alpha_sum) as u8,
                    (blue_sum / alpha_sum) as u8,
                    (alpha_sum / samples_per_pixel) as u8,
                ]);
            }
        }
    }

    pixels
}

fn sample_color(key: TrayIconKey, x: f32, y: f32) -> Color {
    let center_x = 14.5;
    let center_y = 14.5;
    let radius = 9.5;
    let distance_from_center = ((x - center_x).powi(2) + (y - center_y).powi(2)).sqrt();
    let tail_distance = distance_to_segment(x, y, 20.3, 20.2, 26.0, 25.9);
    let base_outline = (distance_from_center - radius).abs() <= 2.65 || tail_distance <= 3.0;
    let base_fill = (distance_from_center - radius).abs() <= 1.75 || tail_distance <= 1.9;
    let (glyph_outline, glyph_fill) = glyph_coverage(key, x, y);

    let mut color = Color::TRANSPARENT;
    if base_outline {
        color = Color::OUTLINE;
    }
    if base_fill {
        color = status_color(key);
    }
    if glyph_outline {
        color = Color::OUTLINE;
    }
    if glyph_fill {
        color = Color::GLYPH;
    }
    color
}

fn status_color(key: TrayIconKey) -> Color {
    match key {
        TrayIconKey::Healthy => Color {
            red: 53,
            green: 213,
            blue: 244,
            alpha: 255,
        },
        TrayIconKey::Warning => Color {
            red: 255,
            green: 179,
            blue: 41,
            alpha: 255,
        },
        TrayIconKey::Critical => Color {
            red: 255,
            green: 113,
            blue: 105,
            alpha: 255,
        },
        TrayIconKey::Stale => Color {
            red: 143,
            green: 166,
            blue: 184,
            alpha: 255,
        },
        TrayIconKey::Unavailable => Color {
            red: 116,
            green: 130,
            blue: 147,
            alpha: 255,
        },
    }
}

fn glyph_coverage(key: TrayIconKey, x: f32, y: f32) -> (bool, bool) {
    match key {
        TrayIconKey::Healthy => {
            let distance = distance(x, y, 14.5, 14.5);
            (distance <= 3.35, distance <= 2.15)
        }
        TrayIconKey::Warning => {
            let vertices = [(14.5, 8.3), (9.1, 19.3), (19.9, 19.3)];
            let fill = point_in_triangle((x, y), vertices);
            let edge_distance = triangle_edge_distance(x, y, vertices);
            (fill || edge_distance <= 1.25, fill && edge_distance > 1.05)
        }
        TrayIconKey::Critical => {
            let stem = distance_to_segment(x, y, 14.5, 9.2, 14.5, 16.1);
            let dot = distance(x, y, 14.5, 20.0);
            (stem <= 2.25 || dot <= 2.45, stem <= 1.1 || dot <= 1.2)
        }
        TrayIconKey::Stale => {
            let clock_distance = (distance(x, y, 14.5, 14.5) - 5.1).abs();
            let minute_hand = distance_to_segment(x, y, 14.5, 14.5, 14.5, 10.9);
            let hour_hand = distance_to_segment(x, y, 14.5, 14.5, 17.4, 16.3);
            (
                clock_distance <= 1.45 || minute_hand <= 1.8 || hour_hand <= 1.8,
                clock_distance <= 0.65 || minute_hand <= 0.7 || hour_hand <= 0.7,
            )
        }
        TrayIconKey::Unavailable => {
            let slash = distance_to_segment(x, y, 7.2, 24.0, 23.9, 7.3);
            (slash <= 2.7, slash <= 1.35)
        }
    }
}

fn distance(x: f32, y: f32, center_x: f32, center_y: f32) -> f32 {
    ((x - center_x).powi(2) + (y - center_y).powi(2)).sqrt()
}

fn distance_to_segment(x: f32, y: f32, start_x: f32, start_y: f32, end_x: f32, end_y: f32) -> f32 {
    let delta_x = end_x - start_x;
    let delta_y = end_y - start_y;
    let length_squared = delta_x * delta_x + delta_y * delta_y;
    let projection = if length_squared == 0.0 {
        0.0
    } else {
        (((x - start_x) * delta_x + (y - start_y) * delta_y) / length_squared).clamp(0.0, 1.0)
    };
    distance(
        x,
        y,
        start_x + projection * delta_x,
        start_y + projection * delta_y,
    )
}

fn point_in_triangle(point: (f32, f32), vertices: [(f32, f32); 3]) -> bool {
    let [a, b, c] = vertices;
    let d1 = signed_area(point, a, b);
    let d2 = signed_area(point, b, c);
    let d3 = signed_area(point, c, a);
    let has_negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_negative && has_positive)
}

fn signed_area(point: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    (point.0 - b.0) * (a.1 - b.1) - (a.0 - b.0) * (point.1 - b.1)
}

fn triangle_edge_distance(x: f32, y: f32, vertices: [(f32, f32); 3]) -> f32 {
    let [a, b, c] = vertices;
    distance_to_segment(x, y, a.0, a.1, b.0, b.1)
        .min(distance_to_segment(x, y, b.0, b.1, c.0, c.1))
        .min(distance_to_segment(x, y, c.0, c.1, a.0, a.1))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::tray_presentation::TrayIconKey;

    use super::image_for;

    const ALL_KEYS: [TrayIconKey; 5] = [
        TrayIconKey::Healthy,
        TrayIconKey::Warning,
        TrayIconKey::Critical,
        TrayIconKey::Stale,
        TrayIconKey::Unavailable,
    ];

    #[test]
    fn renders_five_transparent_distinct_shape_and_color_icons_at_tray_size() {
        let mut rgba_variants = HashSet::new();
        let mut shape_variants = HashSet::new();

        for key in ALL_KEYS {
            let image = image_for(key);
            assert_eq!((image.width(), image.height()), (32, 32));
            assert_eq!(image.rgba().len(), 32 * 32 * 4);
            assert_eq!(image.rgba()[3], 0, "top-left must remain transparent");

            rgba_variants.insert(image.rgba().to_vec());
            shape_variants.insert(
                image
                    .rgba()
                    .chunks_exact(4)
                    .map(|pixel| pixel[3] >= 96)
                    .collect::<Vec<_>>(),
            );
        }

        assert_eq!(rgba_variants.len(), ALL_KEYS.len());
        assert_eq!(shape_variants.len(), ALL_KEYS.len());
    }
}
