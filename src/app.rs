use std::{
    cell::RefCell,
    ops::{Range, RangeBounds},
    path::Path,
    rc::Rc,
    sync::{mpsc::Receiver, Arc},
};

use eframe::{
    egui::{
        self,
        text_edit::{CCursorRange, TextEditState},
        Button, Context, Id, ImageButton, KeyboardShortcut, Layout, Modifiers, RichText, Sense,
        TextFormat, TopBottomPanel,
    },
    emath::Align,
    epaint::{
        pos2,
        text::{layout, LayoutJob},
        vec2, Color32, FontFamily, FontId, Rect, Stroke, TextureHandle, TextureId, Vec2,
    },
};

use egui_extras::RetainedImage;
use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent};

use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::{SyntaxDefinition, SyntaxSet},
    util::LinesWithEndings,
};

use pulldown_cmark::{CodeBlockKind, HeadingLevel};
use smallvec::SmallVec;

use crate::{
    picker::Picker,
    theme::{AppTheme, ColorTheme, FontTheme},
};

// let ps = SyntaxSet::load_defaults_newlines();
// let ts = ThemeSet::load_defaults();
pub struct AppState {
    markdown: String,
    saved: String,
    theme: AppTheme,
    prev_md_layout: MdLayout,
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    msg_queue: Receiver<AsyncMessage>,
    icons: AppIcons,
    selected_note: u32,
    hidden: bool,
    md_annotation_shortcuts: Vec<MdAnnotationShortcut>,
}

pub struct AppIcons {
    pub more: RetainedImage,
    pub gear: RetainedImage,
    pub question_mark: RetainedImage,
    pub close: RetainedImage,
}

#[derive(Debug)]
pub enum AsyncMessage {
    ToggleVisibility,
}

struct MdAnnotationShortcut {
    name: &'static str,
    annotation: &'static str,
    shortcut: KeyboardShortcut,
}

impl AppState {
    pub fn new(init_data: AppInitData) -> Self {
        let AppInitData {
            theme,
            msg_queue,
            icons,
        } = init_data;
        Self {
            theme,
            markdown: "# title
- adsd
- fdsf
	- [ ] fdsf
	- [x] fdsf
1. fa
2. fdsf
3. 
bo**dy**
i*tali*c


```rs
let a = Some(115);
```"
            .to_string(),
            saved: "".to_string(),
            prev_md_layout: MdLayout::new(),
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            icons,
            msg_queue,
            selected_note: 0,
            hidden: false,
            md_annotation_shortcuts: [
                (
                    "bold",
                    "**",
                    KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::B),
                ),
                (
                    "emphasize",
                    "*",
                    KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::I),
                ),
                (
                    "strike",
                    "~~",
                    KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, egui::Key::E),
                ),
            ]
            .map(|(name, annotation, shortcut)| MdAnnotationShortcut {
                name,
                annotation,
                shortcut,
            })
            .into_iter()
            .collect(),
        }
    }
}

pub struct AppInitData {
    pub theme: AppTheme,
    pub msg_queue: Receiver<AsyncMessage>,
    pub icons: AppIcons,
}

#[no_mangle]
pub fn create_app_state(data: AppInitData) -> AppState {
    AppState::new(data)
}

struct MarkdownState {
    nesting: i32,
    bold: i32,
    strike: i32,
    emphasis: i32,
    text: i32,
    heading: [i32; 6],
}

fn load_image_from_path(path: &std::path::Path) -> Result<egui::ColorImage, image::ImageError> {
    let image = image::io::Reader::open(path)?.decode()?;
    let size = [image.width() as _, image.height() as _];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_flat_samples();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        size,
        pixels.as_slice(),
    ))
}

