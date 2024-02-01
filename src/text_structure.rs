use std::ops::Range;

use eframe::{
    egui::TextFormat,
    epaint::{text::LayoutJob, Color32, FontId, Stroke},
};
use linkify::LinkFinder;
use pulldown_cmark::{CodeBlockKind, HeadingLevel};
use smallvec::{smallvec, SmallVec};
use syntect::{
    easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet, util::LinesWithEndings,
};

use crate::theme::{AppTheme, ColorTheme, FontTheme};

// pub enum Ev {
//     // Strike,
//     // Bold,
//     // Emphasis,
//     // Text,
//     // Heading(HeadingLevel),
//     // Paragraph,
//     // CodeBlock { lang: String },
//     // Code,
//     ListItem,
//     // TaskMarker(bool),
//     ListStart(Option<u64>),
//     ListEnd,
//     // RawLink { url: String },
//     // MdLink,
//     // Image,
//     // Html,
//     // ContainerEnd,
// }

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
    Heading(HeadingLevel),
    CodeBlock,
    Code,
}

#[derive(Debug)]
struct AnnotationPoint {
    str_offset: usize,
    // span_index: SpanIndex,
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
    MdLink,
    Heading(HeadingLevel),
    Paragraph,
    CodeBlock,
    CodeBlockContent,
    List,
    Code,
    Html,
    ListItem,
    Image,
    Root,
}

#[derive(Debug, Clone)]
pub enum SpanMeta {
    CodeBlock {
        lang: String,
    },
    // CodeBlockContent {
    //     block_span: SpanIndex,
    // },
    TaskMarker {
        checked: bool,
        // list_item_index: SpanIndex,
    },
    List(ListDesc),
    Link {
        url: String,
    },
}

// #[derive(Debug, Clone)]
// pub struct ListItemDesc {
//     pub item_index: u32,
//     pub depth: i32,
//     pub started_numbered_index: Option<u64>,
//     pub item_byte_pos: Range<usize>,
// }

// impl ListItemDesc {
//     pub fn is_numbered(&self) -> bool {
//         self.started_numbered_index.is_some()
//     }
// }

#[derive(Debug, Clone)]
struct ListDesc {
    starting_index: Option<u64>,
    // items_count: u32,
}

struct MarkdownRunningState {
    nesting: i8,
    bold: i8,
    strike: i8,
    emphasis: i8,
    text: i8,
    link: i8,
    task_marker: i8,
    code: i8,
    heading: [i8; 6],
}

impl MarkdownRunningState {
    fn new() -> Self {
        Self {
            nesting: 0,
            bold: 0,
            strike: 0,
            code: 0,
            emphasis: 0,
            heading: Default::default(),
            text: 0,
            link: 0,
            task_marker: 0,
        }
    }
}

#[derive(Debug, Clone)]
struct SpanDesc {
    kind: SpanKind,
    byte_pos: Range<usize>,
    parent: SpanIndex,
}

#[derive(Debug)]
pub struct TextStructure {
    points: Vec<AnnotationPoint>,
    raw_links: Vec<RawLink>,
    spans: Vec<SpanDesc>,
    metadata: Vec<(SpanIndex, SpanMeta)>,
}

#[derive(Debug)]
struct RawLink {
    url: String,
    byte_pos: Range<usize>,
}

