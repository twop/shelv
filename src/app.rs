use std::{
    fmt::format,
    ops::Range,
    sync::{mpsc::Receiver, Arc},
};

use eframe::{
    egui::{
        self,
        text::CCursor,
        text_edit::{CCursorRange, TextEditOutput, TextEditState},
        Button, Context, Id, ImageButton, Key, KeyboardShortcut, Layout, Modifiers, Painter,
        RichText, Sense, TopBottomPanel, Ui,
    },
    emath::{Align, Align2},
    epaint::{pos2, vec2, Color32, FontId, Galley, Rect, Stroke, Vec2},
};

use egui_extras::RetainedImage;

use linkify::LinkFinder;
use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};

use pulldown_cmark::{CodeBlockKind, HeadingLevel};

use crate::{
    md_shortcut::{
        execute_instruction, Edge, Instruction, InstructionCondition, MdAnnotationShortcut,
        ShortcutContext, Source,
    },
    persistent_state::PersistentState,
    picker::{Picker, PickerItem},
    text_structure::{
        self, Ev, InteractiveTextPart, SpanKind, TextStructure, TextStructureBuilder,
    },
    theme::AppTheme,
};

pub struct Note {
    text: String,
    shortcut: KeyboardShortcut,
}

pub struct AppState {
    notes: Vec<Note>,
    selected_note: u32,
    save_to_storage: bool,

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
    h1: KeyboardShortcut,
    h2: KeyboardShortcut,
    h3: KeyboardShortcut,
    // h4: KeyboardShortcut,
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
        let text_structure = TextStructure::create_from(text);

        let mut job = text_structure.create_layout_job(text, theme, syntax_set, theme_set);

        job.wrap.max_width = wrap_width;