impl MarkdownState {
    fn to_text_format(&self, theme: &AppTheme) -> TextFormat {
        let AppTheme {
            fonts: FontTheme { size, family },
            colors,
        } = theme;

        let ColorTheme {
            md_strike,
            md_annotation,
            md_body,
            md_header,
            ..
        } = colors;

        let emphasis = self.emphasis > 0;
        let bold = self.bold > 0;

        let font_size = match self.heading {
            [h1, ..] if h1 > 0 => size.h1,
            [_, h2, ..] if h2 > 0 => size.h2,
            [_, _, h3, ..] if h3 > 0 => size.h3,
            [_, _, _, h4, ..] if h4 > 0 => size.h4,
            [_, _, _, _, h5, ..] if h5 > 0 => size.h4,
            [_, _, _, _, _, h6] if h6 > 0 => size.h4,
            _ => size.normal,
        };

        let color = match (self.heading.iter().any(|h| *h > 0), self.text > 0) {
            (_, false) => md_annotation,
            (true, true) => md_header,
            (false, true) => md_body,
        };

        let font_family = match (emphasis, bold) {
            (true, true) => &family.bold_italic,
            (false, true) => &family.bold,
            (true, false) => &family.italic,
            (false, false) => &family.normal,
        };

        TextFormat {
            color: *color,
            font_id: FontId::new(font_size, font_family.clone()),
            strikethrough: if self.strike > 0 {
                Stroke::new(0.6, *md_strike)
            } else {
                Stroke::NONE
            },
            ..Default::default()
        }
    }
}

impl MarkdownState {
    fn new() -> Self {
        Self {
            nesting: 0,
            bold: 0,
            strike: 0,
            emphasis: 0,
            heading: Default::default(),
            text: 0,
        }
    }
}

#[derive(Debug)]
enum PointKind {
    Start,
    End,
}
#[derive(Debug)]
struct AnnotationPoint {
    offset: usize,
    kind: PointKind, // 1 or -1 (start and end respectively)
    annotation: Annotation,
}

#[derive(Debug, Clone)]
struct ListItem {
    index: u32,
    byte_range: Range<usize>,
    depth: i32,
    starting_index: Option<u64>,
}

struct ListDesc {
    starting_index: Option<u64>,
    items_count: u32,
}

struct MdLayout {
    list_stack: SmallVec<[ListDesc; 4]>,
    points: Vec<AnnotationPoint>,
    list_items: Vec<ListItem>, // range and depth
}

#[derive(Debug, Clone)]
enum Annotation {
    Strike,
    Bold,
    Emphasis,
    Text,
    Heading(HeadingLevel),
    Code { lang: String },
}

enum Ev {
    Annotation(Annotation),
    ListItem,
    TaskMarker(bool),
    Heading(HeadingLevel),
    ListStart(Option<u64>),
    ListEnd,
}

impl MdLayout {
    fn new() -> Self {
        Self {
            points: Default::default(),
            list_items: Default::default(),
            list_stack: Default::default(),
        }
    }

    fn event(&mut self, ev: Ev, range: Range<usize>) {
        match ev {
            Ev::Annotation(annotation) => {
                self.points.push(AnnotationPoint {
                    offset: range.start,
                    kind: PointKind::Start,
                    annotation: annotation.clone(),
                });

                self.points.push(AnnotationPoint {
                    offset: range.end,
                    kind: PointKind::End,
                    annotation,
                });
            }
            Ev::ListItem => {
                // depth starts with zero for top level list
                let depth = self.list_stack.len() as i32 - 1;
                if let Some(list_desc) = self.list_stack.last_mut() {
                    self.list_items.push(ListItem {
                        index: list_desc.items_count,
                        byte_range: range,
                        depth,
                        starting_index: list_desc.starting_index.clone(),
                    });

                    list_desc.items_count += 1;
                }
            }

            Ev::TaskMarker(checked) => {
                // the last one to add would be the most nested, thus the one we need
                let item =
                    self.list_items.iter().rev().find(|r| {
                        r.byte_range.start <= range.start && r.byte_range.end >= range.end
                    });

                if let (Some(r), true) = (item, checked) {
                    self.event(Ev::Annotation(Annotation::Strike), r.byte_range.clone());
                }
            }
            // TODO use that for shortcuts maybe
            Ev::Heading(level) => {
                self.event(Ev::Annotation(Annotation::Heading(level)), range);
            }
            Ev::ListStart(starting_index) => self.list_stack.push(ListDesc {
                starting_index,
                items_count: 0,
            }),

            Ev::ListEnd => {
                self.list_stack.pop();
            }
        }
    }

