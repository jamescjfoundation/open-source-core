// SPDX-License-Identifier: MIT OR Apache-2.0
//! A card as both views draw it.

use egui::{Align2, Color32, FontId, Painter, Rect, Stroke, StrokeKind, vec2};
use jcj_theme::Theme;

use crate::model::{COLORS, Card};
use crate::text::{heading_preview, preview};

/// The ink on a card: dark, since card colors are pale in every theme.
const INK: Color32 = Color32::from_gray(20);

/// A card's color, or the first of [`COLORS`] when it is not `#RRGGBB`.
pub fn card_color(hex: &str) -> Color32 {
    jcj_theme::parse_hex(hex).or_else(|_| jcj_theme::parse_hex(COLORS[0])).unwrap_or(Color32::from_rgb(0xFF, 0xF2, 0xA8))
}

/// Paint `card` filling `r`: its color, a border (thicker in the accent color when `selected`),
/// its label at the top right, its title and as much of its body as fits.
pub fn paint_card(p: &Painter, r: Rect, card: &Card, selected: bool, t: &Theme) {
    let radius = f32::from(t.radius.card);
    p.rect_filled(r, radius, card_color(&card.color));
    let stroke = if selected { Stroke::new(2.5, t.accent) } else { Stroke::new(1.0, Color32::from_black_alpha(60)) };
    p.rect_stroke(r, radius, stroke, StrokeKind::Inside);
    let inner = r.shrink2(vec2(8.0, 6.0));
    let mut top = inner.min.y;
    let mut title_w = inner.width();
    if !card.label.is_empty() {
        let g = p.layout_no_wrap(card.label.clone(), FontId::proportional(10.5), INK);
        let chip = Rect::from_min_size(egui::pos2(inner.max.x - g.size().x - 8.0, inner.min.y), g.size() + vec2(8.0, 2.0));
        p.rect_filled(chip, 6.0, Color32::from_black_alpha(28));
        p.galley(chip.min + vec2(4.0, 1.0), g, INK);
        title_w -= chip.width() + 4.0;
    }
    let title = if card.title.is_empty() { "Untitled card".to_string() } else { heading_preview(&card.title) };
    let title_color = if card.title.is_empty() { INK.gamma_multiply(0.55) } else { INK };
    let tg = p.layout(title, FontId::proportional(14.0), title_color, title_w.max(20.0));
    let title_h = tg.size().y;
    p.galley(egui::pos2(inner.min.x, top), tg, title_color);
    top += title_h + 4.0;
    if !card.body.is_empty() && top < inner.max.y {
        let body = p.layout(preview(&card.body, 400), FontId::proportional(12.0), INK, inner.width());
        let body_rect = Rect::from_min_max(egui::pos2(inner.min.x, top), inner.max);
        p.with_clip_rect(body_rect.intersect(p.clip_rect())).galley(body_rect.min, body, INK);
    }
}

/// Paint a small text hint at the center of `r` (an empty board's message).
pub(crate) fn hint(p: &Painter, r: Rect, text: &str, t: &Theme) {
    p.text(r.center(), Align2::CENTER_CENTER, text, FontId::proportional(t.type_sizes.body), t.text_dim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_fall_back_to_the_first_card_color() {
        assert_eq!(card_color("#B8D4F5"), Color32::from_rgb(0xB8, 0xD4, 0xF5));
        assert_eq!(card_color("nope"), Color32::from_rgb(0xFF, 0xF2, 0xA8));
    }
}
