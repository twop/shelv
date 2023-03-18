use std::ops::Range;

use eframe::{
    egui::TextFormat,
    epaint::{text::LayoutJob, Color32, FontId, Stroke},
};
use pulldown_cmark::HeadingLevel;
use smallvec::{smallvec, SmallVec};
use syntect::{
    easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet, util::LinesWithEndings,
};

use crate::theme::{AppTheme, ColorTheme, FontTheme};

pub enum Ev {
    Strike,
    Bold,
    Emphasis,
    Text,
    Heading(HeadingLevel),
    CodeBlock { lang: String },
    ListItem,
    TaskMarker(bool),
    ListStart(Option<u64>),
    ListEnd,
    RawLink { url: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpanIndex(usize);

#[derive(Debug)]
enum PointKind {
    Start,
    End,
}

#[derive(Debug, Copy, Clone)]
enum Annotation {
    Strike,
    Bold,
    Emphasis,
    Text,
    TaskMarker,
    Link,
    Heading,
    CodeBlock,
}

#[derive(Debug)]
struct AnnotationPoint {
    str_offset: usize,
    span_index: SpanIndex,
    kind: PointKind, // 1 or -1 (start and end respectively)
    annotation: Annotation,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SpanKind {
    Strike,
    Bold,
    Emphasis,
    Text,
    TaskMarker,
    Link,
    Heading,
    CodeBlock,
    CodeBlockContent,
    // List,
    ListItem,
}

#[derive(Debug, Clone)]
pub enum SpanMeta {
    CodeBlock {
        content_span: Option<SpanIndex>,
    },
    CodeBlockContent {
        lang: String,
        block_span: SpanIndex,
    },
    TaskMarker {
        checked: bool,
        list_item_index: SpanIndex,
    },
    ListItem(ListItemDesc),
    Heading(HeadingLevel),
    Link {
        url: String,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct ListItemDesc {
    pub item_index: u32,
    pub depth: i32,
    pub starting_index: Option<u64>,
}

struct ListDesc {
    starting_index: Option<u64>,
    items_count: u32,
}

struct MarkdownState {
    nesting: i8,
    bold: i8,
    strike: i8,
    emphasis: i8,
    text: i8,
    link: i8,
    task_marker: i8,
    heading: [i8; 6],
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
            link: 0,
            task_marker: 0,
        }
    }
}

pub struct TextStructure {
    points: Vec<AnnotationPoint>,
    spans: Vec<(SpanKind, Range<usize>)>,
    metadata: Vec<(SpanIndex, SpanMeta)>,
}

pub struct TextStructureBuilder<'a> {
    text: &'a str,
    list_stack: SmallVec<[ListDesc; 4]>,
    last_code_block: Option<(SpanIndex, Range<usize>, String)>,

    spans: Vec<(SpanKind, Range<usize>)>,
    metadata: Vec<(SpanIndex, SpanMeta)>,
}

pub enum InteractiveTextPart<'a> {
    // byte pos the text, note that it is not the same as char
    TaskMarker {
        byte_range: Range<usize>,
        checked: bool,
    },
    Link(&'a str),
}

pub struct SpanSearchResult {
    pub span_byte_range: Range<usize>,
    pub content_byte_range: Range<usize>,
}

impl<'a> TextStructureBuilder<'a> {
    pub fn start(text: &'a str) -> Self {
        Self {
            text,
            list_stack: Default::default(),
            last_code_block: None,
            spans: vec![],
            metadata: vec![],
        }
    }

    pub fn event(&mut self, ev: Ev, pos: Range<usize>) {
        match ev {
            Ev::ListItem => {
                // depth starts with zero for top level list
                let depth = self.list_stack.len() as i32 - 1;
                if let Some(list_desc) = self.list_stack.last_mut() {
                    let item_index = list_desc.items_count;
                    let starting_index = list_desc.starting_index.clone();

                    list_desc.items_count += 1;

                    self.add_with_meta(
                        SpanKind::ListItem,
                        pos,
                        SpanMeta::ListItem(ListItemDesc {
                            item_index,
                            depth,
                            starting_index,
                        }),
                    );
                }
            }

            Ev::TaskMarker(checked) => {
                // the last one to add would be the most nested, thus the one we need
                let list_item = self.spans.iter().enumerate().rev().find(
                    |(_, (kind, byte_range))| match kind {
                        SpanKind::ListItem => {
                            byte_range.start <= pos.start && byte_range.end >= pos.end
                        }
                        _ => false,
                    },
                );

                if let Some((index, _)) = list_item {
                    self.add_with_meta(
                        SpanKind::TaskMarker,
                        pos,
                        SpanMeta::TaskMarker {
                            checked,
                            list_item_index: SpanIndex(index),
                        },
                    );
                }
            }

            Ev::Heading(level) => {
                self.add_with_meta(SpanKind::Heading, pos, SpanMeta::Heading(level));
            }

            Ev::ListStart(starting_index) => self.list_stack.push(ListDesc {
                starting_index,
                items_count: 0,
            }),

            Ev::ListEnd => {
                self.list_stack.pop();
            }
            Ev::Strike => {
                self.add(SpanKind::Heading, pos);
            }
            Ev::Bold => {
                self.add(SpanKind::Bold, pos);
            }
            Ev::Emphasis => {
                self.add(SpanKind::Emphasis, pos);
            }
            Ev::Text => match self.last_code_block.clone() {
                // Text can be within a code block
                Some((block_span, block_pos, lang))
                    if block_pos.start <= pos.start && block_pos.end >= pos.end =>
                {
                    // trim the last \n if any, it look odd,
                    // but \n is parsed as a part of the body, thus bandage that
                    let pos = if self.text[pos.clone()].ends_with("\n") {
                        pos.start..pos.end - 1
                    } else {
                        pos
                    };

                    let span_index = self.add_with_meta(
                        SpanKind::CodeBlockContent,
                        pos.clone(),
                        SpanMeta::CodeBlockContent {
                            block_span,
                            lang: lang.clone(),
                        },
                    );

                    // notify parent block that there is some content
                    if let Some(SpanMeta::CodeBlock { content_span, .. }) = self
                        .metadata
                        .iter_mut()
                        .find(|(i, _)| *i == block_span)
                        .map(|(_, meta)| meta)
                    {
                        *content_span = Some(span_index);
                    }
                }

                _ => {
                    self.add(SpanKind::Text, pos);
                }
            },

            Ev::CodeBlock { lang } => {
                let span_index = self.add_with_meta(
                    SpanKind::CodeBlock,
                    pos.clone(),
                    SpanMeta::CodeBlock { content_span: None },
                );

                self.last_code_block = Some((span_index, pos, lang));
            }
            Ev::RawLink { url } => {
                self.add_with_meta(SpanKind::Link, pos, SpanMeta::Link { url });
            }
        }
    }

    fn add(&mut self, kind: SpanKind, pos: Range<usize>) -> SpanIndex {
        let index = SpanIndex(self.spans.len());
        self.spans.push((kind, pos));
        index
    }

    fn add_with_meta(&mut self, kind: SpanKind, pos: Range<usize>, meta: SpanMeta) -> SpanIndex {
        let index = SpanIndex(self.spans.len());
        self.metadata.push((index, meta));
        self.spans.push((kind, pos));
        index
    }

    pub fn finish(mut self) -> TextStructure {
        let Self {
            text,
            list_stack,
            last_code_block,
            spans,
            metadata,
        } = self;

        let points = fill_annotation_points(vec![], &spans, &metadata);

        TextStructure {
            points,
            spans,
            metadata,
        }
    }
}

impl TextStructure {
    pub fn create_layout_job(
        &self,
        text: &str,
        theme: &AppTheme,
        syntax_set: &SyntaxSet,
        theme_set: &ThemeSet,
    ) -> LayoutJob {
        let mut pos: usize = 0;
        let mut job = LayoutJob::default();

        let mut state = MarkdownState::new();

        let code_font_id = FontId {
            size: theme.fonts.size.normal,
            family: theme.fonts.family.code.clone(),
        };

        // println!("points: {:#?}", points);
        for point in self.points.iter() {
            if let (Annotation::CodeBlock, PointKind::End) = (&point.annotation, &point.kind) {
                let code = text.get(pos..point.str_offset).unwrap_or("");

                let lang = find_metadata(point.span_index, &self.metadata)
                    .and_then(|meta| match meta {
                        SpanMeta::CodeBlockContent { lang, .. } => Some(lang.as_str()),
                        _ => None,
                    })
                    .unwrap_or("");

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
                    text.get(pos..point.str_offset).unwrap_or(""),
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
                    Annotation::Link => state.link += delta,
                    Annotation::TaskMarker => state.task_marker += delta,
                    Annotation::Emphasis => state.emphasis += delta,
                    Annotation::Heading => {
                        if let Some(SpanMeta::Heading(level)) =
                            find_metadata(point.span_index, &self.metadata)
                        {
                            state.heading[*level as usize] += delta;
                        }
                    }
                    Annotation::CodeBlock => (),
                }
            }

            pos = point.str_offset;
        }

        // the last piece of text
        job.append(
            text.get(pos..).unwrap_or(""),
            0.0,
            state.to_text_format(theme),
        );

        job
    }

    pub fn find_interactive_text_part(&self, byte_cursor: usize) -> Option<InteractiveTextPart> {
        self.spans
            .iter()
            .enumerate()
            .find_map(|(index, (kind, part_pos))| match kind {
                SpanKind::TaskMarker | SpanKind::Link if part_pos.contains(&byte_cursor) => {
                    find_metadata(SpanIndex(index), &self.metadata).and_then(|meta| {
                        match (kind, meta) {
                            (SpanKind::TaskMarker, SpanMeta::TaskMarker { checked, .. }) => {
                                Some(InteractiveTextPart::TaskMarker {
                                    byte_range: part_pos.clone(),
                                    checked: *checked,
                                })
                            }
                            (SpanKind::Link, SpanMeta::Link { url }) => {
                                Some(InteractiveTextPart::Link(url.as_str()))
                            }
                            _ => None,
                        }
                    })
                }
                _ => None,
            })
    }

    pub fn find_surrounding_list_item(&self, byte_range: Range<usize>) -> Option<ListItemDesc> {
        self.spans
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, (kind, item_pos))| match kind {
                SpanKind::ListItem
                    if item_pos.start <= byte_range.start && item_pos.end >= byte_range.end =>
                {
                    Some(SpanIndex(index))
                }
                _ => None,
            })
            .and_then(|index| find_metadata(index, &self.metadata))
            .and_then(|meta| match meta {
                SpanMeta::ListItem(desc) => Some(*desc),
                _ => None,
            })
    }

    pub fn find_span_at(
        &self,
        span_kind: SpanKind,
        byte_range: Range<usize>,
    ) -> Option<SpanSearchResult> {
        self.spans
            .iter()
            .enumerate()
            .rev()
            .find(|(_, (kind, item_pos))| {
                *kind == span_kind
                    && item_pos.start <= byte_range.start
                    && item_pos.end >= byte_range.end
            })
            .and_then(|(index, (kind, pos))| {
                let ranges = match kind {
                    SpanKind::Strike => Some((pos.clone(), pos.start + 2..pos.end - 2)), //~~{}~~
                    SpanKind::Bold => Some((pos.clone(), pos.start + 2..pos.end - 2)),   //**{}**
                    SpanKind::Emphasis => Some((pos.clone(), pos.start + 1..pos.end - 1)), //*{}*
                    SpanKind::CodeBlock => find_metadata(SpanIndex(index), &self.metadata)
                        .and_then(|meta| match meta {
                            SpanMeta::CodeBlock { content_span } => match content_span {
                                Some(span) => {
                                    // if there is code inside, return it as the inner range
                                    let (_, content_range) = self.spans[span.0].clone();
                                    Some((pos.clone(), content_range))
                                }
                                // means that there is no content yet inside the code block
                                // thus just return empty range for the content, maybe make it more elegant
                                None => Some((pos.clone(), pos.start..pos.start)),
                            },
                            _ => None,
                        }),
                    SpanKind::Text => todo!(),
                    SpanKind::TaskMarker => todo!(),
                    SpanKind::Link => todo!(),
                    SpanKind::Heading => todo!(),
                    SpanKind::CodeBlockContent => todo!(),
                    SpanKind::ListItem => todo!(),
                };

                ranges.map(|(outer, inner)| SpanSearchResult {
                    span_byte_range: outer,
                    content_byte_range: inner,
                })
            })
    }
}