    fn layout(
        &mut self,
        text: &str,
        theme: &AppTheme,
        syntax_set: &SyntaxSet,
        theme_set: &ThemeSet,
    ) -> LayoutJob {
        let MdLayout { points, .. } = self;
        points.sort_by_key(|p| p.offset);

        let mut pos: usize = 0;
        let mut job = LayoutJob::default();

        let mut state = MarkdownState::new();

        let code_font_id = FontId {
            size: theme.fonts.size.normal,
            family: theme.fonts.family.code.clone(),
        };

        // println!("points: {:#?}", points);
        for point in points {
            if let (Annotation::Code { lang }, PointKind::End) = (&point.annotation, &point.kind) {
                let code = text.get(pos..point.offset).unwrap_or("");

                match syntax_set.find_syntax_by_extension(&lang) {
                    Some(syntax) => {
                        let mut h =
                            HighlightLines::new(syntax, &theme_set.themes["base16-ocean.dark"]);
                        // let s = "pub struct Wow { hi: u64 }\nfn blah() -> u64 {}";
                        // for line in LinesWithEndings::from(s) {
                        //     let ranges: Vec<(Style, &str)> = h.highlight_line(line, &ps).unwrap();
                        //     let escaped = as_24_bit_terminal_escaped(&ranges[..], true);
                        //     print!("{}", escaped);
                        // }

                        for line in LinesWithEndings::from(code) {
                            let ranges = h.highlight_line(line, &syntax_set).unwrap();
                            for (style, part) in ranges {
                                let front = style.foreground;

                                // println!("{:?}", (part, style.foreground));
                                job.append(
                                    part,
                                    0.0,
                                    TextFormat::simple(
                                        code_font_id.clone(),
                                        Color32::from_rgb(front.r, front.g, front.b),
                                    ),
                                );
                            }
                        }
                    }
                    None => job.append(
                        code,
                        0.0,
                        TextFormat::simple(code_font_id.clone(), theme.colors.normal_text_color),
                    ),
                }
            } else {
                job.append(
                    text.get(pos..point.offset).unwrap_or(""),
                    0.0,
                    state.to_text_format(theme),
                );

                let delta = match point.kind {
                    PointKind::Start => 1,
                    PointKind::End => -1,
                };

                match &point.annotation {
                    Annotation::Strike => state.strike += delta,
                    Annotation::Bold => state.bold += delta,
                    Annotation::Text => state.text += delta,
                    Annotation::Emphasis => state.emphasis += delta,
                    Annotation::Heading(level) => state.heading[*level as usize] += delta,
                    Annotation::Code { lang } => (),
                }
            }

            pos = point.offset;
        }

        // the last piece of text
        job.append(
            text.get(pos..).unwrap_or(""),
            0.0,
            state.to_text_format(theme),
        );

        job
    }
}

