use eframe::egui::{Color32, KeyboardShortcut, RichText, Stroke};
use egui_taffy::{Tui, TuiBuilder, TuiBuilderLogic, TuiInnerResponse};

use crate::{
    settings_parsing::format_mac_shortcut_with_symbols,
    theme::{AppIcon, AppTheme},
};

#[derive(Debug, Clone, Copy)]
pub enum IconButtonSize {
    Small,
    Medium,
    Large,
}

impl IconButtonSize {
    pub fn get_icon_font_size(&self, theme: &AppTheme) -> f32 {
        match self {
            IconButtonSize::Small => theme.fonts.size.small,
            IconButtonSize::Medium => theme.fonts.size.normal,
            IconButtonSize::Large => theme.sizes.toolbar_icon,
        }
    }
}

pub fn rich_text_tooltip(
    tooltip_text: &str,
    shortcut: Option<KeyboardShortcut>,
    theme: &AppTheme,
) -> RichText {
    RichText::new(match shortcut {
        Some(shortcut) => {
            format!(
                "{} ({})",
                tooltip_text,
                format_mac_shortcut_with_symbols(shortcut)
            )
        }
        None => tooltip_text.to_string(),
    })
    .color(theme.colors.subtle_text_color)
}

pub fn apply_icon_button_styling(tui: &mut Tui) -> TuiBuilder {
    tui.mut_egui_style(|style| {
        style.visuals.widgets.active.bg_stroke = Stroke::NONE;
        style.visuals.widgets.hovered.bg_stroke = Stroke::NONE;
        style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        style.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
    })
}

pub fn render_icon_button(
    tui: &mut Tui,
    icon: AppIcon,
    size: IconButtonSize,
    theme: &AppTheme,
    tooltip: Option<(&str, Option<KeyboardShortcut>)>,
) -> TuiInnerResponse<eframe::egui::Response> {
    let icon_size = size.get_icon_font_size(theme);

    apply_icon_button_styling(tui).button(|tui| {
        let label = tui.label(icon.render(icon_size, theme.colors.subtle_text_color));

        if let Some((tooltip_text, shortcut)) = tooltip {
            label.on_hover_ui(|ui| {
                ui.label(rich_text_tooltip(tooltip_text, shortcut, theme));
            })
        } else {
            label
        }
    })
}

// Helper function to render toggle icon button - to be called inside tui.show(|t| { ... })
pub fn render_icon_toggle_button(
    tui: &mut Tui,
    icon: AppIcon,
    size: IconButtonSize,
    is_toggled: bool,
    theme: &AppTheme,
    tooltip: Option<(&str, Option<KeyboardShortcut>)>,
) -> TuiInnerResponse<eframe::egui::Response> {
    let icon_size = size.get_icon_font_size(theme);
    let icon_color = if is_toggled {
        theme.colors.button_pressed_fg
    } else {
        theme.colors.subtle_text_color
    };

    apply_icon_button_styling(tui).button(|tui| {
        let label = tui.label(icon.render(icon_size, icon_color));

        if let Some((tooltip_text, shortcut)) = tooltip {
            label.on_hover_ui(|ui| {
                ui.label(rich_text_tooltip(tooltip_text, shortcut, theme));
            })
        } else {
            label
        }
    })
}
