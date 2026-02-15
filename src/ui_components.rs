use std::sync::Arc;

use eframe::egui::{
    Color32, FontId, KeyboardShortcut, RichText, Stroke, TextFormat, TextWrapMode, Widget,
    WidgetText, text::LayoutJob, vec2,
};
use egui_taffy::{AsTuiBuilder, Tui, TuiBuilder, TuiBuilderLogic, TuiInnerResponse, TuiWidget};

use crate::{
    settings_parsing::format_mac_shortcut_with_symbols,
    taffy_styles::{StyleBuilder, style},
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
) -> WidgetText {
    match shortcut {
        Some(shortcut) => WidgetText::LayoutJob(
            {
                let mut job = LayoutJob::default();

                let normal_font_id = FontId {
                    size: theme.fonts.size.normal,
                    family: theme.fonts.family.normal.clone(),
                };

                job.append(
                    &format!("{tooltip_text}  "),
                    0.0,
                    TextFormat::simple(normal_font_id.clone(), theme.colors.subtle_text_color),
                );

                job.append(
                    &format_mac_shortcut_with_symbols(shortcut),
                    0.0,
                    TextFormat::simple(normal_font_id, theme.colors.outline_fg),
                );
                job
            }
            .into(),
        ),

        None => WidgetText::RichText(
            RichText::new(tooltip_text.to_string())
                .color(theme.colors.subtle_text_color)
                .into(),
        ),
    }
}

pub fn apply_icon_btn_styling(style: &mut eframe::egui::Style) {
    style.visuals.widgets.active.bg_stroke = Stroke::NONE;
    style.visuals.widgets.hovered.bg_stroke = Stroke::NONE;
    style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
    // style.spacing.button_padding = vec2(1.0, 1.0);
    style.wrap_mode = Some(TextWrapMode::Extend);
}

/// IconButton widget that implements TuiWidget trait with builder pattern and fade animation
pub struct IconButton<'theme> {
    icon: AppIcon,
    size: IconButtonSize,
    tooltip: Option<(String, Option<KeyboardShortcut>)>,
    text: Option<String>,
    text_size: f32,
    fade: f32,
    is_toggled: bool,
    theme: &'theme AppTheme,
    color: Option<Color32>,
}