pub struct TextStructureBuilder<'a> {
    text: &'a str,
    container_stack: SmallVec<[SpanIndex; 8]>,
    spans: Vec<SpanDesc>,
    raw_links: Vec<RawLink>,
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
            spans: vec![SpanDesc {
                kind: SpanKind::Root,
                byte_pos: 0..0,
                parent: SpanIndex(0),
            }],
            metadata: vec![],
            container_stack: smallvec![SpanIndex(0)],
            raw_links: vec![],
        }
    }

    // pub fn event(&mut self, ev: Ev, pos: Range<usize>) {
    //     match ev {
    //         Ev::ListItem => {
    //             // depth starts with zero for top level list
    //             let depth = self.list_stack.len() as i32 - 1;
    //             if let Some(list_desc) = self.list_stack.last_mut() {
    //                 let item_index = list_desc.items_count;
    //                 let starting_index = list_desc.starting_index.clone();

    //                 list_desc.items_count += 1;

    //                 let list_item_pos = trim_trailing_new_lines(&self.text, &pos);
    //                 self.add_with_meta(
    //                     SpanKind::ListItem,
    //                     list_item_pos.clone(),
    //                     SpanMeta::ListItem(ListItemDesc {
    //                         item_index,
    //                         depth,
    //                         started_numbered_index: starting_index,
    //                         item_byte_pos: list_item_pos,
    //                     }),
    //                 );
    //             }
    //         } // Ev::TaskMarker(checked) => {
    //           //     // the last one to add would be the most nested, thus the one we need
    //           //     let list_item = self.spans.iter().enumerate().rev().find(
    //           //         |(_, SpanDesc { kind, byte_pos, .. })| match kind {
    //           //             SpanKind::ListItem => is_sub_range(byte_pos, &pos),
    //           //             _ => false,
    //           //         },
    //           //     );

    //           //     if let Some((index, _)) = list_item {
    //           //         self.add_with_meta(
    //           //             SpanKind::TaskMarker,
    //           //             pos,
    //           //             SpanMeta::TaskMarker {
    //           //                 checked,
    //           //                 list_item_index: SpanIndex(index),
    //           //             },
    //           //         );
    //           //     }
    //           // }
    //           // Ev::ListStart(starting_index) => self.list_stack.push(ListDesc {
    //           //     starting_index,
    //           //     items_count: 0,
    //           // }),

    //           // Ev::ListEnd => {
    //           //     self.list_stack.pop();
    //           // }
    //           // Ev::Text => match self.last_code_block.clone() {
    //           //     // Text can be within a code block
    //           //     Some((block_span, block_pos, lang)) if is_sub_range(&block_pos, &pos) => {
    //           //         // trim the last \n if any, it look odd,
    //           //         // but \n is parsed as a part of the body, thus bandage that
    //           //         let pos = trim_trailing_new_lines(&self.text, &pos);

    //           //         let span_index = self.add_with_meta(
    //           //             SpanKind::CodeBlockContent,
    //           //             pos.clone(),
    //           //             SpanMeta::CodeBlockContent {
    //           //                 block_span,
    //           //                 lang: lang.clone(),
    //           //             },
    //           //         );

    //           //         // notify parent block that there is some content
    //           //         if let Some(SpanMeta::CodeBlock { content_span, .. }) = self
    //           //             .metadata
    //           //             .iter_mut()
    //           //             .find(|(i, _)| *i == block_span)
    //           //             .map(|(_, meta)| meta)
    //           //         {
    //           //             *content_span = Some(span_index);
    //           //         }
    //           //     }

    //           //     _ => {
    //           //         self.add(SpanKind::Text, pos);
    //           //     }
    //           // },

    //           // Ev::CodeBlock { lang } => {
    //           //     let span_index = self.add_with_meta(
    //           //         SpanKind::CodeBlock,
    //           //         pos.clone(),
    //           //         SpanMeta::CodeBlock { lang },
    //           //     );
    //           // }
    //           // Ev::Heading(level) => {
    //           //     self.add(
    //           //         SpanKind::Heading(level),
    //           //         trim_trailing_new_lines(&self.text, &pos),
    //           //     );
    //           // }

    //           // Ev::Paragraph => {
    //           //     self.add(
    //           //         SpanKind::Paragraph,
    //           //         trim_trailing_new_lines(&self.text, &pos),
    //           //     );
    //           // }
    //     }
    // }

    fn add(&mut self, kind: SpanKind, pos: Range<usize>) -> SpanIndex {
        let index = SpanIndex(self.spans.len());
        self.spans.push(SpanDesc {
            kind,
            byte_pos: pos,
            parent: self.container_stack.last().unwrap().clone(),
        });
        index
    }

    fn add_with_meta(&mut self, kind: SpanKind, pos: Range<usize>, meta: SpanMeta) -> SpanIndex {
        let index = self.add(kind, pos);
        self.metadata.push((index, meta));
        index
    }

    pub fn finish(self) -> TextStructure {
        let Self {
            text,
            spans,
            metadata,
            raw_links,
            ..
        } = self;

        let points = fill_annotation_points(vec![], &spans, &metadata, &raw_links);

        TextStructure {
            points,
            spans,
            metadata,
            raw_links,
        }
    }

    fn print_structure(&self) {
        println!("\n\n-----parser-----");
        println!("text: {:?}", self.text);
        println!("-----text-end-----\n");

        let mut container_stack: SmallVec<[SpanIndex; 8]> = Default::default();

        for (index, span) in self.spans.iter().enumerate() {
            let SpanDesc {
                kind,
                byte_pos: range,
                parent,
            } = span;

            if container_stack.is_empty() {
                container_stack.push(*parent);
            }

            if container_stack.last().unwrap() != parent {
                if let Some((found_parent_index, _)) = container_stack
                    .iter()
                    .enumerate()
                    .find(|(_, p)| *p == parent)
                {
                    let drop_count = container_stack.len() - found_parent_index - 1;
                    for _ in 0..drop_count {
                        container_stack.pop();
                    }
                } else {
                    container_stack.push(*parent);
                }
            }

            println!(
                "{}{kind:?} ({}-{})-> {:?}",
                "  ".repeat(container_stack.len() - 1),
                range.start,
                range.end,
                &self.text[range.start..range.end]
            );
        }

        println!("---structure-end---\n");

        let md_parser_options = pulldown_cmark::Options::ENABLE_STRIKETHROUGH
            | pulldown_cmark::Options::ENABLE_TASKLISTS
            | pulldown_cmark::Options::ENABLE_SMART_PUNCTUATION;

        let parser = pulldown_cmark::Parser::new_ext(self.text, md_parser_options);

        let mut depth = 0;
        for (ev, range) in parser.into_offset_iter() {
            if let pulldown_cmark::Event::End(_) = &ev {
                depth -= 1;
            }

            println!(
                "{}{:?} -> {:?}",
                "  ".repeat(depth),
                ev,
                &self.text[range.start..range.end]
            );

            if let pulldown_cmark::Event::Start(_) = &ev {
                depth += 1;
            }
        }
        println!("---parser-end---");
    }
}