#[no_mangle]
pub fn render(state: &mut AppState, ctx: &egui::Context, frame: &mut eframe::Frame) {
    let id = Id::new("text_edit");

    while let Ok(msg) = state.msg_queue.try_recv() {
        println!("got in render: {msg:?}");
        match msg {
            AsyncMessage::ToggleVisibility => {
                state.hidden = !state.hidden;
                frame.set_visible(!state.hidden);

                if !state.hidden {
                    set_cursor_at_the_end(&state.markdown, ctx, id);
                    frame.focus_window();
                }
            }
        }
    }

    render_footer(&mut state.selected_note, ctx, &state.icons, &state.theme);
    render_header_panel(ctx, &state.icons, &state.theme);

    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

            let mut md = MdLayout::new();

            let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                let options = pulldown_cmark::Options::ENABLE_STRIKETHROUGH
                    | pulldown_cmark::Options::ENABLE_TASKLISTS
                    | pulldown_cmark::Options::ENABLE_SMART_PUNCTUATION;

                if state.saved != text {
                    let parser = pulldown_cmark::Parser::new_ext(text, options);
                    println!("-----parser-----");
                    println!("{:?}", text);
                    println!("-----text-end-----");
                    let mut depth = 0;
                    for (ev, range) in parser.into_offset_iter() {
                        if let pulldown_cmark::Event::End(_) = &ev {
                            depth -= 1;
                        }

                        println!(
                            "{}{:?} -> {:?}",
                            "  ".repeat(depth),
                            ev,
                            &text[range.start..range.end] // .lines()
                                                          // .map(|l| format!("{}{}", "  ".repeat(depth), l))
                                                          // .reduce(|mut all, l| {
                                                          //     all.extend(l.chars());
                                                          //     all
                                                          // })
                        );

                        if let pulldown_cmark::Event::Start(_) = &ev {
                            depth += 1;
                        }
                    }
                    println!("---parser-end---");
                    state.saved = text.to_string();
                }

                let parser = pulldown_cmark::Parser::new_ext(text, options);

                let mut code_block: Option<String> = None;

                for (ev, range) in parser.into_offset_iter() {
                    use pulldown_cmark::Event::*;
                    use pulldown_cmark::Tag::*;
                    match ev {
                        Start(tag) => match tag {
                            Strong => {
                                md.event(Ev::Annotation(Annotation::Bold), range);
                            }
                            Emphasis => {
                                md.event(Ev::Annotation(Annotation::Emphasis), range);
                            }
                            Strikethrough => {
                                md.event(Ev::Annotation(Annotation::Strike), range);
                            }
                            CodeBlock(CodeBlockKind::Fenced(lang)) => {
                                code_block = Some(lang.as_ref().to_string())
                            }
                            Item => md.event(Ev::ListItem, range),
                            Heading(level, _, _) => md.event(Ev::Heading(level), range),
                            List(starting_index) => md.event(Ev::ListStart(starting_index), range),
                            _ => (),
                        },

                        End(List(_)) => md.event(Ev::ListEnd, range),
                        End(CodeBlock(CodeBlockKind::Fenced(_))) => code_block = None,

                        Text(_) => {
                            if let Some(lang) = code_block.take() {
                                md.event(Ev::Annotation(Annotation::Code { lang }), range)
                            } else {
                                md.event(Ev::Annotation(Annotation::Text), range)
                            }
                        }

                        TaskListMarker(checked) => md.event(Ev::TaskMarker(checked), range),
                        _ => (),
                    }
                }

                let mut job = md.layout(text, &state.theme, &state.syntax_set, &state.theme_set);
                job.wrap.max_width = wrap_width;

                // let mut galley = layout(&mut ui.ctx().fonts().lock().fonts, job.into());
                // Arc::new(galley)
                let galley = ui.fonts(|f| f.layout_job(job));

                galley
            };

            let inside_item = TextEditState::load(ui.ctx(), id)
                .and_then(|edit_state| edit_state.ccursor_range())
                .and_then(|cursor_range| {
                    let text = &mut state.markdown;
                    use egui::TextBuffer as _;

                    let [start, end] = cursor_range.sorted();

                    let byte_start = text.byte_index_from_char_index(start.index);
                    let byte_end = text.byte_index_from_char_index(end.index);

                    let inside_item = state
                        .prev_md_layout
                        .list_items
                        .iter()
                        .rev()
                        .find(|item| {
                            item.byte_range.start <= byte_start && item.byte_range.end >= byte_end
                        })
                        .map(|r| r.clone());

                    inside_item
                });

            let output = {
                let before = state.markdown.clone();

                let res = egui::TextEdit::multiline(&mut state.markdown)
                    .font(egui::TextStyle::Monospace) // for cursor height
                    .code_editor()
                    .id(id)
                    // .desired_rows(1000)
                    .lock_focus(true)
                    .desired_width(f32::INFINITY)
                    .frame(false)
                    .layouter(&mut layouter)
                    .show(ui);

                if before != state.markdown {
                    println!("before: {}\nafter:{}\n", before, state.markdown);
                }

                res
            };

            let space_below = ui.available_rect_before_wrap();

            if space_below.height() > 0.
                && ui
                    .interact(space_below, Id::new("space_below"), Sense::click())
                    .clicked()
            {
                set_cursor_at_the_end(&state.markdown, ctx, id);
            }

            // // Test UI controls to tune in styles
            // let resp = ui.button("ClickMe");
            // let mut checked = true;
            // let resp = ui.checkbox(&mut checked, "checkbox");

            // let capture = ui.add(Button::new(
            //     RichText::new("CAPTURE").text_style(egui::TextStyle::Heading),
            // ));

            // let mut my_f32: f32 = 30.0;
            // ui.add(egui::Slider::new(&mut my_f32, 0.0..=100.0).text("My value"));

            if ui.input_mut(|input| input.key_pressed(egui::Key::Enter)) {
                if let (Some(inside_item), Some(text_cursor_range), Some(mut edit_state)) = (
                    inside_item,
                    output.cursor_range,
                    TextEditState::load(ui.ctx(), id),
                ) {
                    let text = &mut state.markdown;
                    use egui::TextBuffer as _;
                    let selected_chars = text_cursor_range.as_sorted_char_range();
                    let text_to_insert = match inside_item.starting_index {
                        Some(starting_index) => format!(
                            "{}{}. ",
                            "\t".repeat(inside_item.depth as usize),
                            starting_index + inside_item.index as u64 + 1
                        ),
                        None => format!("{}- ", "\t".repeat(inside_item.depth as usize)),
                    };
                    text.insert_text(text_to_insert.as_str(), selected_chars.start);

                    let [min, max] = text_cursor_range.as_ccursor_range().sorted();

                    println!("prev cursor: {:#?}", edit_state.ccursor_range());
                    // NOTE that cursor range works in chars, but in this case we inserted only chars that fit into u8
                    // that byte size and char size of insertion are te same in this case
                    edit_state.set_ccursor_range(Some(CCursorRange::two(
                        min + text_to_insert.len(),
                        max + text_to_insert.len(),
                    )));

                    println!("next cursor: {:#?}", edit_state.ccursor_range());

                    edit_state.store(ui.ctx(), id);
                }
            }

            for md_shortcut in state.md_annotation_shortcuts.iter() {
                if ui.input_mut(|input| input.consume_shortcut(&md_shortcut.shortcut)) {
                    if let (Some(text_cursor_range), Some(mut edit_state)) =
                        (output.cursor_range, TextEditState::load(ui.ctx(), id))
                    {
                        let text = &mut state.markdown;
                        use egui::TextBuffer as _;
                        let selected_chars = text_cursor_range.as_sorted_char_range();
                        let selected_text = text.char_range(selected_chars.clone());

                        let annotation = md_shortcut.annotation;
                        let annotation_len = annotation.chars().count();

                        let is_already_annotated = selected_text
                            .starts_with(md_shortcut.annotation)
                            && selected_text.ends_with(annotation)
                            && selected_text.chars().count() >= annotation_len * 2;

                        if is_already_annotated {
                            text.delete_char_range(Range {
                                start: selected_chars.start,
                                end: selected_chars.start + annotation_len,
                            });
                            text.delete_char_range(Range {
                                start: selected_chars.end - annotation_len * 2,
                                end: selected_chars.end - annotation_len,
                            });
                        } else {
                            text.insert_text(annotation, selected_chars.start);
                            text.insert_text(
                                md_shortcut.annotation,
                                selected_chars.end + annotation_len,
                            );
                        };

                        let [min, max] = text_cursor_range.as_ccursor_range().sorted();

                        // println!("prev cursor: {:#?}", edit_state.ccursor_range());
                        edit_state.set_ccursor_range(Some(CCursorRange::two(
                            min,
                            if is_already_annotated {
                                max - annotation_len * 2
                            } else {
                                max + annotation_len * 2
                            },
                        )));

                        // println!("next cursor: {:#?}", edit_state.ccursor_range());

                        edit_state.store(ui.ctx(), id);
                    }
                }
            }

            state.prev_md_layout = md;
        });
    });
}

