// SPDX-License-Identifier: MIT OR Apache-2.0
//! A theme as egui's visuals and style.

use egui::{Color32, Context, CornerRadius, Stroke, Style, Visuals, vec2};

use crate::tokens::Theme;

/// egui's visuals for `t`.
pub fn visuals(t: &Theme) -> Visuals {
    let mut v = if t.dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = t.surface;
    v.window_fill = t.surface_high;
    v.extreme_bg_color = t.input;
    v.faint_bg_color = t.hover;
    v.window_stroke = Stroke::new(1.0, t.border);
    v.selection.bg_fill = t.accent_soft;
    v.selection.stroke = Stroke::new(1.0, t.accent);
    v.hyperlink_color = t.accent;
    v.error_fg_color = t.danger;
    v.override_text_color = None;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, t.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, t.text);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, t.text);
    v.widgets.hovered.weak_bg_fill = t.hover;
    v.widgets.hovered.bg_fill = t.hover;
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, t.text);
    v.widgets.active.fg_stroke = Stroke::new(1.5, t.text);
    v.widgets.open.weak_bg_fill = t.accent_soft;
    let control = CornerRadius::same(t.radius.control);
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = control;
    }
    v.window_corner_radius = CornerRadius::same(t.radius.window);
    v.menu_corner_radius = CornerRadius::same(t.radius.window);
    v.window_shadow.color = Color32::from_black_alpha(if t.dark { 96 } else { 40 });
    v.popup_shadow.color = Color32::from_black_alpha(if t.dark { 96 } else { 40 });
    v
}

/// egui's style for `t`: `base` with the theme's visuals and spacing.
pub fn style(t: &Theme, base: &Style) -> Style {
    let mut s = base.clone();
    s.visuals = visuals(t);
    s.spacing.item_spacing = vec2(t.spacing.item_x, t.spacing.item_y);
    s.spacing.button_padding = vec2(t.spacing.button_x, t.spacing.button_y);
    s.spacing.indent = t.spacing.indent;
    s.spacing.window_margin = egui::Margin::same(t.spacing.panel_margin.round().clamp(0.0, 127.0) as i8);
    for (text_style, size) in [
        (egui::TextStyle::Body, t.type_sizes.body),
        (egui::TextStyle::Button, t.type_sizes.body),
        (egui::TextStyle::Small, t.type_sizes.caption),
        (egui::TextStyle::Heading, t.type_sizes.title + 4.0),
        (egui::TextStyle::Monospace, t.type_sizes.body),
    ] {
        if let Some(font) = s.text_styles.get_mut(&text_style) {
            font.size = size;
        }
    }
    s
}

/// Put `t` in use: egui's style for both of its themes (so egui's own light/dark switch shows
/// this theme too), and [`Theme::get`] for the interface's own drawing.
pub fn apply(ctx: &Context, t: &Theme) {
    let base = ctx.global_style();
    let s = style(t, &base);
    ctx.set_style_of(egui::Theme::Light, s.clone());
    ctx.set_style_of(egui::Theme::Dark, s);
    ctx.set_theme(if t.dark { egui::Theme::Dark } else { egui::Theme::Light });
    t.set(ctx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Mode, Tokens};

    #[test]
    fn visuals_follow_the_tokens() {
        let tokens = Tokens::default();
        let dark = tokens.theme(Mode::Dark);
        let v = visuals(&dark);
        assert!(v.dark_mode);
        assert_eq!(v.panel_fill, tokens.dark.surface);
        assert_eq!(v.selection.stroke.color, tokens.dark.accent);
        assert_eq!(v.window_corner_radius, CornerRadius::same(tokens.radius.window));
        let ctx = Context::default();
        apply(&ctx, &dark);
        assert_eq!(Theme::get(&ctx), dark);
        assert_eq!(ctx.style_of(egui::Theme::Light).visuals.panel_fill, tokens.dark.surface);
        assert_eq!(ctx.style_of(ctx.theme()).spacing.indent, tokens.spacing.indent);
    }
}