fn trim_trailing_new_lines(text: &str, pos: &Range<usize>) -> Range<usize> {
    let (start, mut end) = (pos.start, pos.end);

    // TODO optimize it by taking a slice from the string
    while start < end && text[start..end].ends_with("\n") {
        end -= 1;
    }

    start..end
}

impl TextStructure {
    pub fn create_from(text: &str) -> TextStructure {
        let mut builder = TextStructureBuilder::start(text);

        let finder = LinkFinder::new();

        for link in finder.links(text) {
            builder.raw_links.push(RawLink {
                url: link.as_str().to_string(),
                byte_pos: link.start()..link.end(),
            });
        }

        let md_parser_options = pulldown_cmark::Options::ENABLE_STRIKETHROUGH
            | pulldown_cmark::Options::ENABLE_TASKLISTS
            | pulldown_cmark::Options::ENABLE_SMART_PUNCTUATION;

        let parser = pulldown_cmark::Parser::new_ext(&text, md_parser_options);

        for (ev, range) in parser.into_offset_iter() {
            use pulldown_cmark::Event::*;
            use pulldown_cmark::Tag::*;
            match ev {
                Start(tag) => {
                    let container = match tag {
                        Strong => Some(builder.add(SpanKind::Bold, range)),
                        Emphasis => Some(builder.add(SpanKind::Emphasis, range)),
                        Strikethrough => Some(builder.add(SpanKind::Strike, range)),

                        CodeBlock(CodeBlockKind::Fenced(lang)) => Some(builder.add_with_meta(
                            SpanKind::CodeBlock,
                            range.clone(),
                            SpanMeta::CodeBlock {
                                lang: lang.as_ref().to_string(),
                            },
                        )),

                        Item => Some(builder.add(SpanKind::ListItem, range)),
                        Heading(level, _, _) => Some(builder.add(
                            SpanKind::Heading(level),
                            trim_trailing_new_lines(&text, &range),
                        )),
                        List(starting_index) => Some(builder.add_with_meta(
                            SpanKind::List,
                            range,
                            SpanMeta::List(ListDesc { starting_index }),
                        )),
                        Paragraph => Some(
                            builder
                                .add(SpanKind::Paragraph, trim_trailing_new_lines(&text, &range)),
                        ),
                        Link(_, url, _) => Some(builder.add_with_meta(
                            SpanKind::MdLink,
                            range,
                            SpanMeta::Link {
                                url: url.to_string(),
                            },
                        )),
                        Image(_, _, _) => Some(builder.add(SpanKind::Image, range)),

                        // We explicitly don't support these containers
                        Table(_)
                        | TableHead
                        | CodeBlock(CodeBlockKind::Indented)
                        | TableRow
                        | TableCell
                        | FootnoteDefinition(_)
                        | BlockQuote => None,
                    };

                    if let Some(container_index) = container {
                        builder.container_stack.push(container_index);
                    }
                }

                End(tag) => {
                    let is_supported_container = match tag {
                        // We explicitly don't support these containers
                        // note that it needs to match "Start" variant
                        Table(_)
                        | TableHead
                        | CodeBlock(CodeBlockKind::Indented)
                        | TableRow
                        | TableCell
                        | FootnoteDefinition(_)
                        | BlockQuote => false,

                        // supported containers. Note that it needs to match "Start" variant
                        Paragraph
                        | CodeBlock(CodeBlockKind::Fenced(_))
                        | Heading(_, _, _)
                        | List(_)
                        | Item
                        | Emphasis
                        | Strong
                        | Strikethrough
                        | Link(_, _, _)
                        | Image(_, _, _) => true,
                    };

                    if is_supported_container {
                        builder.container_stack.pop();
                    }
                }

                Text(_) => {
                    builder.add(SpanKind::Text, range);
                }

                TaskListMarker(checked) => {
                    builder.add_with_meta(
                        SpanKind::TaskMarker,
                        range.clone(),
                        SpanMeta::TaskMarker { checked },
                    );
                }

                Code(_) => {
                    builder.add(SpanKind::Code, range);
                }

                Html(_) => {
                    builder.add(SpanKind::Html, range);
                }

                FootnoteReference(_) | SoftBreak | HardBreak | Rule => (),
            }
        }

        builder.print_structure();

        let text_structure = builder.finish();
        text_structure
    }

