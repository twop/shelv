use std::{
    ops::Range,
    sync::{mpsc::Receiver, Arc},
};

use eframe::{
    egui::{
        self,
        text_edit::{CCursorRange, TextEditOutput},
        Context, Id, ImageButton, KeyboardShortcut, Layout, Modifiers, RichText, Sense,
        TopBottomPanel, Ui,
    },
    emath::Align,
    epaint::{pos2, vec2, Color32, FontId, Galley, Stroke, Vec2},
};

use egui_extras::RetainedImage;

use linkify::LinkFinder;
use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};

use pulldown_cmark::CodeBlockKind;

use crate::{
    picker::Picker,
    text_structure::{Ev, InteractiveTextPart, TextStructure},
    theme::AppTheme,
};

// let ps = SyntaxSet::load_defaults_newlines();
// let ts = ThemeSet::load_defaults();
pub struct AppState {
    note: String,
    selected_note: u32,

    theme: AppTheme,
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    msg_queue: Receiver<AsyncMessage>,
    icons: AppIcons,
    hidden: bool,
    md_annotation_shortcuts: Vec<MdAnnotationShortcut>,
    app_shortcuts: AppShortcuts,

    computed_layout: Option<ComputedLayout>,
}

struct AppShortcuts {
    bold: KeyboardShortcut,
    emphasize: KeyboardShortcut,
    strikethrough: KeyboardShortcut,
    code_block: KeyboardShortcut,
}

struct ComputedLayout {
    galley: Arc<Galley>,
    wrap_width: f32,
    text_structure: TextStructure,
    computed_for: String, // maybe use hash not to double store the string content?
}

impl ComputedLayout {
    fn should_recompute(&self, text: &str, max_width: f32) -> bool {
        self.wrap_width != max_width || self.computed_for != text
    }

    fn compute(
        text: &str,
        wrap_width: f32,
        ui: &Ui,
        theme: &AppTheme,
        syntax_set: &SyntaxSet,
        theme_set: &ThemeSet,
    ) -> Self {
        let mut text_structure = TextStructure::new();

        let finder = LinkFinder::new();

        for link in finder.links(text) {
            text_structure.event(
                Ev::RawLink {
                    url: link.as_str().to_string(),
                },
                link.start()..link.end(),
            );
        }

        let md_parser_options = pulldown_cmark::Options::ENABLE_STRIKETHROUGH
            | pulldown_cmark::Options::ENABLE_TASKLISTS
            | pulldown_cmark::Options::ENABLE_SMART_PUNCTUATION;

        let parser = pulldown_cmark::Parser::new_ext(&text, md_parser_options);

        let mut code_block: Option<(String, Range<usize>)> = None;

        for (ev, range) in parser.into_offset_iter() {
            use pulldown_cmark::Event::*;
            use pulldown_cmark::Tag::*;
            match ev {
                Start(tag) => match tag {
                    Strong => text_structure.event(Ev::Bold, range),
                    Emphasis => text_structure.event(Ev::Emphasis, range),
                    Strikethrough => text_structure.event(Ev::Strike, range),
                    CodeBlock(CodeBlockKind::Fenced(lang)) => {
                        code_block = Some((lang.as_ref().to_string(), range))
                    }
                    Item => text_structure.event(Ev::ListItem, range),
                    Heading(level, _, _) => text_structure.event(Ev::Heading(level), range),
                    List(starting_index) => {
                        text_structure.event(Ev::ListStart(starting_index), range)
                    }
                    _ => (),
                },

                End(List(_)) => text_structure.event(Ev::ListEnd, range),
                End(CodeBlock(CodeBlockKind::Fenced(_))) => code_block = None,

                Text(_) => {
                    if let Some((lang, code_block_pos)) = code_block.take() {
                        text_structure.event(
                            Ev::CodeBody {
                                lang,
                                code_block_pos,
                            },
                            range,
                        )
                    } else {
                        text_structure.event(Ev::Text, range)
                    }
                }

                TaskListMarker(checked) => {
                    text_structure.event(Ev::TaskMarker(checked), range.clone());
                }
                _ => (),
            }
        }

        let mut job = text_structure.create_layout_job(text, theme, syntax_set, theme_set);

        job.wrap.max_width = wrap_width;

        let galley = ui.fonts(|f| f.layout_job(job));

        //     let parser = pulldown_cmark::Parser::new_ext(text, md_parser_options);
        //     println!("-----parser-----");
        //     println!("{:?}", text);
        //     println!("-----text-end-----");
        //     let mut depth = 0;
        //     for (ev, range) in parser.into_offset_iter() {
        //         if let pulldown_cmark::Event::End(_) = &ev {
        //             depth -= 1;
        //         }

        //         println!(
        //             "{}{:?} -> {:?}",
        //             "  ".repeat(depth),
        //             ev,
        //             &text[range.start..range.end] // .lines()
        //                                           // .map(|l| format!("{}{}", "  ".repeat(depth), l))
        //                                           // .reduce(|mut all, l| {
        //                                           //     all.extend(l.chars());
        //                                           //     all
        //                                           // })
        //         );

        //         if let pulldown_cmark::Event::Start(_) = &ev {
        //             depth += 1;
        //         }
        //     }
        //     println!("---parser-end---");

        Self {
            galley,
            wrap_width,
            text_structure,
            computed_for: text.to_string(),
        }
    }
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

