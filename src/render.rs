use std::f32::consts::{FRAC_PI_2, TAU};
use windows::Win32::Foundation::COLORREF;

use crate::models::ColorMode;
use crate::native_interop;
use crate::poller;

pub const CAPSULE_RADIUS: i32 = 12;
pub const DRAG_HANDLE_WIDTH: i32 = 2;
pub const DRAG_HANDLE_HEIGHT: i32 = 14;
pub const MODEL_BLOCK_WIDTH: i32 = 114;
pub const MODEL_DIVIDER_WIDTH: i32 = 17;
pub const CAPSULE_LEFT_PADDING: i32 = 20;
pub const CAPSULE_RIGHT_PADDING: i32 = 14;
pub const WIDGET_HEIGHT: i32 = 36;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ModelKind {
    Codex,
    Antigravity,
    ClaudeCode,
}

/// Exact 2D signed distance field for a rounded rectangle.
/// Returns negative values inside, 0.0 on the border, and positive values outside.
pub fn sd_rounded_box(px: f32, py: f32, w: f32, h: f32, r: f32) -> f32 {
    let half_w = w * 0.5;
    let half_h = h * 0.5;
    let qx = (px - half_w).abs() - (half_w - r);
    let qy = (py - half_h).abs() - (half_h - r);
    let inside_dist = qx.max(qy).min(0.0);
    let ox = qx.max(0.0);
    let oy = qy.max(0.0);
    let outside_dist = (ox * ox + oy * oy).sqrt();
    inside_dist + outside_dist - r
}

/// Parse raw usage text into (percentage, timestamp).
/// Strips out Chinese labels ("剩余", "重置"), approximation signs ("≈", "~"),
/// ensuring GDI fonts don't produce tofu replacement boxes or overflow text bounds.
pub fn parse_display_parts(text: &str) -> (String, String) {
    let clean = text
        .replace("剩余", "")
        .replace('剩', "")
        .replace('余', "")
        .replace("重置", "")
        .replace('≈', "")
        .replace('~', "");

    let tokens: Vec<&str> = clean
        .split(|c| c == '\u{00b7}' || c == ' ' || c == '\t')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if tokens.is_empty() {
        return (String::new(), String::new());
    }

    let pct = tokens[0].to_string();
    let time = if tokens.len() > 1 {
        tokens[1..].join(" ")
    } else {
        String::new()
    };

    (pct, time)
}

pub fn tint_bg(bg: (u8, u8, u8), accent: (u8, u8, u8), factor: f32) -> COLORREF {
    let r = ((bg.0 as f32) * (1.0 - factor) + (accent.0 as f32) * factor).round() as u32;
    let g = ((bg.1 as f32) * (1.0 - factor) + (accent.1 as f32) * factor).round() as u32;
    let b = ((bg.2 as f32) * (1.0 - factor) + (accent.2 as f32) * factor).round() as u32;
    COLORREF(native_interop::colorref(r as u8, g as u8, b as u8))
}

pub fn resolve_model_colors(
    kind: ModelKind,
    color_mode: ColorMode,
    is_dark: bool,
    p5h: f64,
    p7d: f64,
) -> ((u8, u8, u8), (u8, u8, u8)) {
    match color_mode {
        ColorMode::Colorful => {
            let c5h_normal = match kind {
                ModelKind::Codex => (16u8, 185u8, 129u8),      // Emerald #10B981
                ModelKind::Antigravity => (59u8, 130u8, 246u8), // Blue #3B82F6
                ModelKind::ClaudeCode => (217u8, 119u8, 87u8),  // Terracotta #D97757
            };
            let c7d_normal = (245u8, 158u8, 11u8);              // Amber for 7d (both Codex and AGY)

            let red_warning = (239u8, 68u8, 68u8);

            let c5h = if p5h > 0.0 && poller::remaining_percentage(p5h) <= 20.0 {
                red_warning
            } else {
                c5h_normal
            };

            let c7d = if p7d > 0.0 && poller::remaining_percentage(p7d) <= 20.0 {
                red_warning
            } else {
                c7d_normal
            };

            (c5h, c7d)
        }
        ColorMode::Monochrome => {
            // Pure black / white / gray version
            if is_dark {
                // In dark mode: crisp white for 5h, silver gray for 7d
                let c5h = (255u8, 255u8, 255u8); // Pure White #FFFFFF
                let c7d = (160u8, 174u8, 192u8); // Slate Silver #A0AEC0
                (c5h, c7d)
            } else {
                // In light mode: deep slate for 5h, graphite gray for 7d
                let c5h = (30u8, 41u8, 59u8);    // Deep Slate #1E293B
                let c7d = (100u8, 116u8, 139u8); // Graphite Gray #64748B
                (c5h, c7d)
            }
        }
    }
}