    pub fn create_layout_job(
        &self,
        text: &str,
        theme: &AppTheme,
        syntax_set: &SyntaxSet,
        theme_set: &ThemeSet,
    ) -> LayoutJob {
        let mut pos: usize = 0;
        let mut job = LayoutJob::default();

        let mut state = MarkdownRunningState::new();

        let code_font_id = FontId {
            size: theme.fonts.size.normal,
            family: theme.fonts.family.code.clone(),
        };

        // println!("points: {:#?}", points);
        for point in self.points.iter() {
            if let (Annotation::CodeBlock, PointKind::End) = (&point.annotation, &point.kind) {
                let code = text.get(pos..point.str_offset).unwrap_or("");

                let lang = "";
                // let lang = find_metadata(point.span_index, &self.metadata)
                //     .and_then(|meta| match meta {
                //         SpanMeta::CodeBlock { lang, .. } => Some(lang.as_str()),
                //         _ => None,
                //     })
                //     .unwrap_or("");

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
                    Annotation::Heading(level) => {
                        state.heading[*level as usize] += delta;
                    }
                    Annotation::CodeBlock => (),
                    Annotation::Code => state.code += delta,
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

    pub fn find_interactive_text_part(
        &self,
        byte_cursor_pos: usize,
    ) -> Option<InteractiveTextPart> {
        self.spans
            .iter()
            .enumerate()
            .find_map(
                |(
                    index,
                    SpanDesc {
                        kind,
                        byte_pos,
                        parent,
                    },
                )| match kind {
                    SpanKind::TaskMarker | SpanKind::MdLink
                        if byte_pos.contains(&byte_cursor_pos) =>
                    {
                        find_metadata(SpanIndex(index), &self.metadata).and_then(|meta| {
                            match (kind, meta) {
                                (SpanKind::TaskMarker, SpanMeta::TaskMarker { checked, .. }) => {
                                    Some(InteractiveTextPart::TaskMarker {
                                        byte_range: byte_pos.clone(),
                                        checked: *checked,
                                    })
                                }
                                (SpanKind::MdLink, SpanMeta::Link { url }) => {
                                    Some(InteractiveTextPart::Link(url.as_str()))
                                }
                                _ => None,
                            }
                        })
                    }
                    _ => None,
                },
            )
            .or_else(|| {
                self.raw_links.iter().find_map(|RawLink { url, byte_pos }| {
                    if byte_pos.contains(&byte_cursor_pos) {
                        Some(InteractiveTextPart::Link(url.as_str()))
                    } else {
                        None
                    }
                })
            })
    }

    pub fn find_surrounding_span_with_meta(
        &self,
        kind: SpanKind,
        byte_cursor: Range<usize>,
    ) -> Option<(SpanIndex, SpanDesc, SpanMeta)> {
        self.spans
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, span)| match kind {
                SpanKind::ListItem if is_sub_range(&span.byte_pos, &byte_cursor) => {
                    Some((SpanIndex(index), span.clone()))
                }
                _ => None,
            })
            .and_then(|(index, span)| {
                find_metadata(index, &self.metadata).map(|meta| (index, span, meta.clone()))
            })
    }

    pub fn find_span_at(
        &self,
        span_kind: SpanKind,
        byte_cursor: Range<usize>,
    ) -> Option<(Range<usize>, SpanIndex)> {
        self.spans
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, SpanDesc { kind, byte_pos, .. })| {
                if *kind == span_kind && is_sub_range(byte_pos, &byte_cursor) {
                    Some((byte_pos.clone(), SpanIndex(i)))
                } else {
                    None
                }
            })
    }

    pub fn find_any_span_at(&self, byte_cursor: Range<usize>) -> Option<(Range<usize>, SpanIndex)> {
        self.spans
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, SpanDesc { byte_pos, .. })| {
                if is_sub_range(byte_pos, &byte_cursor) {
                    Some((byte_pos.clone(), SpanIndex(i)))
                } else {
                    None
                }
            })
    }

    pub fn get_span_inner_content(&self, idx: SpanIndex) -> Range<usize> {
        let SpanIndex(index) = idx;
        let SpanDesc {
            kind,
            byte_pos: pos,
            ..
        } = self.spans[index].clone();
        match kind {
            SpanKind::Strike => pos.start + 2..pos.end - 2, //~~{}~~
            SpanKind::Bold => pos.start + 2..pos.end - 2,   //**{}**
            SpanKind::Emphasis | SpanKind::Code => pos.start + 1..pos.end - 1, //*{}* or `{}`

            // TODO what to do with Root?
            SpanKind::Root => 0..0,

            // these can be considered as atomic, thus return itself
            SpanKind::Text
            | SpanKind::TaskMarker
            | SpanKind::MdLink
            | SpanKind::CodeBlockContent
            | SpanKind::Html
            | SpanKind::Image => pos,

            // these are all containers, thus we need to enumerate their content
            SpanKind::ListItem
            | SpanKind::Heading(_)
            | SpanKind::Paragraph
            | SpanKind::CodeBlock
            | SpanKind::List => self
                .spans
                .iter()
                .skip(index + 1)
                .take_while(|desc| desc.byte_pos.end <= pos.end)
                .fold(None, |area, desc| match area {
                    None => Some(desc.byte_pos.clone()),
                    Some(area) => {
                        Some(area.start.min(desc.byte_pos.start)..area.end.max(desc.byte_pos.end))
                    }
                })
                .unwrap_or(pos.start..pos.start),
        }
    }
}

