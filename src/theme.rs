use std::collections::BTreeMap;

use eframe::{
    egui::{
        self,
        style::{Selection, WidgetVisuals, Widgets},
        FontDefinitions, Margin, RichText, TextStyle, Visuals,
    },
    epaint::{Color32, FontFamily, FontId, Rounding, Shadow, Stroke},
};

use crate::nord::Nord;

pub enum AppIcon {
    More,
    Settings,
    // pub question_mark: TextureHandle,
    Close,
    Twitter,
    HomeSite,
    Discord,
}

impl AppIcon {
    pub fn render(&self, size: f32, color: Color32) -> RichText {
        use egui_phosphor::light as P;
        RichText::new(match self {
            AppIcon::More => P::DOTS_THREE_OUTLINE,
            AppIcon::Settings => P::GEAR_FINE,
            AppIcon::Close => P::X,
            AppIcon::Twitter => P::TWITTER_LOGO,
            AppIcon::HomeSite => P::HOUSE_SIMPLE,
            AppIcon::Discord => P::DISCORD_LOGO,
        })
        .family(eframe::epaint::FontFamily::Proportional)
        .color(color)
        .size(size)
    }
}

// #[derive(Debug, Clone, Copy)]
pub struct Sizes {
    pub xs: f32,
    pub s: f32,
    pub m: f32,
    pub l: f32,
    pub xl: f32,

    // semantic
    pub header_footer: f32,
    pub toolbar_icon: f32,
}

pub struct AppTheme {
    pub fonts: FontTheme,
    pub colors: ColorTheme,
    pub sizes: Sizes,
}

impl AppTheme {
    pub fn nord() -> Self {
        Self {
            fonts: FontTheme::default(),
            colors: ColorTheme::nord(),
            sizes: Sizes::new(),
        }
    }
}

impl Default for AppTheme {
    fn default() -> Self {
        Self::nord()
    }
}

pub struct FontSizes {
    pub h1: f32,
    pub h2: f32,
    pub h3: f32,
    pub h4: f32,
    pub normal: f32,
    pub small: f32,
}

impl FontSizes {
    pub fn new() -> Self {
        Self {
            h1: 24.,
            h2: 20.,
            h3: 18.,
            h4: 16.,
            normal: 14.,
            small: 8.,
        }
    }
}

impl Sizes {
    pub fn new() -> Self {
        let xs = 4.0;
        let s = 8.0;
        let m = 12.0;
        let l = 16.0;
        let xl = 24.0;

        Self {
            xs,
            s,
            m,
            l,
            xl,
            header_footer: xl + xs,
            toolbar_icon: l + xs / 2.,
        }
    }
}
pub struct FontFamilies {
    pub normal: FontFamily,
    pub italic: FontFamily,
    pub bold: FontFamily,
    pub bold_italic: FontFamily,
    pub code: FontFamily,
}

impl FontFamilies {
    pub fn new() -> Self {
        Self {
            normal: FontFamily::Name("inter".into()),
            italic: FontFamily::Name("inter-italic".into()),
            bold: FontFamily::Name("inter-bold".into()),
            bold_italic: FontFamily::Name("inter-bold-italic".into()),
            code: FontFamily::Monospace,
        }
    }
}

pub struct FontTheme {
    pub size: FontSizes,

    // families
    pub family: FontFamilies,
}

impl Default for FontTheme {
    fn default() -> Self {
        Self {
            size: FontSizes::new(),
            family: FontFamilies::new(),
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
    pub md_link: Color32,
    pub md_code: Color32,

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
        let md_annotation: Color32 = Nord::NORD4.shade(0.5);
        let md_body = Nord::NORD4;
        let md_header = Nord::NORD6;
        // same as hyperlink_color
        let md_link = Nord::NORD7;
        let md_code = Nord::NORD13;

        let secondary_icon = Nord::NORD3.shade(1.1);

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
        let hyperlink_color = Nord::NORD7;
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
            md_code,
            subtle_text_color,
            md_link,
        }
    }
}

pub fn configure_styles(ctx: &egui::Context, theme: &AppTheme) {
    let mut style = (*ctx.style()).clone();

    style.text_styles = text_styles(&theme.fonts);
    style.visuals = visuals(&theme.colors);
    // style.spacing.window_margin = Margin::same(0.);
    ctx.set_style(style);
}

pub fn get_font_definitions() -> FontDefinitions {
    // Start with the default fonts (we will be adding to them rather than replacing them).
    let mut fonts = FontDefinitions::default();

    fonts
        .font_data
        .insert("phosphor".into(), egui_phosphor::Variant::Light.font_data());

    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .push("phosphor".to_owned());

    // Install my own font (maybe supporting non-latin characters).
    // .ttf and .otf files supported.
    fonts.font_data.insert(
        "inter".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/Inter-Light.otf")),
    );

    fonts.font_data.insert(
        "inter-italic".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/Inter-LightItalic.otf")),
    );

    fonts.font_data.insert(
        "inter-bold".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/Inter-SemiBold.otf")),
    );
    fonts.font_data.insert(
        "inter-bold-italic".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/Inter-SemiBoldItalic.otf")),
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
        .entry(egui::FontFamily::Name("inter".into()))
        .or_default()
        .push("inter".to_owned());

    fonts
        .families
        .entry(egui::FontFamily::Name("inter-bold".into()))
        .or_default()
        .push("inter-bold".to_owned());

    fonts
        .families
        .entry(egui::FontFamily::Name("inter-italic".into()))
        .or_default()
        .push("inter-italic".to_owned());

    fonts
        .families
        .entry(egui::FontFamily::Name("inter-bold-italic".into()))
        .or_default()
        .push("inter-bold-italic".to_owned());

    // Tell egui to use these fonts:
    fonts
}

fn text_styles(FontTheme { size, family }: &FontTheme) -> BTreeMap<TextStyle, FontId> {
    [
        (
            TextStyle::Heading,
            FontId::new(size.h1, family.normal.clone()),
        ),
        (
            TextStyle::Body,
            FontId::new(size.normal, family.normal.clone()),
        ),
        (
            TextStyle::Monospace,
            FontId::new(size.normal, family.code.clone()),
        ),
        (
            TextStyle::Button,
            FontId::new(size.normal, family.normal.clone()),
        ),
        (
            TextStyle::Small,
            FontId::new(size.small, family.normal.clone()),
        ),
    ]
    .into()
}

pub trait ColorManipulation {
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
        md_link: _,
        subtle_text_color: _,
        md_code: _,
    } = color_theme.clone();

    // --- window ---
    let selection = Selection {
        bg_fill: selection_bg,
        stroke: Stroke::new(1.0, selection_stroke),
    };

    let debug_color = Color32::from_rgb(255, 0, 100);

    let widgets = Widgets {
        noninteractive: WidgetVisuals {
            weak_bg_fill: Color32::from_gray(27),
            bg_fill: debug_color,
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
            bg_stroke: Stroke::new(2.0, debug_color), //Stroke::new(1.0, Color32::from_gray(60)),
            fg_stroke: Stroke::new(2.0, debug_color), // Stroke::new(1.0, Color32::from_gray(210)),
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
        text_cursor_preview: false,
        clip_rect_margin: 3.,
        button_frame: true,
        collapsing_header_frame: false,
        indent_has_left_vline: true,
        striped: false,
        slider_trailing_fill: false,
        widgets,
        text_cursor: Stroke::new(2.0, normal_text_color),
        interact_cursor: Some(egui::CursorIcon::PointingHand),
        image_loading_spinners: true,
    }
}
