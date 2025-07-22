use eframe::egui::{Color32, KeyboardShortcut, RichText, Stroke};
use egui_taffy::{AsTuiBuilder, Tui, TuiBuilder, TuiBuilderLogic, TuiInnerResponse, TuiWidget};

use crate::{
    settings_parsing::format_mac_shortcut_with_symbols,
    theme::{AppIcon, AppTheme},
};

#[derive(Debug, Clone, Copy)]
pub enum IconButtonSize {
    Small,
    Medium,
    Large,
    ExtraLarge,
}

impl IconButtonSize {
    pub fn get_icon_font_size(&self, theme: &AppTheme) -> f32 {
        match self {
            IconButtonSize::Small => theme.fonts.size.small,
            IconButtonSize::Medium => theme.fonts.size.normal,
            IconButtonSize::Large => theme.sizes.toolbar_icon,
            IconButtonSize::ExtraLarge => theme.fonts.size.h3,
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

pub fn apply_icon_btn_styling(style: &mut eframe::egui::Style) {
    style.visuals.widgets.active.bg_stroke = Stroke::NONE;
    style.visuals.widgets.hovered.bg_stroke = Stroke::NONE;
    style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
}

/// IconButton widget that implements TuiWidget trait with builder pattern and fade animation
pub struct IconButton<'theme> {
    icon: AppIcon,
    size: IconButtonSize,
    tooltip: Option<(String, Option<KeyboardShortcut>)>,
    fade: f32,
    is_toggled: bool,
    theme: &'theme AppTheme,
}

impl<'theme> IconButton<'theme> {
    /// Create a new IconButton with required icon parameter
    pub fn new(icon: AppIcon, theme: &'theme AppTheme) -> Self {
        Self {
            icon,
            theme,
            size: IconButtonSize::Medium,
            tooltip: None,
            fade: 1.0,
            is_toggled: false,
        }
    }

    /// Set the button size
    pub fn size(mut self, size: IconButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Set the tooltip text and optional keyboard shortcut
    pub fn tooltip(mut self, text: impl Into<String>, shortcut: Option<KeyboardShortcut>) -> Self {
        self.tooltip = Some((text.into(), shortcut));
        self
    }

    /// Set the fade value from 0.0 to 1.0 for animations
    pub fn fade(mut self, fade: f32) -> Self {
        self.fade = fade.clamp(0.0, 1.0);
        self
    }

    /// Set whether the button is in a toggled state
    pub fn toggled(mut self, is_toggled: bool) -> Self {
        self.is_toggled = is_toggled;
        self
    }
}

impl<'theme> TuiWidget for IconButton<'theme> {
    type Response = eframe::egui::Response;

    fn taffy_ui(self, tuib: TuiBuilder) -> Self::Response {
        let tui = tuib.tui();
        let Self {
            icon,
            size,
            tooltip,
            fade,
            is_toggled,
            theme,
        } = self;

        {
            let icon_size = size.get_icon_font_size(theme);

            let base_color = if is_toggled {
                theme.colors.button_pressed_fg
            } else {
                theme.colors.subtle_text_color
            };

            // Apply fade animation using gamma_multiply and lerp_to_gamma pattern
            let icon_color = theme
                .colors
                .subtle_text_color
                .gamma_multiply(0.2)
                .lerp_to_gamma(base_color, fade);

            tui.mut_egui_style(apply_icon_btn_styling)
                .button(|tui| {
                    let label = tui.label(icon.render(icon_size, icon_color));

                    if let Some((tooltip_text, shortcut)) = tooltip.as_ref() {
                        label.on_hover_ui(|ui| {
                            ui.label(rich_text_tooltip(tooltip_text, shortcut.clone(), theme));
                        })
                    } else {
                        label
                    }
                })
                .response
        }
    }
}