/// Software rasterizer for Scheme C1 Concentric Dual Rings (outer 5h, inner 7d, clean minimalist center).
pub fn rasterize_concentric_ring(
    buffer: &mut [u32],
    buf_w: i32,
    buf_h: i32,
    cx: f32,
    cy: f32,
    p5h: f32,
    p7d: f32,
    _kind: ModelKind,
    c5h_rgb: (u8, u8, u8),
    c7d_rgb: (u8, u8, u8),
    is_dark: bool,
    dpi_scale: f32,
) {
    let r1 = 11.5 * dpi_scale;
    let w1 = 2.2 * dpi_scale;
    let r2 = 8.0 * dpi_scale;
    let w2 = 1.8 * dpi_scale;

    let track_rgb = if is_dark { (255u8, 255u8, 255u8) } else { (0u8, 0u8, 0u8) };
    let track_alpha = if is_dark { 0.12f32 } else { 0.08f32 };

    let sw5h = TAU * (p5h.clamp(0.0, 100.0) / 100.0);
    let sw7d = TAU * (p7d.clamp(0.0, 100.0) / 100.0);

    let box_r = (14.5 * dpi_scale).ceil() as i32;
    let min_x = ((cx - box_r as f32).floor() as i32).max(0);
    let max_x = ((cx + box_r as f32).ceil() as i32).min(buf_w - 1);
    let min_y = ((cy - box_r as f32).floor() as i32).max(0);
    let max_y = ((cy + box_r as f32).ceil() as i32).min(buf_h - 1);

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            let dx = (px as f32 + 0.5) - cx;
            let dy = (py as f32 + 0.5) - cy;
            let r = (dx * dx + dy * dy).sqrt();
            if r > 15.0 * dpi_scale {
                continue;
            }

            let angle = dy.atan2(dx);
            let mut th = angle + FRAC_PI_2;
            if th < 0.0 {
                th += TAU;
            }

            // Track 5h
            let d1_t = (r - r1).abs();
            let a1_t = (0.5 - (d1_t - w1 * 0.5)).clamp(0.0, 1.0);

            // Arc 5h with round caps
            let d_s = (dx * dx + (dy + r1) * (dy + r1)).sqrt();
            let ex = r1 * sw5h.sin();
            let ey = -r1 * sw5h.cos();
            let d_e = ((dx - ex) * (dx - ex) + (dy - ey) * (dy - ey)).sqrt();
            let mut d_arc5h = d_s.min(d_e);
            if p5h > 0.0 && th <= sw5h {
                d_arc5h = d_arc5h.min(d1_t);
            }
            let a1_arc = if p5h > 0.0 {
                (0.5 - (d_arc5h - w1 * 0.5)).clamp(0.0, 1.0)
            } else {
                0.0
            };

            // Track 7d
            let d2_t = (r - r2).abs();
            let a2_t = (0.5 - (d2_t - w2 * 0.5)).clamp(0.0, 1.0);

            // Arc 7d with round caps
            let d_s2 = (dx * dx + (dy + r2) * (dy + r2)).sqrt();
            let ex2 = r2 * sw7d.sin();
            let ey2 = -r2 * sw7d.cos();
            let d_e2 = ((dx - ex2) * (dx - ex2) + (dy - ey2) * (dy - ey2)).sqrt();
            let mut d_arc7d = d_s2.min(d_e2);
            if p7d > 0.0 && th <= sw7d {
                d_arc7d = d_arc7d.min(d2_t);
            }
            let a2_arc = if p7d > 0.0 {
                (0.5 - (d_arc7d - w2 * 0.5)).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let idx = (py * buf_w + px) as usize;
            let current = buffer[idx];
            let bg_r = ((current >> 16) & 0xFF) as f32;
            let bg_g = ((current >> 8) & 0xFF) as f32;
            let bg_b = (current & 0xFF) as f32;

            let mut out_r = bg_r;
            let mut out_g = bg_g;
            let mut out_b = bg_b;

            // Blend track 5h
            let alpha1_t = a1_t * track_alpha;
            out_r = out_r * (1.0 - alpha1_t) + (track_rgb.0 as f32) * alpha1_t;
            out_g = out_g * (1.0 - alpha1_t) + (track_rgb.1 as f32) * alpha1_t;
            out_b = out_b * (1.0 - alpha1_t) + (track_rgb.2 as f32) * alpha1_t;

            // Blend arc 5h
            out_r = out_r * (1.0 - a1_arc) + (c5h_rgb.0 as f32) * a1_arc;
            out_g = out_g * (1.0 - a1_arc) + (c5h_rgb.1 as f32) * a1_arc;
            out_b = out_b * (1.0 - a1_arc) + (c5h_rgb.2 as f32) * a1_arc;

            // Blend track 7d
            let alpha2_t = a2_t * track_alpha;
            out_r = out_r * (1.0 - alpha2_t) + (track_rgb.0 as f32) * alpha2_t;
            out_g = out_g * (1.0 - alpha2_t) + (track_rgb.1 as f32) * alpha2_t;
            out_b = out_b * (1.0 - alpha2_t) + (track_rgb.2 as f32) * alpha2_t;

            // Blend arc 7d
            out_r = out_r * (1.0 - a2_arc) + (c7d_rgb.0 as f32) * a2_arc;
            out_g = out_g * (1.0 - a2_arc) + (c7d_rgb.1 as f32) * a2_arc;
            out_b = out_b * (1.0 - a2_arc) + (c7d_rgb.2 as f32) * a2_arc;

            let out_r = (out_r.round() as u32).min(255);
            let out_g = (out_g.round() as u32).min(255);
            let out_b = (out_b.round() as u32).min(255);

            buffer[idx] = 0xFF000000 | (out_r << 16) | (out_g << 8) | out_b;
        }
    }
}