        let galley = ui.fonts(|f| f.layout_job(job));

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

// struct MdAnnotationShortcut {
//     name: &'static str,
//     annotation: &'static str,
//     shortcut: KeyboardShortcut,
// }

impl AppState {
    pub fn new(init_data: AppInitData) -> Self {
        let AppInitData {
            theme,
            msg_queue,
            icons,
            persistent_state,
        } = init_data;

        use Instruction::*;
        use InstructionCondition::*;

        let app_shortcuts = AppShortcuts {
            bold: KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::B),
            emphasize: KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::I),
            strikethrough: KeyboardShortcut::new(
                Modifiers::COMMAND | Modifiers::SHIFT,
                egui::Key::E,
            ),
            code_block: KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, egui::Key::C),
            h1: KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, egui::Key::Num1),
            h2: KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, egui::Key::Num2),
            h3: KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, egui::Key::Num3),
            // h4: KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, egui::Key::Num4),
        };

        let notes_count = 4;

        let (selected_note, mut notes) = persistent_state
            .map(|s| (s.selected_note, s.notes))
            .unwrap_or_else(|| (0, (0..notes_count).map(|i| format!("{i}")).collect()));

        let restored_notes_count = notes.len();
        if restored_notes_count != notes_count {
            for i in restored_notes_count..notes_count {
                notes.push(format!("{i}"));
            }
        }

        let notes: Vec<Note> = notes
            .into_iter()
            .enumerate()
            .map(|(index, text)| {
                let key = number_to_key(index as u8 + 1).unwrap();
                Note {
                    text,
                    shortcut: KeyboardShortcut::new(Modifiers::COMMAND, key),
                }
            })
            .collect();

        //             note: "# title
        // - adsd
        // - fdsf
        // 	- [ ] fdsf
        // 	- [x] fdsf
        // 1. fa
        // 2. fdsf
        // 3.
        // bo**dy**
        // i*tali*c
        // https://www.nordtheme.com/docs/colors-and-palettes
        // ```rs
        // let a = Some(115);
        // ```"
        // .to_string(),

        Self {
            save_to_storage: false,
            theme,
            notes,
            computed_layout: None,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            icons,
            msg_queue,
            selected_note,
            hidden: false,
            md_annotation_shortcuts: [
                ("Bold", "**", app_shortcuts.bold, SpanKind::Bold),
                ("Italic", "*", app_shortcuts.emphasize, SpanKind::Emphasis),
                (
                    "Strikethrough",
                    "~~",
                    app_shortcuts.strikethrough,
                    SpanKind::Strike,
                ),
            ]
            .map(
                |(name, annotation, shortcut, target_span)| MdAnnotationShortcut {
                    name,
                    shortcut,
                    instruction: Condition {
                        cond: IsNoneOrEmpty(Source::Selection),
                        if_true: Box::new(Seq(vec![
                            Insert(annotation),
                            PlaceCursor(Edge::Start),
                            PlaceCursor(Edge::End),
                            Insert(annotation),
                        ])),
                        if_false: Box::new(Seq(vec![
                            PlaceCursor(Edge::Start),
                            Insert(annotation),
                            CopyFrom(Source::Selection),
                            Insert(annotation),
                            PlaceCursor(Edge::End),
                        ])),
                    },
                    target_span,
                },
            )
            .into_iter()
            .chain(std::iter::once(MdAnnotationShortcut {
                name: "Code Block",
                shortcut: app_shortcuts.code_block,
                instruction: Instruction::sequence([
                    Instruction::condition(
                        // add new line prior if we start in the middle of the text
                        Any(vec![
                            IsNoneOrEmpty(Source::BeforeSelection),
                            EndsWith(Source::BeforeSelection, "\n"),
                        ]),
                        Insert(""),
                        Insert("\n"),
                    ),
                    Insert("```"),
                    PlaceCursor(Edge::Start),
                    PlaceCursor(Edge::End),
                    Insert("\n"),
                    Instruction::condition(
                        IsNoneOrEmpty(Source::Selection),
                        Insert(""),
                        CopyFrom(Source::Selection),
                    ),
                    Instruction::condition(
                        Any(vec![
                            IsNoneOrEmpty(Source::Selection),
                            EndsWith(Source::Selection, "\n"),
                        ]),
                        Insert(""),
                        Insert("\n"),
                    ),
                    Insert("```"),
                    Instruction::condition(
                        Any(vec![
                            IsNoneOrEmpty(Source::AfterSelection),
                            StartsWith(Source::AfterSelection, "\n"),
                        ]),
                        Insert(""),
                        Insert("\n"),
                    ),
                ]),
                target_span: SpanKind::CodeBlock,
            }))
            .chain(
                [
                    ("H1", "#", HeadingLevel::H1, app_shortcuts.h1),
                    ("H2", "##", HeadingLevel::H2, app_shortcuts.h2),
                    ("H3", "###", HeadingLevel::H3, app_shortcuts.h3),
                    // ("H4", "####", HeadingLevel::H4, app_shortcuts.h4),
                ]
                .map(|(name, prefix, level, shortcut)| MdAnnotationShortcut {
                    name,
                    shortcut,
                    instruction: MatchFirst(
                        [
                            SpanKind::Heading(HeadingLevel::H1),
                            SpanKind::Heading(HeadingLevel::H2),
                            SpanKind::Heading(HeadingLevel::H3),
                            SpanKind::Heading(HeadingLevel::H4),
                            SpanKind::Heading(HeadingLevel::H5),
                            SpanKind::Heading(HeadingLevel::H6),
                            SpanKind::Paragraph,
                        ]
                        .into_iter()
                        .filter(|kind| *kind != SpanKind::Heading(level))
                        .map(|kind| {
                            (
                                InstructionCondition::IsInside(kind),
                                Seq([
                                    SetReplaceArea(kind),
                                    Insert(prefix),
                                    Insert(" "),
                                    PlaceCursor(Edge::Start),
                                    CopyFrom(Source::SurroundingSpanContent(kind)),
                                    PlaceCursor(Edge::End),
                                ]
                                .into()),
                            )
                        })
                        .chain(
                            [(
                                InstructionCondition::IsInsideUnmarkedArea,
                                Seq([
                                    Insert(prefix),
                                    Insert(" "),
                                    PlaceCursor(Edge::Start),
                                    PlaceCursor(Edge::End),
                                ]
                                .into()),
                            )]
                            .into_iter(),
                        )
                        .collect(),
                    ),
                    target_span: SpanKind::Heading(level),
                }),
            )
            .collect(),
            app_shortcuts,
        }
    }

    pub fn should_persist(&mut self) -> Option<PersistentState> {
        if self.save_to_storage {
            self.save_to_storage = false;
            Some(PersistentState {
                notes: self.notes.iter().map(|n| n.text.clone()).collect(),
                selected_note: self.selected_note,
            })
        } else {
            None
        }
    }
}