fn fill_annotation_points(
    mut points: Vec<AnnotationPoint>,
    spans: &Vec<SpanDesc>,
    metadata: &Vec<(SpanIndex, SpanMeta)>,
    raw_links: &Vec<RawLink>,
) -> Vec<AnnotationPoint> {
    for (
        index,
        SpanDesc {
            kind,
            byte_pos,
            parent,
        },
    ) in spans.iter().enumerate()
    {
        let pos = byte_pos.clone();
        let span_index = SpanIndex(index);
        let pair: SmallVec<[(Annotation, Range<usize>); 2]> = match kind {
            SpanKind::Strike => smallvec![(Annotation::Strike, pos)],
            SpanKind::Bold => smallvec![(Annotation::Bold, pos)],
            SpanKind::Emphasis => smallvec![(Annotation::Emphasis, pos)],
            SpanKind::Text => smallvec![(Annotation::Text, pos)],
            SpanKind::TaskMarker => match find_metadata(span_index, metadata) {
                Some(SpanMeta::TaskMarker { checked }) => match *checked {
                    true => {
                        let list_item_content = iterate_children_of(*parent, spans).fold(
                            pos.clone(),
                            |range, sibling| {
                                range.start.min(sibling.byte_pos.start)
                                    ..range.end.max(sibling.byte_pos.end)
                            },
                        );
                        smallvec![
                            (Annotation::TaskMarker, pos),
                            (Annotation::Strike, list_item_content)
                        ]
                    }
                    false => smallvec![(Annotation::TaskMarker, pos)],
                },
                _ => smallvec![],
            },

            SpanKind::Heading(level) => smallvec![(Annotation::Heading(*level), pos)],
            SpanKind::CodeBlockContent => smallvec![(Annotation::CodeBlock, pos)],
            SpanKind::Code => smallvec![(Annotation::Code, pos)],

            SpanKind::MdLink
            | SpanKind::CodeBlock
            | SpanKind::List
            | SpanKind::Root
            | SpanKind::Html
            | SpanKind::Image
            | SpanKind::ListItem
            | SpanKind::Paragraph => smallvec![],
        };

        for (annotation, pos) in pair {
            points.push(AnnotationPoint {
                str_offset: pos.start,
                kind: PointKind::Start,
                annotation,
                // span_index,
            });

            points.push(AnnotationPoint {
                str_offset: pos.end,
                kind: PointKind::End,
                annotation,
                // span_index,
            });
        }
    }

    for (kind, str_offset) in raw_links.iter().flat_map(|link| {
        [
            (PointKind::Start, link.byte_pos.start),
            (PointKind::End, link.byte_pos.end),
        ]
    }) {
        points.push(AnnotationPoint {
            str_offset,
            kind: PointKind::Start,
            annotation: Annotation::Link,
        });
    }

    points.sort_by_key(|p| p.str_offset);
    points
}