fn set_cursor_at_the_end(text: &str, ctx: &Context, id: Id) {
    if let Some(mut text_edit_state) = egui::TextEdit::load_state(ctx, id) {
        let ccursor = egui::text::CCursor::new(text.chars().count());

        text_edit_state.set_ccursor_range(Some(egui::text::CCursorRange::one(ccursor)));
        text_edit_state.store(ctx, id);

        ctx.memory_mut(|mem| mem.request_focus(id));
    }
}

fn render_footer(selected: &mut u32, ctx: &Context, icons: &AppIcons, theme: &AppTheme) {
    TopBottomPanel::bottom("footer")
        // .exact_height(32.)
        .show_separator_line(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let height = 24.;
                let avail_width = ui.available_width();
                ui.set_min_size(vec2(avail_width, height));

                set_menu_bar_style(ui);

                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.add(Picker {
                        current: selected,
                        count: 4,
                        gap: 8.,
                        radius: 8.,
                        inactive: theme.colors.outline_fg,
                        hover: theme.colors.button_hover_bg_stroke,
                        pressed: theme.colors.button_pressed_fg,
                        selected_stroke: theme.colors.button_fg,
                        selected_fill: theme.colors.button_bg,
                        outline: Stroke::new(1.0, theme.colors.outline_fg),
                    });
                    // let capture = ui.add(
                    //     Button::image_and_text(
                    //         icons.more.texture_id(ctx),
                    //         Vec2::new(18., 18.),
                    //         RichText::new("Note 1").text_style(egui::TextStyle::Button),
                    //     )
                    //     .min_size(vec2(24., 24.)),
                    // );

                    // ui.add_space(4.);
                    // let record = ui.add(
                    //     Button::image_and_text(
                    //         icons.more.texture_id(ctx),
                    //         Vec2::new(18., 18.),
                    //         RichText::new("Note 2").text_style(egui::TextStyle::Button),
                    //     )
                    //     .min_size(vec2(24., 24.)),
                    // );
                });

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let settings = ui.add(ImageButton::new(
                        icons.gear.texture_id(ctx),
                        Vec2::new(18., 18.),
                    ));
                    // // ui.add_space(4.);
                    // ui.separator();
                });
            });
        });
}