impl<'theme> IconButton<'theme> {
    /// Create a new IconButton with required icon parameter
    pub fn new(icon: AppIcon, theme: &'theme AppTheme) -> Self {
        Self {
            icon,
            theme,
            size: IconButtonSize::Medium,
            tooltip: None,
            text: None,
            fade: 1.0,
            is_toggled: false,
            color: None,
            text_size: theme.fonts.size.normal,
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

    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
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
            color,
            text,
            text_size,
        } = self;

        {
            let icon_size = size.get_icon_font_size(theme);

            let desired_fg_color = get_button_fg_color(is_toggled, theme, color);

            let icon_color = theme
                .colors
                .subtle_text_color
                // .gamma_multiply(0.2)
                .lerp_to_gamma(desired_fg_color, fade);

            tui.mut_egui_style(apply_icon_btn_styling)
                // FIXME padding is not taken from egui.styles()
                // .style(style().padding(0.))
                .button(|tui| {
                    let label = if let Some(text) = text.as_ref() {
                        tui.label(
                            icon.render_with_text_size(icon_size, text_size, icon_color, text),
                        )
                    } else {
                        tui.label(icon.render(icon_size, icon_color))
                    };

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

pub fn get_button_fg_color(is_toggled: bool, theme: &AppTheme, color: Option<Color32>) -> Color32 {
    let base_color = if let Some(color) = color {
        color
    } else if is_toggled {
        theme.colors.button_pressed_fg
    } else {
        theme.colors.subtle_text_color
    };
    base_color
}

impl<'theme> Widget for IconButton<'theme> {
    fn ui(self, ui: &mut eframe::egui::Ui) -> eframe::egui::Response {
        let Self {
            icon,
            size,
            tooltip,
            fade,
            is_toggled,
            theme,
            color,
            text,
            text_size,
        } = self;

        let icon_size = size.get_icon_font_size(theme);

        let base_color = if let Some(color) = color {
            color
        } else if is_toggled {
            theme.colors.button_pressed_fg
        } else {
            theme.colors.subtle_text_color
        };

        let icon_color = theme
            .colors
            .subtle_text_color
            .gamma_multiply(0.2)
            .lerp_to_gamma(base_color, fade);

        apply_icon_btn_styling(ui.style_mut());

        let label = if let Some(text) = text.as_ref() {
            ui.button(icon.render_with_text_size(icon_size, text_size, icon_color, text))
        } else {
            ui.button(icon.render(icon_size, icon_color))
        };

        if let Some((tooltip_text, shortcut)) = tooltip.as_ref() {
            label.on_hover_ui(|ui| {
                ui.label(rich_text_tooltip(tooltip_text, shortcut.clone(), theme));
            })
        } else {
            label
        }
    }
}

// ============================================================================
// Filename truncation
// ============================================================================

/// Configuration for truncating long filenames
#[derive(Debug, Clone, Copy)]
pub struct FilenameCapConfig {
    /// Maximum total characters allowed
    pub max_chars: usize,
    /// Number of characters to preserve from the beginning (before ..)
    pub offset_from_beginning: usize,
}

impl Default for FilenameCapConfig {
    fn default() -> Self {
        Self {
            max_chars: 20,
            offset_from_beginning: 4,
        }
    }
}

/// Truncates a filename to a maximum length while preserving the extension.
///
/// Examples:
/// - `too_very_long.md` -> `too_..long.md` (with offset=4, max=15)
/// - `short.md` -> `short.md` (no truncation needed)
/// - `no_extension` -> `no_e..ion_here` (no extension to preserve)
pub fn truncate_filename(filename: &str, config: FilenameCapConfig) -> String {
    let FilenameCapConfig {
        max_chars,
        offset_from_beginning,
    } = config;

    if filename.len() <= max_chars {
        return filename.to_string();
    }

    let separator = "..";

    // Find the last dot to identify extension
    if let Some(dot_pos) = filename.rfind('.') {
        let name = &filename[..dot_pos];
        let ext = &filename[dot_pos..]; // includes the dot

        // Check if extension is too long to preserve
        let min_required = offset_from_beginning + separator.len() + ext.len();

        if min_required > max_chars {
            // Extension is too long, truncate without preserving it
            let end_chars = max_chars.saturating_sub(offset_from_beginning + separator.len());
            let start = &filename[..offset_from_beginning.min(filename.len())];
            let end = if end_chars > 0 && filename.len() > offset_from_beginning + separator.len() {
                &filename[filename.len().saturating_sub(end_chars)..]
            } else {
                ""
            };
            format!("{}{}{}", start, separator, end)
        } else {
            // We can preserve the extension
            // Calculate how many chars we have left for the name (excluding separator and extension)
            let available_for_name = max_chars.saturating_sub(separator.len() + ext.len());

            // Start takes offset_from_beginning chars, but not more than available
            let start_len = offset_from_beginning
                .min(available_for_name)
                .min(name.len());

            // End takes the remaining available chars from the end of the name
            let end_len = available_for_name.saturating_sub(start_len);

            let start = &name[..start_len];
            let end = if end_len > 0 && name.len() > start_len {
                let end_start_pos = name.len().saturating_sub(end_len);
                &name[end_start_pos..]
            } else {
                ""
            };

            format!("{}{}{}{}", start, separator, end, ext)
        }
    } else {
        // No extension found
        let end_chars = max_chars.saturating_sub(offset_from_beginning + separator.len());
        let start = &filename[..offset_from_beginning.min(filename.len())];
        let end = if end_chars > 0 && filename.len() > offset_from_beginning + separator.len() {
            &filename[filename.len().saturating_sub(end_chars)..]
        } else {
            ""
        };
        format!("{}{}{}", start, separator, end)
    }
}
