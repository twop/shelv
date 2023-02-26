use std::collections::BTreeMap;

use eframe::{
    egui::{
        self,
        style::{Selection, WidgetVisuals, Widgets},
        FontDefinitions, Margin, TextStyle, Visuals,
    },
    epaint::{Color32, FontFamily, FontId, Rounding, Shadow, Stroke},
};

use crate::nord::Nord;

pub struct AppTheme {
    pub fonts: FontTheme,
    pub colors: ColorTheme,
}

impl AppTheme {
    #[no_mangle]
    pub fn nord() -> Self {
        Self {
            fonts: FontTheme::default(),
            colors: ColorTheme::nord(),
        }
    }
}

impl Default for AppTheme {
    fn default() -> Self {
        Self::nord()
    }
}

pub struct FontTheme {
    pub h1: FontId,
    pub h2: FontId,
    pub h3: FontId,
    pub h4: FontId,
    pub body: FontId,
    pub code: FontId,
    pub button: FontId,
    pub small: FontId,
    pub bold_family: FontFamily,
}

impl Default for FontTheme {
    fn default() -> Self {
        Self {
            h1: FontId::proportional(24.),
            h2: FontId::proportional(20.),
            h3: FontId::proportional(18.),
            h4: FontId::proportional(16.),
            body: FontId::proportional(14.),
            bold_family: FontFamily::Name("inter-bold".into()),
            code: FontId::monospace(14.),
            button: FontId::monospace(12.),
            small: FontId::monospace(8.),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ColorTheme {
    // ---------
    // editor specific colors
    pub md_strike: Color32,
    pub md_annotation: Color32,
    pub md_body: Color32,
    pub md_header: Color32,

    // ---------
    // egui settings and general colors
    pub rounding_controls: Rounding,
    pub rounding_window: Rounding,

    pub button_bg: Color32,
    pub button_bg_stroke: Color32,
    pub button_fg: Color32,
    pub button_hover_bg: Color32,
    pub button_hover_bg_stroke: Color32,
    pub button_hover_fg: Color32,

    pub button_pressed_bg: Color32,
    pub button_pressed_bg_stroke: Color32,
    pub button_pressed_fg: Color32,

    pub main_bg: Color32,
    pub outline_fg: Color32,
    pub selection_bg: Color32,
    pub selection_stroke: Color32,
    pub hyperlink_color: Color32,
    pub normal_text_color: Color32,
    pub subtle_text_color: Color32,

    // Something just barely different from the background color.
    // Used for [`crate::Grid::striped`].
    pub faint_bg_color: Color32,

    // Very dark or light color (for corresponding theme).
    // Used as the background of text edits, scroll bars and others things
    // that needs to look different from other interactive stuff.
    pub extreme_bg_color: Color32,

    // Background color behind code-styled monospaced labels.
    pub code_bg_color: Color32,

    // A good color for warning text (e.g. orange).
    pub warn_fg_color: Color32,

    // A good color for error text (e.g. red).
    pub error_fg_color: Color32,
}

impl ColorTheme {
    pub fn nord() -> Self {
        // ---------
        // editor specific colors
        let md_strike: Color32 = Nord::NORD4;
        let md_annotation: Color32 = Nord::NORD4;
        let md_body = Nord::NORD4;
        let md_header = Nord::NORD6;

        // ---------
        // egui settings and general colors
        let rounding_controls = Rounding::same(6.0);
        let rounding_window = Rounding::same(6.0);

        let button_bg = Nord::NORD1;
        let button_bg_stroke = Color32::TRANSPARENT; // Nord::NORD8.shade(0.7);
        let button_fg = Nord::NORD8;
        let button_hover_bg = Nord::NORD1.shade(0.95);
        let button_hover_bg_stroke = Nord::NORD8.shade(0.8);
        let button_hover_fg = Nord::NORD8.shade(1.1);

        let button_pressed_bg = Nord::NORD1.shade(0.9);
        let button_pressed_bg_stroke = Nord::NORD8.shade(0.9);
        let button_pressed_fg = Nord::NORD8.shade(1.2);

        let main_bg = Nord::NORD0.shade(0.8);
        let outline_fg = Nord::NORD1;
        let selection_bg = Nord::NORD1;
        let selection_stroke = Nord::NORD6;
        let hyperlink_color = Nord::NORD9;
        let normal_text_color = Nord::NORD4;
        let subtle_text_color = Nord::NORD4.shade(0.5);

        // Something just barely different from the background color.
        // Used for [`crate::Grid::striped`].
        let faint_bg_color = Nord::NORD0;

        // Very dark or light color (for corresponding theme).
        // Used as the background of text edits, scroll bars and others things
        // that needs to look different from other interactive stuff.
        let extreme_bg_color = Nord::NORD0.shade(0.6);

        // Background color behind code-styled monospaced labels.
        let code_bg_color = Nord::NORD0.shade(0.6);

        // A good color for warning text (e.g. orange).
        let warn_fg_color = Nord::NORD12;

        // A good color for error text (e.g. red).
        let error_fg_color = Nord::NORD11;

        Self {
            rounding_controls,
            rounding_window,
            button_bg,
            button_fg,
            button_hover_bg,
            button_hover_bg_stroke,
            button_hover_fg,
            button_pressed_bg,
            button_pressed_bg_stroke,
            button_pressed_fg,
            main_bg,
            outline_fg,
            selection_bg,
            selection_stroke,
            hyperlink_color,
            normal_text_color,
            faint_bg_color,
            extreme_bg_color,
            code_bg_color,
            warn_fg_color,
            error_fg_color,
            md_strike,
            md_annotation,
            button_bg_stroke,
            md_body,
            md_header,
            subtle_text_color,
        }
    }
}

#[no_mangle]
pub fn configure_styles(ctx: &egui::Context, theme: &AppTheme) {
    let fonts = get_font_definitions();
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();

    style.text_styles = text_styles(&theme.fonts);
    style.visuals = visuals(&theme.colors);
    // style.spacing.window_margin = Margin::same(0.);
    ctx.set_style(style);
}

fn get_font_definitions() -> FontDefinitions {
    // Start with the default fonts (we will be adding to them rather than replacing them).
    let mut fonts = FontDefinitions::default();

    // Install my own font (maybe supporting non-latin characters).
    // .ttf and .otf files supported.
    fonts.font_data.insert(
        "inter".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/Inter-Light.otf")),
    );

    fonts.font_data.insert(
        "inter-bold".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/Inter-SemiBold.otf")),
    );

    // Put my font first (highest priority) for proportional text:
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "inter".to_owned());

    // Put my font as last fallback for monospace:
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("inter".to_owned());

    fonts
        .families
        .entry(egui::FontFamily::Name("inter-bold".into()))
        .or_default()
        .push("inter-bold".to_owned());

    // Tell egui to use these fonts:
    fonts
}

fn text_styles(theme: &FontTheme) -> BTreeMap<TextStyle, FontId> {
    [
        (TextStyle::Heading, theme.h1.clone()),
        (TextStyle::Body, theme.body.clone()),
        (TextStyle::Monospace, theme.code.clone()),
        (TextStyle::Button, theme.button.clone()),
        (TextStyle::Small, theme.small.clone()),
    ]
    .into()
}

trait ColorManipulation {
    fn shade(self, by: f32) -> Self;
}

impl ColorManipulation for Color32 {
    fn shade(self, by: f32) -> Self {
        let [r, g, b, a] = self.to_array();

        Color32::from_rgba_premultiplied(
            (r as f32 * by) as u8,
            (g as f32 * by) as u8,
            (b as f32 * by) as u8,
            a,
        )
    }
}

fn visuals(color_theme: &ColorTheme) -> Visuals {
    let ColorTheme {
        rounding_controls,
        rounding_window,
        button_bg,
        button_fg,
        button_hover_bg,
        button_hover_bg_stroke,
        button_hover_fg,
        button_pressed_bg,
        button_pressed_bg_stroke,
        button_pressed_fg,
        main_bg,
        outline_fg,
        selection_bg,
        selection_stroke,
        hyperlink_color,
        normal_text_color,
        faint_bg_color,
        extreme_bg_color,
        code_bg_color,
        warn_fg_color,
        error_fg_color,
        button_bg_stroke,
        md_strike: _,
        md_annotation: _,
        md_body: _,
        md_header: _,
        subtle_text_color: _,
    } = color_theme.clone();

    // --- window ---
    let selection = Selection {
        bg_fill: selection_bg,
        stroke: Stroke::new(1.0, selection_stroke),
    };

    let widgets = Widgets {
        noninteractive: WidgetVisuals {
            weak_bg_fill: Color32::from_gray(27),
            bg_fill: Color32::from_gray(27),
            bg_stroke: Stroke::new(1.0, Color32::from_gray(60)), // separators, indentation lines
            fg_stroke: Stroke::new(1.0, normal_text_color),      // normal text color
            rounding: rounding_controls,
            expansion: 0.0,
        },
        inactive: WidgetVisuals {
            weak_bg_fill: button_bg, // button background
            bg_fill: button_bg,      // checkbox background
            bg_stroke: Stroke::new(1., button_bg_stroke),
            fg_stroke: Stroke::new(1.0, button_fg), // button text
            rounding: rounding_controls,
            expansion: 0.0,
        },
        hovered: WidgetVisuals {
            weak_bg_fill: button_hover_bg,
            bg_fill: button_hover_bg,
            bg_stroke: Stroke::new(1.0, button_hover_bg_stroke), // e.g. hover over window edge or button
            fg_stroke: Stroke::new(1.5, button_hover_fg),
            rounding: rounding_controls,
            expansion: 1.0,
        },
        active: WidgetVisuals {
            weak_bg_fill: button_pressed_bg,
            bg_fill: button_pressed_bg,
            bg_stroke: Stroke::new(1.0, button_pressed_bg_stroke),
            fg_stroke: Stroke::new(2.0, button_pressed_fg),
            rounding: rounding_controls,
            expansion: 1.0,
        },
        open: WidgetVisuals {
            weak_bg_fill: Color32::from_gray(27),
            bg_fill: Color32::from_gray(27),
            bg_stroke: Stroke::new(1.0, Color32::from_gray(60)),
            fg_stroke: Stroke::new(1.0, Color32::from_gray(210)),
            rounding: rounding_controls,
            expansion: 0.0,
        },
    };

    Visuals {
        dark_mode: true,
        override_text_color: None,
        selection,
        hyperlink_color,
        faint_bg_color,
        extreme_bg_color,
        code_bg_color,
        warn_fg_color,
        error_fg_color,
        window_rounding: rounding_window,
        window_shadow: Shadow::small_dark(),
        window_fill: main_bg,
        window_stroke: Stroke {
            width: 0.5,
            color: outline_fg,
        },
        menu_rounding: rounding_window,
        panel_fill: main_bg,
        popup_shadow: Shadow::small_dark(),
        resize_corner_size: 12.,
        text_cursor_width: 2.0,
        text_cursor_preview: false,
        clip_rect_margin: 3.,
        button_frame: true,
        collapsing_header_frame: false,
        indent_has_left_vline: true,
        striped: false,
        slider_trailing_fill: false,
        widgets,
    }
}