fn set_menu_bar_style(ui: &mut egui::Ui) {
    let style = ui.style_mut();
    style.spacing.button_padding = vec2(0.0, 0.0);
    style.spacing.item_spacing = vec2(0.0, 0.0);
    style.visuals.widgets.active.bg_stroke = Stroke::NONE;
    style.visuals.widgets.hovered.bg_stroke = Stroke::NONE;
    style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
}

fn render_header_panel(ctx: &egui::Context, icons: &AppIcons, theme: &AppTheme) {
    TopBottomPanel::top("top_panel")
        .show_separator_line(false)
        .show(ctx, |ui| {
            // println!("-----");
            // println!("before menu {:?}", ui.available_size());
            ui.horizontal(|ui| {
                let height = 24.;
                let avail_width = ui.available_width();
                let avail_rect = ui.available_rect_before_wrap();
                ui.painter().line_segment(
                    [avail_rect.left(), avail_rect.right()]
                        .map(|x| pos2(x, avail_rect.top() + height)),
                    Stroke::new(1.0, theme.colors.outline_fg),
                );
                ui.set_min_size(vec2(avail_width, height));
                let icon_block_width = 48.;

                set_menu_bar_style(ui);

                // println!("before x {:?}", ui.available_size());

                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.set_width(icon_block_width);
                    let close_btn = ui.add(ImageButton::new(
                        icons.close.texture_id(ctx),
                        Vec2::new(18., 18.),
                    ));

                    if close_btn.clicked() {}
                });

                // println!("before title {:?}", ui.available_size());

                ui.scope(|ui| {
                    ui.set_width(avail_width - 2. * icon_block_width);
                    ui.with_layout(
                        Layout::centered_and_justified(egui::Direction::LeftToRight),
                        |ui| {
                            ui.label(
                                RichText::new("Shelv")
                                    .color(theme.colors.subtle_text_color)
                                    .font(FontId {
                                        size: theme.fonts.size.normal,
                                        family: theme.fonts.family.bold.clone(),
                                    }),
                            );
                        },
                    );
                });

                // println!("before help {:?}", ui.available_size());

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.set_width(icon_block_width);

                    let help = ui.add(ImageButton::new(
                        icons.question_mark.texture_id(ctx),
                        Vec2::new(18., 18.),
                    ));

                    if help.clicked() {}
                });
            });
        });
}