pub struct AppInitData {
    pub theme: AppTheme,
    pub msg_queue: Receiver<AsyncMessage>,
    pub icons: AppIcons,
    pub persistent_state: Option<PersistentState>,
}

#[derive(Debug)]
struct ShortcutExecContext<'a> {
    structure: &'a TextStructure,
    text: &'a str,
    selection_byte_range: Range<usize>,
    replace_range: Range<usize>,
}

impl<'a> ShortcutContext<'a> for ShortcutExecContext<'a> {
    fn get_source(&self, source: Source) -> Option<&'a str> {
        match source {
            Source::Selection => {
                if self.selection_byte_range.is_empty() {
                    None
                } else {
                    self.text.get(self.selection_byte_range.clone())
                }
            }
            Source::BeforeSelection => self.text.get(0..self.selection_byte_range.start),
            Source::AfterSelection => self.text.get(self.selection_byte_range.end..),
            Source::SurroundingSpanContent(kind) => self
                .structure
                .find_span_at(kind, self.selection_byte_range.clone())
                .map(|(_, index)| self.structure.get_span_inner_content(index))
                .and_then(|content| self.text.get(content.clone())),
        }
    }

    fn is_inside_span(&self, kind: SpanKind) -> bool {
        self.structure
            .find_span_at(kind, self.selection_byte_range.clone())
            .is_some()
    }

    fn set_replace_area(&mut self, kind: SpanKind) {
        if let Some((range, _)) = self
            .structure
            .find_span_at(kind, self.selection_byte_range.clone())
        {
            self.replace_range = range;
        }
    }

    fn is_inside_unmarked(&self) -> bool {
        self.structure
            .find_any_span_at(self.selection_byte_range.clone())
            .is_none()
    }
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