#[inline(always)]
fn iterate_children_of(index: SpanIndex, spans: &Vec<SpanDesc>) -> impl Iterator<Item = &SpanDesc> {
    let parent_parent = spans[index.0].parent;
    spans
        .iter()
        .skip(index.0)
        .take_while(move |child| child.parent != parent_parent)
}

#[inline(always)]
fn find_metadata(index: SpanIndex, metadata: &Vec<(SpanIndex, SpanMeta)>) -> Option<&SpanMeta> {
    metadata
        .iter()
        .find(|(i, _)| index == *i)
        .map(|(_, meta)| meta)
}

impl MarkdownRunningState {
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
            md_code,
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

        let color = if self.link > 0 {
            *md_link
        } else if self.code > 0 {
            *md_code
        } else {
            match (self.heading.iter().any(|h| *h > 0), self.text > 0) {
                (_, false) => *md_annotation,
                (true, true) => *md_header,
                (false, true) => *md_body,
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    pub fn test_inner_content() {
        let md = "## ti**tle** \n";

        let structure = TextStructure::create_from(md);

        let (h2_range, idx) = structure
            .find_span_at(SpanKind::Heading(HeadingLevel::H2), 4..6)
            .unwrap();

        println!("{:#?}", structure.spans);
        assert_eq!(Some("## ti**tle** "), md.get(h2_range));

        let content_range = structure.get_span_inner_content(idx);

        // note that it skips the first space due to h2 pattern
        // and doesn't capture the trailing space, and neither "\n"
        assert_eq!(Some("ti**tle**"), md.get(content_range));
    }

    #[test]
    pub fn test_span_detection() {
        let md = "a\n\nb";

        let structure = TextStructure::create_from(md);

        let (a_range, _) = structure.find_span_at(SpanKind::Paragraph, 0..0).unwrap();
        let (b_range, _) = structure.find_span_at(SpanKind::Paragraph, 3..3).unwrap();

        println!("{:#?}", structure.spans);
        assert_eq!(Some("a"), md.get(a_range));
        assert_eq!(Some("b"), md.get(b_range));

        let second_line = structure.find_span_at(SpanKind::Paragraph, 2..2);
        assert_eq!(None, second_line);
    }

    #[test]
    pub fn test_print() {
        let md = "- a\n\t- b\n- c";

        let structure = TextStructure::create_from(md);

        let (a_range, _) = structure.find_span_at(SpanKind::Paragraph, 0..0).unwrap();
        let (b_range, _) = structure.find_span_at(SpanKind::Paragraph, 3..3).unwrap();

        println!("{:#?}", structure.spans);
        assert_eq!(Some("a"), md.get(a_range));
        assert_eq!(Some("b"), md.get(b_range));

        let second_line = structure.find_span_at(SpanKind::Paragraph, 2..2);
        assert_eq!(None, second_line);
    }
}

fn is_sub_range(outer: &Range<usize>, inner: &Range<usize>) -> bool {
    outer.start <= inner.start && outer.end >= inner.end
}