/// Post-processing pass for the layered window bitmap buffer.
/// Applies mathematical subpixel anti-aliasing to the rounded capsule edges,
/// blends the subtle 1px border without jagged GDI artifacts, and sets
/// premultiplied alpha channels for clean taskbar blending.
pub fn apply_capsule_and_alpha(
    pixel_data: &mut [u32],
    width: i32,
    height: i32,
    corner_r: i32,
    is_dark: bool,
) {
    let (cap_bg_rgb, border_rgb) = if is_dark {
        ((30u8, 30u8, 34u8), (60u8, 60u8, 65u8))
    } else {
        ((251u8, 250u8, 250u8), (220u8, 222u8, 228u8))
    };
    let cap_r = cap_bg_rgb.0 as u32;
    let cap_g = cap_bg_rgb.1 as u32;
    let cap_b = cap_bg_rgb.2 as u32;
    let cap_dib_rgb = (cap_r << 16) | (cap_g << 8) | cap_b;

    let alpha_capsule = 220u32;
    let premul_r = (cap_r * alpha_capsule + 127) / 255;
    let premul_g = (cap_g * alpha_capsule + 127) / 255;
    let premul_b = (cap_b * alpha_capsule + 127) / 255;
    let frosted_pixel = (alpha_capsule << 24) | (premul_r << 16) | (premul_g << 8) | premul_b;

    let w_f = width as f32;
    let h_f = height as f32;
    let r_f = corner_r as f32;

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            let px = &mut pixel_data[idx];

            let dist = sd_rounded_box(x as f32 + 0.5, y as f32 + 0.5, w_f, h_f, r_f);

            if dist >= 0.5 {
                // Fully outside capsule -> 100% transparent
                *px = 0x00000000;
            } else if dist > -1.0 {
                // Outer 1.5px anti-aliased border edge
                let edge_t = (0.5 - dist).clamp(0.0, 1.0);
                let border_alpha = (alpha_capsule as f32 * edge_t).round() as u32;
                let pr = (border_rgb.0 as u32 * border_alpha + 127) / 255;
                let pg = (border_rgb.1 as u32 * border_alpha + 127) / 255;
                let pb = (border_rgb.2 as u32 * border_alpha + 127) / 255;
                *px = (border_alpha << 24) | (pr << 16) | (pg << 8) | pb;
            } else {
                // Inside the capsule
                let rgb = *px & 0x00FFFFFF;
                if rgb == cap_dib_rgb {
                    // Unmodified background pixel -> frosted glass
                    *px = frosted_pixel;
                } else if (*px >> 24) == 0 {
                    // GDI foreground text or lines -> fully opaque premultiplied
                    *px = 0xFF000000 | rgb;
                }
                // Ring rasterizer pixels already have premultiplied alpha (>= 0xFF000000), preserve them
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_display_parts_chinese_and_english() {
        // Chinese locale formats from poller
        let (pct, time) = parse_display_parts("剩余86%  22:05重置");
        assert_eq!(pct, "86%");
        assert_eq!(time, "22:05");

        let (pct, time) = parse_display_parts("≈56%  10/12重置");
        assert_eq!(pct, "56%");
        assert_eq!(time, "10/12");

        let (pct, time) = parse_display_parts("剩100%  18:30重置");
        assert_eq!(pct, "100%");
        assert_eq!(time, "18:30");

        let (pct, time) = parse_display_parts("剩余15%");
        assert_eq!(pct, "15%");
        assert_eq!(time, "");

        // English locale formats
        let (pct, time) = parse_display_parts("100% · 21:00");
        assert_eq!(pct, "100%");
        assert_eq!(time, "21:00");

        let (pct, time) = parse_display_parts("86% · 10/12");
        assert_eq!(pct, "86%");
        assert_eq!(time, "10/12");

        let (pct, time) = parse_display_parts("56% · 2h 15m");
        assert_eq!(pct, "56%");
        assert_eq!(time, "2h 15m");

        let (pct, time) = parse_display_parts("--");
        assert_eq!(pct, "--");
        assert_eq!(time, "");
    }

    #[test]
    fn test_sd_rounded_box_geometry() {
        let w = 100.0;
        let h = 36.0;
        let r = 12.0;

        // Center should be deep inside (negative)
        assert!(sd_rounded_box(50.0, 18.0, w, h, r) < -10.0);

        // Outside should be positive
        assert!(sd_rounded_box(-5.0, 18.0, w, h, r) > 0.0);
        assert!(sd_rounded_box(105.0, 18.0, w, h, r) > 0.0);
        assert!(sd_rounded_box(50.0, -5.0, w, h, r) > 0.0);
        assert!(sd_rounded_box(50.0, 45.0, w, h, r) > 0.0);

        // Corner point (0, 0) should be outside because corner is rounded
        assert!(sd_rounded_box(0.0, 0.0, w, h, r) > 0.0);
    }

    #[test]
    fn test_resolve_model_colors_colorful_and_monochrome() {
        // Colorful mode
        let (codex_5h, codex_7d) = resolve_model_colors(ModelKind::Codex, ColorMode::Colorful, false, 50.0, 50.0);
        assert_eq!(codex_5h, (16, 185, 129));
        assert_eq!(codex_7d, (245, 158, 11));

        let (agy_5h, agy_7d) = resolve_model_colors(ModelKind::Antigravity, ColorMode::Colorful, false, 50.0, 50.0);
        assert_eq!(agy_5h, (59, 130, 246));
        assert_eq!(agy_7d, (245, 158, 11));

        // Low remaining threshold (used >= 80% means remaining <= 20%) -> warning red
        let (c5h_warn, _) = resolve_model_colors(ModelKind::Codex, ColorMode::Colorful, false, 85.0, 50.0);
        assert_eq!(c5h_warn, (239, 68, 68));

        // Monochrome mode (dark vs light)
        let (dark_5h, dark_7d) = resolve_model_colors(ModelKind::Codex, ColorMode::Monochrome, true, 50.0, 50.0);
        assert_eq!(dark_5h, (255, 255, 255));
        assert_eq!(dark_7d, (160, 174, 192));

        let (light_5h, light_7d) = resolve_model_colors(ModelKind::Codex, ColorMode::Monochrome, false, 50.0, 50.0);
        assert_eq!(light_5h, (30, 41, 59));
        assert_eq!(light_7d, (100, 116, 139));
    }
}