pub fn render(state: &mut AppState, ctx: &egui::Context, frame: &mut eframe::Frame) {
    let id = Id::new(("text_edit", state.selected_note));

    while let Ok(msg) = state.msg_queue.try_recv() {
        println!("got in render: {msg:?}");
        match msg {
            AsyncMessage::ToggleVisibility => {
                state.hidden = !state.hidden;
                frame.set_visible(!state.hidden);

                if !state.hidden {
                    set_cursor_at_the_end(&state.notes[state.selected_note as usize].text, ctx, id);
                    frame.focus_window();
                }
            }
        }
    }

    let prev_selected_note = state.selected_note;
    render_footer(
        &mut state.selected_note,
        &state.notes,
        ctx,
        &state.icons,
        &state.theme,
    );

    ctx.input_mut(|input| {
        for (index, shortcut) in state.notes.iter().map(|n| n.shortcut).enumerate() {
            if input.consume_shortcut(&shortcut) {
                state.selected_note = index as u32;
            }
        }
    });

    if prev_selected_note != state.selected_note {
        state.save_to_storage = true;
        // TODO invalidate layout and set cursor to the end
    }

    let current_note = &mut state.notes[state.selected_note as usize].text;

    render_header_panel(ctx, &state.icons, &state.theme);

    egui::CentralPanel::default().show(ctx, |ui| {
        let avail_space = ui.available_rect_before_wrap();

        render_hints(
            &format!("{}", state.selected_note + 1),
            current_note
                .is_empty()
                .then(|| state.md_annotation_shortcuts.as_slice()),
            avail_space,
            ui.painter(),
            ctx,
            &state.theme,
        );

        // ---- TAB in LISTS ----
        // Note that it happens before rendering the panel
        if ui.input_mut(|input| input.modifiers.is_none() && input.key_pressed(egui::Key::Tab)) {
            if let (Some(mut text_edit_state), Some(computed_layout)) =
                (TextEditState::load(ctx, id), &state.computed_layout)
            {
                use egui::TextBuffer;

                if let Some(ccursor_range) = text_edit_state.ccursor_range() {
                    let [mut ccursor_start, mut ccursor_end] = ccursor_range.sorted();
                    let byte_start = current_note.byte_index_from_char_index(ccursor_start.index);
                    let byte_end = current_note.byte_index_from_char_index(ccursor_start.index);

                    if let Some(inside_list_item) = computed_layout
                        .text_structure
                        .find_surrounding_list_item(byte_start..byte_end)
                    {
                        let chars_before = current_note[0..inside_list_item.item_byte_pos.start]
                            .chars()
                            .count();

                        let cursor_before_list_item = computed_layout
                            .galley
                            .from_ccursor(CCursor::new(chars_before));

                        // only apply logic if the item is located on the same line as the cursor
                        match text_edit_state.cursor_range(&computed_layout.galley) {
                            Some(range)
                                if range.sorted().primary.rcursor.row
                                    == cursor_before_list_item.rcursor.row =>
                            {
                                let insertion = "\t";
                                let list_item_pos = inside_list_item.item_byte_pos.clone();

                                // TODO: normalize working with numbered lists
                                // if inside_list_item.is_numbered() {
                                //     let (bytes, chars): (usize, usize) = current_note
                                //         [list_item_pos.clone()]
                                //     .chars()
                                //     .take_while(|c| *c != '.')
                                //     .fold((0, 0), |(bytes, chars), c| {
                                //         (bytes + c.len_utf8(), chars + 1)
                                //     });

                                //     current_note.replace_range(
                                //         list_item_pos.start..list_item_pos.start + bytes,
                                //         "1",
                                //     );

                                //     // todo adjust cursor and selection
                                // }

                                let inserted_chars_count = insertion.chars().count();
                                current_note.insert_str(list_item_pos.start, insertion);

                                text_edit_state.set_ccursor_range(Some(CCursorRange::two(
                                    ccursor_start + inserted_chars_count,
                                    ccursor_end + inserted_chars_count,
                                )));

                                text_edit_state.store(ctx, id);

                                // Prevent TAB from modifying the text state
                                ui.input_mut(|input| {
                                    input.consume_key(Modifiers::NONE, egui::Key::Tab)
                                });
                            }
                            _ => (),
                        }
                    }
                }
            }
        }

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
                response: text_edit_response,
                galley,
                text_draw_pos,
                text_clip_rect: _,
                state: mut text_edit_state,
                mut cursor_range,
            } = egui::TextEdit::multiline(current_note)
                .font(egui::TextStyle::Monospace) // for cursor height
                .code_editor()
                .id(id)
                .lock_focus(true)
                .desired_width(f32::INFINITY)
                .frame(false)
                .layouter(&mut layouter)
                .show(ui);

            if text_edit_response.changed() {
                state.save_to_storage = true;
            }

            let space_below = ui.available_rect_before_wrap();

            // ---- CLICKING ON EMPTY AREA FOCUSES ON TEXT EDIT ----
            if space_below.height() > 0.
                && ui
                    .interact(space_below, Id::new("space_below"), Sense::click())
                    .clicked()
            {
                set_cursor_at_the_end(current_note, ctx, id);
            }

            // ---- SHORTCUTS FOR MAKING BOLD/ITALIC/STRIKETHROUGH ----
            if let (Some(text_cursor_range), Some(computed_layout)) =
                (cursor_range, &state.computed_layout)
            {
                for md_shortcut in state.md_annotation_shortcuts.iter() {
                    if ui.input_mut(|input| input.consume_shortcut(&md_shortcut.shortcut)) {
                        use egui::TextBuffer as _;

                        let selected_char_range = text_cursor_range.as_sorted_char_range();

                        let byte_start =
                            current_note.byte_index_from_char_index(selected_char_range.start);

                        let byte_end =
                            current_note.byte_index_from_char_index(selected_char_range.end);

                        let span = computed_layout
                            .text_structure
                            .find_span_at(md_shortcut.target_span, byte_start..byte_end)
                            .map(|(span_range, idx)| {
                                (
                                    span_range,
                                    computed_layout.text_structure.get_span_inner_content(idx),
                                )
                            });

                        let [cursor_start, cursor_end] = match span {
                            Some((span_byte_range, content_byte_range)) => {
                                // we need to remove the annotations because it is already annotated
                                // for example: if it is already "bold" then remove "**" on each side

                                match (
                                    current_note.get(content_byte_range).map(|s| s.to_string()),
                                    current_note
                                        .get(0..span_byte_range.start)
                                        .map(|s| s.chars().count()),
                                ) {
                                    (Some(inner_content), Some(span_char_offset)) => {
                                        current_note.replace_range(span_byte_range, &inner_content);

                                        let cursor_start = CCursor::new(span_char_offset);

                                        [cursor_start, cursor_start + inner_content.chars().count()]
                                    }
                                    _ => text_cursor_range.as_ccursor_range().sorted(),
                                }
                            }
                            None => {
                                // means that we need to execute instruction for the shortcut, presumably to add annotations
                                let mut cx = ShortcutExecContext {
                                    structure: &computed_layout.text_structure,
                                    text: current_note,
                                    selection_byte_range: byte_start..byte_end,
                                    replace_range: byte_start..byte_end,
                                };

                                // println!("!! md shortcut:\n{:#?}", cx);
                                match execute_instruction(&mut cx, &md_shortcut.instruction) {
                                    Some(result) => {
                                        let cursor_start = CCursor::new(
                                            current_note[..cx.replace_range.start].chars().count(),
                                        );

                                        current_note.replace_range(
                                            cx.replace_range.clone(),
                                            &result.content,
                                        );

                                        [
                                            cursor_start + result.relative_char_cursor.start,
                                            cursor_start + result.relative_char_cursor.end,
                                        ]
                                    }
                                    None => text_cursor_range.as_ccursor_range().sorted(),
                                }
                            }
                        };

                        text_edit_state
                            .set_ccursor_range(Some(CCursorRange::two(cursor_start, cursor_end)));

                        cursor_range = text_edit_state.cursor_range(&galley);
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
                                    current_note.replace_range(
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
                    use egui::TextBuffer;

                    let char_range = text_cursor_range.as_sorted_char_range();
                    let byte_start = current_note.byte_index_from_char_index(char_range.start);
                    let byte_end = current_note.byte_index_from_char_index(char_range.end);

                    let inside_item = computed_layout.text_structure.find_surrounding_list_item(
                        // note that "\n" was already inserted,
                        //thus we need to just look for "cursor_start -1" to detect a list item
                        if current_note[..byte_start].ends_with("\n") {
                            (byte_start - 1)..(byte_start - 1)
                        } else {
                            byte_start..byte_end
                        },
                    );

                    println!(
                        "\nnewline\nbefore_cursor='{}'\ncursor='{}'\nafter='{}'",
                        &current_note[0..byte_start],
                        &current_note[byte_start..byte_end],
                        &current_note[byte_end..]
                    );

                    if let Some(inside_list_item) = inside_item {
                        use egui::TextBuffer as _;

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

                        current_note.insert_text(text_to_insert.as_str(), char_range.start);

                        let [min, max] = text_cursor_range.as_ccursor_range().sorted();

                        // that byte size and char size of insertion are te same in this case
                        text_edit_state.set_ccursor_range(Some(CCursorRange::two(
                            min + text_to_insert.len(),
                            max + text_to_insert.len(),
                        )));
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

fn render_footer(
    selected: &mut u32,
    notes: &[Note],
    ctx: &Context,
    icons: &AppIcons,
    theme: &AppTheme,
) {
    TopBottomPanel::bottom("footer")
        // .exact_height(32.)
        .show_separator_line(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let sizes = &theme.sizes;
                let avail_width = ui.available_width();
                ui.set_min_size(vec2(avail_width, sizes.header_footer));

                set_menu_bar_style(ui);

                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.add(Picker {
                        current: selected,
                        items: notes
                            .iter()
                            .map(|n| PickerItem {
                                tooltip: format!("Shelf {}", ctx.format_shortcut(&n.shortcut)),
                            })
                            .collect::<Vec<_>>()
                            .as_slice(),
                        gap: sizes.s,
                        radius: sizes.s,
                        inactive: theme.colors.outline_fg,
                        hover: theme.colors.button_hover_bg_stroke,
                        pressed: theme.colors.button_pressed_fg,
                        selected_stroke: theme.colors.button_fg,
                        selected_fill: theme.colors.button_bg,
                        outline: Stroke::new(1.0, theme.colors.outline_fg),
                    });
                });

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // if ui.add(button).on_hover_ui(tooltip_ui).clicked() {
                    //     ui.output_mut(|o| o.copied_text = chr.to_string());
                    // }
                    let settings = ui.add(ImageButton::new(
                        icons.gear.texture_id(ctx),
                        Vec2::new(sizes.toolbar_icon, sizes.toolbar_icon),
                    ));
                    // .on_hover_ui(tooltip_ui);
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
                let sizes = &theme.sizes;

                let avail_width = ui.available_width();
                let avail_rect = ui.available_rect_before_wrap();
                ui.painter().line_segment(
                    [avail_rect.left(), avail_rect.right()]
                        .map(|x| pos2(x, avail_rect.top() + sizes.header_footer)),
                    Stroke::new(1.0, theme.colors.outline_fg),
                );
                ui.set_min_size(vec2(avail_width, sizes.header_footer));
                let icon_block_width = sizes.xl * 2.;

                set_menu_bar_style(ui);

                // println!("before x {:?}", ui.available_size());

                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.set_width(icon_block_width);
                    let close_btn = ui.add(ImageButton::new(
                        icons.close.texture_id(ctx),
                        Vec2::new(sizes.toolbar_icon, sizes.toolbar_icon),
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

fn render_hints(
    title: &str,
    shortcuts: Option<&[MdAnnotationShortcut]>,
    available_space: Rect,
    painter: &Painter,
    cx: &egui::Context,
    theme: &AppTheme,
) {
    let AppTheme {
        fonts,
        colors,
        sizes,
    } = theme;

    let hint_color = theme.colors.outline_fg;

    painter.text(
        available_space.center_top(), //+ vec2(0., sizes.header_footer)
        Align2::CENTER_TOP,
        title,
        FontId {
            size: fonts.size.h1,
            family: fonts.family.normal.clone(),
        },
        hint_color,
    );

    match shortcuts {
        Some(shortcuts) if shortcuts.len() > 0 => {
            let hints_font_id = FontId {
                size: fonts.size.normal,
                family: fonts.family.normal.clone(),
            };

            let starting_point = available_space.center()
                - vec2(
                    0.,
                    ((2 * shortcuts.len() - 1) as f32) / 2.0 * fonts.size.normal,
                );

            for (i, md_shortcut) in shortcuts.iter().enumerate() {
                painter.text(
                    starting_point + vec2(0., (i as f32) * 2.0 * fonts.size.normal),
                    Align2::RIGHT_CENTER,
                    format!("{}  ", md_shortcut.name,),
                    hints_font_id.clone(),
                    hint_color,
                );
                painter.text(
                    starting_point + vec2(0., (i as f32) * 2.0 * fonts.size.normal),
                    Align2::CENTER_CENTER,
                    ":",
                    hints_font_id.clone(),
                    hint_color,
                );
                painter.text(
                    starting_point + vec2(0., (i as f32) * 2.0 * fonts.size.normal),
                    Align2::LEFT_CENTER,
                    format!("  {}", cx.format_shortcut(&md_shortcut.shortcut)),
                    hints_font_id.clone(),
                    hint_color,
                );
            }
        }
        _ => (),
    };
}

fn number_to_key(key: u8) -> Option<Key> {
    match key {
        0 => Some(Key::Num0),
        1 => Some(Key::Num1),
        2 => Some(Key::Num2),
        3 => Some(Key::Num3),
        4 => Some(Key::Num4),
        5 => Some(Key::Num5),
        6 => Some(Key::Num6),
        7 => Some(Key::Num7),
        8 => Some(Key::Num8),
        9 => Some(Key::Num9),
        _ => None,
    }
}
