const STYLESHEET: &str = include_str!("../../src/App.css");

fn token(selector: &str, name: &str) -> String {
    let marker = format!("{selector} {{");
    let start = STYLESHEET.find(&marker).expect("theme selector exists") + marker.len();
    let block = &STYLESHEET[start..];
    let block = &block[..block.find('}').expect("theme block closes")];
    let declaration = format!("--{name}:");
    let value = block
        .lines()
        .find_map(|line| line.trim().strip_prefix(&declaration))
        .expect("token exists")
        .trim()
        .trim_end_matches(';');
    value.to_owned()
}

fn luminance(hex: &str) -> f64 {
    let (offsets, width) = if hex.len() == 4 {
        ([1, 2, 3], 1)
    } else {
        ([1, 3, 5], 2)
    };
    let channel = |offset: usize| {
        let encoded = &hex[offset..offset + width];
        let encoded = if width == 1 {
            encoded.repeat(2)
        } else {
            encoded.to_owned()
        };
        let encoded = u8::from_str_radix(&encoded, 16).expect("valid hex color");
        let value = f64::from(encoded) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(offsets[0]) + 0.7152 * channel(offsets[1]) + 0.0722 * channel(offsets[2])
}

fn contrast(foreground: &str, background: &str) -> f64 {
    let foreground = luminance(foreground);
    let background = luminance(background);
    (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05)
}

fn assert_small_text_contrast(selector: &str) {
    let foreground = token(selector, "text-muted");
    for surface in ["surface-panel", "surface-card"] {
        let background = token(selector, surface);
        let ratio = contrast(&foreground, &background);
        assert!(
            ratio >= 4.5,
            "{selector} muted text on {surface} is only {ratio:.2}:1"
        );
    }
}

#[test]
fn dark_muted_text_meets_wcag_aa_on_compact_surfaces() {
    assert_small_text_contrast(":root");
}

#[test]
fn light_muted_text_meets_wcag_aa_on_compact_surfaces() {
    assert_small_text_contrast(":root[data-theme=\"light\"]");
}