fn fill_annotation_points(
    mut points: Vec<AnnotationPoint>,
    spans: &Vec<(SpanKind, Range<usize>)>,
    metadata: &Vec<(SpanIndex, SpanMeta)>,
) -> Vec<AnnotationPoint> {
    for (index, (kind, pos)) in spans.iter().enumerate() {
        let span_index = SpanIndex(index);
        let pair: SmallVec<[(Annotation, &Range<usize>); 2]> = match kind {
            SpanKind::Strike => smallvec![(Annotation::Strike, pos)],
            SpanKind::Bold => smallvec![(Annotation::Bold, pos)],
            SpanKind::Emphasis => smallvec![(Annotation::Emphasis, pos)],
            SpanKind::Text => smallvec![(Annotation::Text, pos)],
            SpanKind::TaskMarker => match find_metadata(span_index, metadata) {
                Some(SpanMeta::TaskMarker {
                    checked,
                    list_item_index,
                }) => match *checked {
                    true => {
                        let (_, list_item_pos) = &spans[list_item_index.0];
                        smallvec![
                            (Annotation::TaskMarker, pos),
                            (Annotation::Strike, list_item_pos)
                        ]
                    }
                    false => smallvec![(Annotation::TaskMarker, pos)],
                },
                _ => smallvec![],
            },
            SpanKind::Link => smallvec![(Annotation::Link, pos)],
            SpanKind::Heading => smallvec![(Annotation::Heading, pos)],
            SpanKind::CodeBlockContent => smallvec![(Annotation::CodeBlock, pos)],
            SpanKind::CodeBlock => smallvec![],
            SpanKind::ListItem => smallvec![],
        };

        for (annotation, pos) in pair {
            points.push(AnnotationPoint {
                str_offset: pos.start,
                kind: PointKind::Start,
                annotation,
                span_index,
            });

            points.push(AnnotationPoint {
                str_offset: pos.end,
                kind: PointKind::End,
                annotation,
                span_index,
            });
        }
    }

    points.sort_by_key(|p| p.str_offset);
    points
}

