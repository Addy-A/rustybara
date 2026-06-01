//! User-configurable styling for the prepress box overlays (Trim / Bleed / Crop).
//!
//! Each box overlay carries its own color, thickness, and dash setting, edited
//! live from the rbv tools panel. Keeping the styles *per box* preserves the
//! at-a-glance color identity of each prepress box. Defaults reproduce the
//! original hard-coded look, so the controls change nothing until adjusted:
//!   - Trim  → blue   (0, 160, 255  @ alpha 220)
//!   - Bleed → orange (255, 100, 0  @ alpha 220)
//!   - Crop  → green  (0, 200, 80   @ alpha 200)
//! all 1.5px wide, dashed (`[6, 4]`).

use egui::Color32;
use skia_safe::Paint;

/// Stroke styling for a single box overlay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxOverlayStyle {
    /// sRGBA stroke color (alpha respected); bound by the egui color picker.
    pub color: Color32,
    /// Stroke width in screen pixels.
    pub thickness: f32,
    /// Dashed when true, solid when false.
    pub dashed: bool,
}

impl BoxOverlayStyle {
    /// Construct an explicit style. (Used by `Default` for `BoxOverlayStyles`
    /// and by tests.)
    pub fn new(color: Color32, thickness: f32, dashed: bool) -> Self {
        Self {
            color,
            thickness,
            dashed,
        }
    }

    /// Build the Skia stroke `Paint` for this box: `Style::Stroke`, this `color`
    /// (alpha included) and `thickness`, plus a `[6, 4]` dash `PathEffect` when
    /// `dashed` is set (solid otherwise).
    pub fn to_stroke_paint(&self) -> Paint {
        let mut paint = Paint::default();
        paint.set_style(skia_safe::paint::Style::Stroke);
        paint.set_stroke_width(self.thickness);
        // Color32 stores premultiplied alpha; unmultiply so the on-screen color
        // matches what the user picked (and the original defaults round-trip).
        let [r, g, b, a] = self.color.to_srgba_unmultiplied();
        paint.set_color(skia_safe::Color::from_argb(a, r, g, b));
        if self.dashed {
            paint.set_path_effect(skia_safe::dash_path_effect::new(&[6.0, 4.0], 0.0));
        }
        paint
    }
}

/// Per-box overlay styles for the three prepress boxes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxOverlayStyles {
    /// Trim box overlay style.
    pub trim: BoxOverlayStyle,
    /// Bleed box overlay style.
    pub bleed: BoxOverlayStyle,
    /// Crop box overlay style.
    pub crop: BoxOverlayStyle,
}

impl Default for BoxOverlayStyles {
    /// Reproduces the original hard-coded overlay look (see module docs).
    fn default() -> Self {
        Self {
            trim: BoxOverlayStyle::new(Color32::from_rgba_unmultiplied(0, 160, 255, 220), 1.5, true),
            bleed: BoxOverlayStyle::new(
                Color32::from_rgba_unmultiplied(255, 100, 0, 220),
                1.5,
                true,
            ),
            crop: BoxOverlayStyle::new(Color32::from_rgba_unmultiplied(0, 200, 80, 200), 1.5, true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_original_colors() {
        let s = BoxOverlayStyles::default();
        assert_eq!(
            s.trim.to_stroke_paint().color(),
            skia_safe::Color::from_argb(220, 0, 160, 255),
            "trim default should be the original blue"
        );
        assert_eq!(
            s.bleed.to_stroke_paint().color(),
            skia_safe::Color::from_argb(220, 255, 100, 0),
            "bleed default should be the original orange"
        );
        assert_eq!(
            s.crop.to_stroke_paint().color(),
            skia_safe::Color::from_argb(200, 0, 200, 80),
            "crop default should be the original green"
        );
    }

    #[test]
    fn defaults_are_dashed_and_1_5px() {
        let s = BoxOverlayStyles::default();
        for paint in [
            s.trim.to_stroke_paint(),
            s.bleed.to_stroke_paint(),
            s.crop.to_stroke_paint(),
        ] {
            assert_eq!(paint.style(), skia_safe::paint::Style::Stroke);
            assert!(
                (paint.stroke_width() - 1.5).abs() < 1e-6,
                "default thickness should be 1.5"
            );
            assert!(paint.path_effect().is_some(), "default boxes are dashed");
        }
    }

    #[test]
    fn solid_has_no_path_effect_and_keeps_thickness() {
        let solid = BoxOverlayStyle::new(Color32::WHITE, 2.0, false).to_stroke_paint();
        assert!(solid.path_effect().is_none(), "solid must not set a dash");
        assert!((solid.stroke_width() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn color_alpha_is_preserved() {
        let p = BoxOverlayStyle::new(Color32::from_rgba_unmultiplied(10, 20, 30, 128), 1.0, false)
            .to_stroke_paint();
        assert_eq!(p.color(), skia_safe::Color::from_argb(128, 10, 20, 30));
    }
}