        let app_shortcuts = AppShortcuts {
            bold: KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::B),
            emphasize: KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::I),
            strikethrough: KeyboardShortcut::new(
                Modifiers::COMMAND | Modifiers::SHIFT,
                egui::Key::E,
            ),
            code_block: KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::CTRL, egui::Key::C),
        };
        Self {
            theme,
            note: "# title
- adsd
- fdsf
	- [ ] fdsf
	- [x] fdsf
1. fa
2. fdsf
3. 
bo**dy**
i*tali*c

https://www.nordtheme.com/docs/colors-and-palettes

```rs
let a = Some(115);
```"
            .to_string(),
            computed_layout: None,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            icons,
            msg_queue,
            selected_note: 0,
            hidden: false,
            md_annotation_shortcuts: [
                ("bold", "**", app_shortcuts.bold),
                ("emphasize", "*", app_shortcuts.emphasize),
                ("strike", "~~", app_shortcuts.strikethrough),
            ]
            .map(|(name, annotation, shortcut)| MdAnnotationShortcut {
                name,
                annotation,
                shortcut,
            })
            .into_iter()
            .collect(),
            app_shortcuts,
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
                    set_cursor_at_the_end(&state.note, ctx, id);
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

            let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                let computed_layout = match state.computed_layout.take() {
                    Some(layout) if !layout.should_recompute(text, wrap_width) => layout,

                    // TODO reuse the prev computed layout
                    _ => ComputedLayout::compute(
                        text,
                        wrap_width,
                        ui,
                        &state.theme,
                        &state.syntax_set,
                        &state.theme_set,
                    ),
                };

                let res = computed_layout.galley.clone();
                state.computed_layout = Some(computed_layout);

                res
            };

            // let mut edited_text = state.markdown.clone();

            let TextEditOutput {
                response: _,
                galley,
                text_draw_pos,
                text_clip_rect: _,
                state: mut text_edit_state,
                mut cursor_range,
            } = egui::TextEdit::multiline(&mut state.note)
                .font(egui::TextStyle::Monospace) // for cursor height
                .code_editor()
                .id(id)
                .lock_focus(true)
                .desired_width(f32::INFINITY)
                .frame(false)
                .layouter(&mut layouter)
                .show(ui);

            let space_below = ui.available_rect_before_wrap();

            // ---- CLICKING ON EMPTY AREA FOCUSES ON TEXT EDIT ----
            if space_below.height() > 0.
                && ui
                    .interact(space_below, Id::new("space_below"), Sense::click())
                    .clicked()
            {
                set_cursor_at_the_end(&state.note, ctx, id);
            }

            // ---- SHORTCUTS FOR MAKING BOLD/ITALIC/STRIKETHROUGH ----
            let md_annotation_shortcuts = &mut state.md_annotation_shortcuts;
            for md_shortcut in md_annotation_shortcuts.iter() {
                if ui.input_mut(|input| input.consume_shortcut(&md_shortcut.shortcut)) {
                    if let Some(text_cursor_range) = cursor_range {
                        let text = &mut state.note;
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
                        text_edit_state.set_ccursor_range(Some(CCursorRange::two(
                            min,
                            if is_already_annotated {
                                max - annotation_len * 2
                            } else {
                                max + annotation_len * 2
                            },
                        )));

                        cursor_range = text_edit_state.cursor_range(&galley);

                        // println!("next cursor: {:#?}", edit_state.ccursor_range());
                    }
                }
            }

            // ---- INTERACTIVE TEXT PARTS (TODO + LINKS) ----
            if let (Some(pointer_pos), Some(computed_layout)) =
                (ui.ctx().pointer_interact_pos(), &state.computed_layout)
            {
                let cursor = galley.cursor_from_pos(pointer_pos - text_draw_pos);
                use egui::TextBuffer;

                let byte_cursor = galley
                    .text()
                    .byte_index_from_char_index(cursor.ccursor.index);

                if let Some(interactive) = computed_layout
                    .text_structure
                    .find_interactive_text_part(byte_cursor)
                {
                    if ui.input(|i| i.modifiers.command) {
                        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                        if ui.input(|i| i.pointer.primary_clicked()) {
                            match interactive {
                                InteractiveTextPart::TaskMarker {
                                    byte_range,
                                    checked,
                                } => {
                                    state.note.replace_range(
                                        byte_range,
                                        if checked { "[ ]" } else { "[x]" },
                                    );
                                }
                                InteractiveTextPart::Link(url) => {
                                    println!("open url {url:}");
                                    ctx.output_mut(|output| output.open_url(url));
                                }
                            }
                        }
                    }
                }
            }

            // ---- AUTO INDENT LISTS ----
            if ui.input_mut(|input| input.key_pressed(egui::Key::Enter)) {
                if let (Some(text_cursor_range), Some(computed_layout)) =
                    (cursor_range, &state.computed_layout)
                {
                    let inside_item = {
                        let text = &mut state.note;
                        use egui::TextBuffer;

                        let [start, end] = text_cursor_range.as_ccursor_range().sorted();

                        let byte_start = text.byte_index_from_char_index(start.index);
                        let byte_end = text.byte_index_from_char_index(end.index);

                        computed_layout
                            .text_structure
                            .find_surrounding_list_item(byte_start..byte_end)
                    };

                    if let Some(inside_list_item) = inside_item {
                        let text = &mut state.note;
                        use egui::TextBuffer as _;
                        let selected_chars = text_cursor_range.as_sorted_char_range();
                        let text_to_insert = match inside_list_item.starting_index {
                            Some(starting_index) => format!(
                                "{}{}. ",
                                "\t".repeat(inside_list_item.depth as usize),
                                starting_index + inside_list_item.item_index as u64 + 1
                            ),
                            None => {
                                format!("{}- ", "\t".repeat(inside_list_item.depth as usize))
                            }
                        };
                        text.insert_text(text_to_insert.as_str(), selected_chars.start);

                        let [min, max] = text_cursor_range.as_ccursor_range().sorted();

                        println!("prev cursor: {:#?}", text_edit_state.ccursor_range());
                        // NOTE that cursor range works in chars, but in this case we inserted only chars that fit into u8
                        // that byte size and char size of insertion are te same in this case
                        text_edit_state.set_ccursor_range(Some(CCursorRange::two(
                            min + text_to_insert.len(),
                            max + text_to_insert.len(),
                        )));

                        println!("next cursor: {:#?}", text_edit_state.ccursor_range());
                    }
                }
            }

            text_edit_state.store(ui.ctx(), id);
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
