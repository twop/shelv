use std::{
    cell::RefCell,
    ops::{Range, RangeBounds},
    path::Path,
    rc::Rc,
    sync::Arc,
};

use eframe::{
    egui::{
        self,
        text_edit::{CCursorRange, TextEditState},
        Id, TextFormat,
    },
    epaint::{
        pos2,
        text::{layout, LayoutJob},
        vec2, Color32, FontId, Rect, Stroke, TextureHandle, TextureId, Vec2,
    },
};
use pulldown_cmark::HeadingLevel;
use smallvec::SmallVec;

struct Theme {
    h1: FontId,
    h2: FontId,
    h3: FontId,
    h4: FontId,
    body: FontId,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            h1: FontId::proportional(24.),
            h2: FontId::proportional(20.),
            h3: FontId::proportional(18.),
            h4: FontId::proportional(16.),
            body: FontId::proportional(14.),
        }
    }
}

pub struct State {
    markdown: String,
    saved: String,
    theme: Theme,
    prev_md_layout: MdLayout,
}

impl Default for State {
    fn default() -> Self {
        Self {
            markdown: "## title\n- item1".to_string(),
            saved: "".to_string(),
            theme: Default::default(),
            prev_md_layout: MdLayout::new(),
        }
    }
}

struct MarkdownState {
    nesting: i32,
    bold: i32,
    strike: i32,
    emphasis: i32,
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
    fn to_text_format(&self, theme: &Theme) -> TextFormat {
        let font_id = match self.heading {
            [h1, ..] if h1 > 0 => &theme.h1,
            [_, h2, ..] if h2 > 0 => &theme.h2,
            [_, _, h3, ..] if h3 > 0 => &theme.h3,
            [_, _, _, h4, ..] if h4 > 0 => &theme.h4,
            [_, _, _, _, h5, ..] if h5 > 0 => &theme.h4,
            [_, _, _, _, _, h6] if h6 > 0 => &theme.h4,
            _ => &theme.body,
        };

        let mut res = TextFormat {
            font_id: font_id.clone(),
            ..Default::default()
        };
        if self.bold > 0 {
            // todo add a different font
            res.underline = Stroke::new(0.1, Color32::LIGHT_GRAY);
        }

        if self.strike > 0 || self.emphasis > 0 {
            // todo add a different font
            res.strikethrough = Stroke::new(0.2, Color32::LIGHT_GRAY);
        }

        res
    }

    // fn has_opened_decor(&self) -> bool {
    //     self.bold > 0 || self.emphasis > 0 || self.strike > 0
    // }
}

impl MarkdownState {
    fn new() -> Self {
        Self {
            nesting: 0,
            bold: 0,
            strike: 0,
            emphasis: 0,
            heading: Default::default(),
        }
    }
}

enum PointKind {
    Start,
    End,
}

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

#[derive(Debug, Copy, Clone)]
enum Annotation {
    Strike,
    Bold,
    Emphasis,
    Heading(HeadingLevel),
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
                    annotation,
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

    fn layout(&mut self, text: &str, theme: &Theme) -> LayoutJob {
        let MdLayout { points, .. } = self;
        points.sort_by_key(|p| p.offset);

        let mut pos: usize = 0;
        let mut job = LayoutJob::default();

        let mut state = MarkdownState::new();

        for point in points {
            job.append(
                text.get(pos..point.offset).unwrap_or(""),
                0.0,
                state.to_text_format(theme),
            );

            let delta = match point.kind {
                PointKind::Start => 1,
                PointKind::End => -1,
            };

            match point.annotation {
                Annotation::Strike => state.strike += delta,
                Annotation::Bold => state.bold += delta,
                Annotation::Emphasis => state.emphasis += delta,
                Annotation::Heading(level) => state.heading[level as usize] += delta,
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
pub fn render(state: &mut State, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
                    for (ev, range) in parser.into_offset_iter() {
                        println!("{:?} -> {:?}", ev, &text[range.start..range.end]);
                    }
                    println!("---parser-end---");
                    state.saved = text.to_string();
                }

                let parser = pulldown_cmark::Parser::new_ext(text, options);

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
                            Item => md.event(Ev::ListItem, range),
                            Heading(level, _, _) => md.event(Ev::Heading(level), range),
                            List(starting_index) => md.event(Ev::ListStart(starting_index), range),
                            _ => (),
                        },

                        End(List(_)) => md.event(Ev::ListEnd, range),

                        TaskListMarker(checked) => md.event(Ev::TaskMarker(checked), range),
                        _ => (),
                    }
                }

                let mut job = md.layout(text, &state.theme);
                job.wrap.max_width = wrap_width;

                // let mut galley = layout(&mut ui.ctx().fonts().lock().fonts, job.into());
                // Arc::new(galley)
                let galley = ui.fonts(|f| f.layout_job(job));

                galley
            };

            let id = Id::new("text_edit");

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

            if ui.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::B)) {
                if let (Some(text_cursor_range), Some(mut edit_state)) =
                    (output.cursor_range, TextEditState::load(ui.ctx(), id))
                {
                    let text = &mut state.markdown;
                    use egui::TextBuffer as _;
                    let selected_chars = text_cursor_range.as_sorted_char_range();
                    let selected_text = text.char_range(selected_chars.clone());

                    let is_already_bold = selected_text.starts_with("**")
                        && selected_text.ends_with("**")
                        && selected_text.len() >= 4;

                    if is_already_bold {
                        text.delete_char_range(Range {
                            start: selected_chars.start,
                            end: selected_chars.start + 2,
                        });
                        text.delete_char_range(Range {
                            start: selected_chars.end - 4,
                            end: selected_chars.end - 2,
                        });
                    } else {
                        text.insert_text("**", selected_chars.start);
                        text.insert_text("**", selected_chars.end + 2);
                    };

                    let [min, max] = text_cursor_range.as_ccursor_range().sorted();

                    println!("prev cursor: {:#?}", edit_state.ccursor_range());
                    edit_state.set_ccursor_range(Some(CCursorRange::two(
                        min,
                        if is_already_bold { max - 4 } else { max + 4 },
                    )));

                    println!("next cursor: {:#?}", edit_state.ccursor_range());

                    edit_state.store(ui.ctx(), id);
                }
            }

            state.prev_md_layout = md;
        });
    });
}