#[inline(always)]
fn find_metadata(index: SpanIndex, metadata: &Vec<(SpanIndex, SpanMeta)>) -> Option<&SpanMeta> {
    metadata
        .iter()
        .find(|(i, _)| index == *i)
        .map(|(_, meta)| meta)
}

impl MarkdownState {
    fn to_text_format(&self, theme: &AppTheme) -> TextFormat {
        let AppTheme {
            fonts: FontTheme { size, family },
            colors,
            sizes: _,
        } = theme;

        let ColorTheme {
            md_strike,
            md_annotation,
            md_body,
            md_header,
            md_link,
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

        let color = match (
            self.link > 0,
            self.heading.iter().any(|h| *h > 0),
            self.text > 0,
        ) {
            (true, _, _) => *md_link,
            (false, _, false) => *md_annotation,
            (false, true, true) => *md_header,
            (false, false, true) => *md_body,
        };

        let font_family = match (emphasis, bold) {
            (true, true) => &family.bold_italic,
            (false, true) => &family.bold,
            (true, false) => &family.italic,
            (false, false) => &family.normal,
        };

        // && command_pressed;
        TextFormat {
            color: if self.task_marker > 0 {
                *md_link
            } else {
                color
            },
            font_id: FontId::new(font_size, font_family.clone()),
            strikethrough: if self.strike > 0 {
                Stroke::new(0.6, *md_strike)
            } else {
                Stroke::NONE
            },
            underline: if self.link > 0 {
                Stroke::new(0.6, *md_link)
            } else {
                Stroke::NONE
            },
            // background: if should_highlight_task {
            //     md_link.gamma_multiply(0.2)
            // } else {
            //     Color32::TRANSPARENT
            // },
            ..Default::default()
        }
    }
}
