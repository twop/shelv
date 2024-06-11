use std::ops::{Deref, Range};

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

use crate::{
    byte_span::ByteSpan,
    scripting::OUTPUT_LANG,
    theme::{AppTheme, ColorTheme, FontTheme},
};

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
    // CodeBlockBody,
    InlineCode,
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
    List,
    InlineCode,
    Html,
    ListItem,
    Image,
    Root,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanMeta {
    CodeBlock { lang: String },
    TaskMarker { checked: bool },
    List(ListDesc),
    Link { url: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListDesc {
    pub starting_index: Option<u64>,
    // items_count: u32,
}

struct MarkdownRunningState {
    // nesting: i8,
    bold: i8,
    strike: i8,
    emphasis: i8,
    text: i8,
    link: i8,
    task_marker: i8,
    code: i8,
    code_block: i8,
    heading: [i8; 6],
}

impl MarkdownRunningState {
    fn new() -> Self {
        Self {
            // nesting: 0,
            bold: 0,
            strike: 0,
            code: 0,
            code_block: 0,
            emphasis: 0,
            heading: Default::default(),
            text: 0,
            link: 0,
            task_marker: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SpanDesc {
    pub kind: SpanKind,
    pub byte_pos: ByteSpan,
    pub parent: SpanIndex,
}

#[derive(Debug)]
pub struct TextStructure {
    points: Vec<AnnotationPoint>,
    raw_links: Vec<RawLink>,
    spans: Vec<SpanDesc>,
    metadata: Vec<(SpanIndex, SpanMeta)>,
    generation: u64,
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
    TaskMarker { byte_range: ByteSpan, checked: bool },
    Link(&'a str),
}

impl<'a> TextStructureBuilder<'a> {
    fn start(
        text: &'a str,
        recycled: (Vec<SpanDesc>, Vec<RawLink>, Vec<(SpanIndex, SpanMeta)>),
    ) -> Self {
        let (mut spans, mut raw_links, mut metadata) = recycled;
        spans.clear();
        spans.push(SpanDesc {
            kind: SpanKind::Root,
            byte_pos: ByteSpan::new(0, 0),
            parent: SpanIndex(0),
        });
        raw_links.clear();
        metadata.clear();

        Self {
            text,
            spans,
            metadata,
            container_stack: smallvec![SpanIndex(0)],
            raw_links,
        }
    }

    fn add(&mut self, kind: SpanKind, pos: ByteSpan) -> SpanIndex {
        let index = SpanIndex(self.spans.len());
        self.spans.push(SpanDesc {
            kind,
            byte_pos: pos,
            parent: self.container_stack.last().unwrap().clone(),
        });
        index
    }

    fn add_with_meta(&mut self, kind: SpanKind, pos: ByteSpan, meta: SpanMeta) -> SpanIndex {
        let index = self.add(kind, pos);
        self.metadata.push((index, meta));
        index
    }

    fn finish(self, mut annotation_points: Vec<AnnotationPoint>, generation: u64) -> TextStructure {
        let Self {
            spans,
            metadata,
            raw_links,
            ..
        } = self;

        annotation_points.clear();

        let points = fill_annotation_points(annotation_points, &spans, &metadata, &raw_links);

        TextStructure {
            points,
            spans,
            metadata,
            raw_links,
            generation,
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

fn trim_trailing_new_lines(text: &str, pos: ByteSpan) -> ByteSpan {
    let (start, mut end) = (pos.start, pos.end);

    // TODO optimize it by taking a slice from the string
    while start < end && text[start..end].ends_with("\n") {
        end -= 1;
    }

    ByteSpan::new(start, end)
}

impl TextStructure {
    pub fn new(text: &str) -> Self {
        let struture = Self {
            points: vec![],
            raw_links: vec![],
            spans: vec![],
            metadata: vec![],
            generation: 0,
        };
        struture.recycle(text)
    }

    pub fn opaque_version(&self) -> u64 {
        self.generation
    }

    pub fn recycle(self, text: &str) -> Self {
        let Self {
            points,
            raw_links,
            spans,
            metadata,
            generation,
        } = self;

        let mut builder = TextStructureBuilder::start(text, (spans, raw_links, metadata));
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
            let range = ByteSpan::from_range(&range);
            match ev {
                Start(tag) => {
                    use pulldown_cmark::Tag::*;
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

                        CodeBlock(CodeBlockKind::Indented) => Some(builder.add_with_meta(
                            SpanKind::CodeBlock,
                            range.clone(),
                            SpanMeta::CodeBlock {
                                lang: "".to_string(),
                            },
                        )),

                        Item => Some(
                            builder.add(SpanKind::ListItem, trim_trailing_new_lines(&text, range)),
                        ),
                        Heading { level, .. } => Some(builder.add(
                            SpanKind::Heading(level),
                            trim_trailing_new_lines(&text, range),
                        )),
                        List(starting_index) => Some(builder.add_with_meta(
                            SpanKind::List,
                            range,
                            SpanMeta::List(ListDesc { starting_index }),
                        )),
                        Paragraph => Some(
                            builder.add(SpanKind::Paragraph, trim_trailing_new_lines(&text, range)),
                        ),
                        Link { dest_url, .. } => Some(builder.add_with_meta(
                            SpanKind::MdLink,
                            range,
                            SpanMeta::Link {
                                url: dest_url.to_string(),
                            },
                        )),
                        Image { .. } => Some(builder.add(SpanKind::Image, range)),

                        // We explicitly don't support these containers
                        Table(_)
                        | TableHead
                        | TableRow
                        | TableCell
                        | FootnoteDefinition(_)
                        | HtmlBlock
                        | MetadataBlock(_)
                        | BlockQuote(_) => None,
                    };

                    if let Some(container_index) = container {
                        builder.container_stack.push(container_index);
                    }
                }

                End(tag) => {
                    use pulldown_cmark::TagEnd as T;
                    let is_supported_container = match tag {
                        // We explicitly don't support these containers
                        // note that it needs to match "Start" variant
                        T::Table
                        | T::TableHead
                        | T::TableRow
                        | T::TableCell
                        | T::FootnoteDefinition
                        | T::HtmlBlock
                        | T::MetadataBlock(_)
                        | T::BlockQuote => false,

                        // supported containers. Note that it needs to match "Start" variant
                        T::Paragraph
                        | T::CodeBlock
                        | T::Heading { .. }
                        | T::List(_)
                        | T::Item
                        | T::Emphasis
                        | T::Strong
                        | T::Strikethrough
                        | T::Link { .. }
                        | T::Image => true,
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
                    builder.add(SpanKind::InlineCode, range);
                }

                Html(_) => {
                    builder.add(SpanKind::Html, range);
                }

                FootnoteReference(_) | DisplayMath(_) | InlineHtml(_) | InlineMath(_)
                | SoftBreak | HardBreak | Rule => (),
            }
        }

        builder.print_structure();

        builder.finish(points, generation.wrapping_add(1))
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
            if state.code_block > 0 && state.text > 0 {
                // means that we are inside code block body

                let code = text.get(pos..point.str_offset).unwrap_or("");

                let lang = match self.find_surrounding_span_with_meta(
                    SpanKind::CodeBlock,
                    ByteSpan::new(pos, point.str_offset),
                ) {
                    Some((_, _, SpanMeta::CodeBlock { lang })) => lang.to_string(),
                    _ => "".to_string(),
                };

                let lang = match lang.as_str() {
                    // "ts" => "typescript",
                    // "rs" => "rust",
                    l if l.starts_with(OUTPUT_LANG) => "js",
                    l => l,
                };

                match syntax_set.find_syntax_by_extension(lang) {
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
            }

            let delta = match point.kind {
                PointKind::Start => 1,
                PointKind::End => -1,
            };

            // TODO rework this a bit
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
                Annotation::InlineCode => state.code += delta,
                Annotation::CodeBlock => state.code_block += delta,
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
                        if byte_pos.contains_pos(byte_cursor_pos) =>
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
        byte_cursor: ByteSpan,
    ) -> Option<(SpanIndex, SpanDesc, SpanMeta)> {
        self.find_span_at(kind, byte_cursor).and_then(|(_, index)| {
            let span = &self.spans[index.0];
            find_metadata(index, &self.metadata).map(|meta| (index, span.clone(), meta.clone()))
        })
    }

    pub fn find_span_at(
        &self,
        span_kind: SpanKind,
        byte_cursor: ByteSpan,
    ) -> Option<(ByteSpan, SpanIndex)> {
        self.spans
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, SpanDesc { kind, byte_pos, .. })| {
                if *kind == span_kind && byte_pos.contains(byte_cursor) {
                    Some((byte_pos.clone(), SpanIndex(i)))
                } else {
                    None
                }
            })
    }

    pub fn iterate_parents_of(
        &self,
        index: SpanIndex,
    ) -> impl Iterator<Item = (SpanIndex, &SpanDesc)> {
        struct ParentIterator<'a> {
            spans: &'a [SpanDesc],
            cur: SpanIndex,
        }

        impl<'a> Iterator for ParentIterator<'a> {
            type Item = (SpanIndex, &'a SpanDesc);

            fn next(&mut self) -> Option<Self::Item> {
                let parent_index = self.spans[self.cur.0].parent;
                let parent = &self.spans[parent_index.0];
                if parent.kind == SpanKind::Root {
                    None
                } else {
                    self.cur = parent_index;
                    Some((parent_index, parent))
                }
            }
        }

        ParentIterator {
            spans: &self.spans,
            cur: index,
        }
    }

    pub fn iterate_immediate_children_of(
        &self,
        parent: SpanIndex,
    ) -> impl Iterator<Item = (SpanIndex, &SpanDesc)> {
        iterate_immediate_children_of(parent, &self.spans)
    }

    pub fn iterate_children_recursively_of(
        &self,
        parent: SpanIndex,
    ) -> impl Iterator<Item = (SpanIndex, &SpanDesc)> {
        iterate_children_recursively_of(parent, &self.spans)
    }

    pub fn find_meta(&self, index: SpanIndex) -> Option<&SpanMeta> {
        find_metadata(index, &self.metadata)
    }

    pub fn iter(&self) -> impl Iterator<Item = (SpanIndex, &SpanDesc)> {
        // note that the first el is root itself
        self.spans
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, desc)| (SpanIndex(i), desc))
    }

    pub fn find_any_span_at(
        &self,
        byte_cursor: ByteSpan,
    ) -> Option<(ByteSpan, SpanKind, SpanIndex)> {
        self.spans
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, SpanDesc { byte_pos, kind, .. })| {
                if byte_pos.contains(byte_cursor) && *kind != SpanKind::Root {
                    Some((*byte_pos, *kind, SpanIndex(i)))
                } else {
                    None
                }
            })
    }

    pub fn get_span_inner_content(&self, idx: SpanIndex) -> ByteSpan {
        let SpanIndex(index) = idx;
        let SpanDesc {
            kind,
            byte_pos: pos,
            ..
        } = self.spans[index].clone();
        let range = match kind {
            SpanKind::Strike => pos.start + 2..pos.end - 2, //~~{}~~
            SpanKind::Bold => pos.start + 2..pos.end - 2,   //**{}**
            SpanKind::Emphasis | SpanKind::InlineCode => pos.start + 1..pos.end - 1, //*{}* or `{}`

            // TODO what to do with Root?
            SpanKind::Root => 0..0,

            // these can be considered as atomic, thus return itself
            SpanKind::Text
            | SpanKind::TaskMarker
            | SpanKind::MdLink
            | SpanKind::Html
            | SpanKind::Image => pos.range(),

            // these are all containers, thus we need to enumerate their content
            SpanKind::ListItem
            | SpanKind::CodeBlock
            | SpanKind::Heading(_)
            | SpanKind::Paragraph
            | SpanKind::List =>
            // self
                // .spans
                // .iter()
                // .skip(index + 1)
                // .take_while(|desc| desc.byte_pos.end <= pos.end)
                // .fold(None, |area, desc| match area {
                //     None => Some(desc.byte_pos.clone()),
                //     Some(area) => {
                //         Some(area.start.min(desc.byte_pos.start)..area.end.max(desc.byte_pos.end))
                //     }
                calc_total_range( iterate_immediate_children_of(idx, &self.spans).map(|(_, desc)| &desc.byte_pos)).map(|byte_span| byte_span.range())
                .unwrap_or( pos.start..pos.start),
        };

        ByteSpan::from_range(&range)
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
        let annotations: SmallVec<[(Annotation, ByteSpan); 2]> = match kind {
            SpanKind::Strike => smallvec![(Annotation::Strike, pos)],
            SpanKind::Bold => smallvec![(Annotation::Bold, pos)],
            SpanKind::Emphasis => smallvec![(Annotation::Emphasis, pos)],
            SpanKind::Text => smallvec![(Annotation::Text, pos)],
            SpanKind::TaskMarker => match find_metadata(span_index, metadata) {
                Some(SpanMeta::TaskMarker { checked }) => match *checked {
                    true => {
                        let list_item_content = calc_total_range(
                            iterate_immediate_children_of(*parent, spans)
                                .map(|(_, desc)| &desc.byte_pos),
                        )
                        .unwrap_or(pos);
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
            SpanKind::InlineCode => smallvec![(Annotation::InlineCode, pos)],
            SpanKind::CodeBlock => smallvec![(Annotation::CodeBlock, pos)],
            // SpanKind::CodeBlock => match find_metadata(span_index, metadata) {
            //     Some(SpanMeta::CodeBlock { lang }) => match *checked {
            //         true => {
            //             let list_item_content =
            //                 calc_total_range(iterate_children_of(*parent, spans))
            //                     .unwrap_or(pos.clone());
            //             smallvec![
            //                 (Annotation::TaskMarker, pos),
            //                 (Annotation::Strike, list_item_content)
            //             ]
            //         }
            //         false => smallvec![(Annotation::TaskMarker, pos)],
            //     },
            //     _ => smallvec![(Annotation::CodeBlock, pos)],
            // },
            SpanKind::MdLink
            | SpanKind::List
            | SpanKind::Root
            | SpanKind::Html
            | SpanKind::Image
            | SpanKind::ListItem
            | SpanKind::Paragraph => smallvec![],
        };

        for (annotation, pos) in annotations {
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
            kind,
            annotation: Annotation::Link,
        });
    }

    points.sort_by_key(|p| p.str_offset);
    points
}

#[inline(always)]
fn iterate_immediate_children_of(
    index: SpanIndex,
    spans: &Vec<SpanDesc>,
) -> impl Iterator<Item = (SpanIndex, &SpanDesc)> {
    iterate_children_recursively_of(index, spans).filter(move |(_, child)| child.parent == index)
}

#[inline(always)]
fn iterate_children_recursively_of(
    index: SpanIndex,
    spans: &Vec<SpanDesc>,
) -> impl Iterator<Item = (SpanIndex, &SpanDesc)> {
    // note that we rely here on ordering
    // hence we need to stop iterating when the parent pos of an element
    // is either the parent of the targeted item (means siblings)
    // or even earlier parent (e.g. even closer to the root)
    let parent_parent = spans[index.0].parent.0;
    spans
        .iter()
        .enumerate()
        .skip(index.0 + 1)
        .map(|(i, desc)| (SpanIndex(i), desc))
        .take_while(move |(_, child)| child.parent.0 > parent_parent)
}

fn calc_total_range<'a>(spans: impl Iterator<Item = &'a ByteSpan>) -> Option<ByteSpan> {
    spans.fold(None, |range, byte_pos| match range {
        Some(range) => Some(ByteSpan::new(
            range.start.min(byte_pos.start),
            range.end.max(byte_pos.end),
        )),
        None => Some(*byte_pos),
    })
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
    use crate::byte_span::RangeRelation;

    use super::*;
    #[test]
    pub fn test_inner_content() {
        let md = "## ti**tle** \n";

        let structure = TextStructure::new(md);

        let (h2_range, idx) = structure
            .find_span_at(SpanKind::Heading(HeadingLevel::H2), ByteSpan::new(4, 6))
            .unwrap();

        println!("{:#?}", structure.spans);
        assert_eq!(Some("## ti**tle** "), md.get(h2_range.range()));

        let content_range = structure.get_span_inner_content(idx);

        // note that it skips the first space due to h2 pattern
        // and doesn't capture the trailing space, and neither "\n"
        assert_eq!(Some("ti**tle**"), md.get(content_range.range()));
    }

    #[test]
    pub fn test_span_detection() {
        let md = "a\n\nb";

        let structure = TextStructure::new(md);

        let (a_range, _) = structure
            .find_span_at(SpanKind::Paragraph, ByteSpan::new(0, 0))
            .unwrap();
        let (b_range, _) = structure
            .find_span_at(SpanKind::Paragraph, ByteSpan::new(3, 3))
            .unwrap();

        println!("{:#?}", structure.spans);
        assert_eq!(Some("a"), md.get(a_range.range()));
        assert_eq!(Some("b"), md.get(b_range.range()));

        let second_line = structure.find_span_at(SpanKind::Paragraph, ByteSpan::new(2, 2));
        assert_eq!(None, second_line);
    }

    #[test]
    pub fn test_code_block_parsing() {
        let md = "```js\ncode\n```";

        let structure = TextStructure::new(md);

        // println!("{:#?}", structure.spans);

        let res = structure.find_span_at(SpanKind::CodeBlock, ByteSpan::new(0, 0));
        assert!(res.is_some());

        let (_, _, meta) = structure
            .find_surrounding_span_with_meta(SpanKind::CodeBlock, ByteSpan::new(0, 0))
            .unwrap();

        let (code_body, _) = structure
            .find_span_at(SpanKind::Text, ByteSpan::new(7, 7))
            .unwrap();

        assert_eq!(Some("code\n"), md.get(code_body.range()));

        assert_eq!(
            SpanMeta::CodeBlock {
                lang: "js".to_string()
            },
            meta
        );
    }

    #[test]
    pub fn test_byte_range_relation() {
        let test_cases = [
            (0..0, 0..0, RangeRelation::Equal),
            (0..1, 1..3, RangeRelation::Before),
            (2..4, 1..2, RangeRelation::After),
            (0..3, 1..2, RangeRelation::Contains),
            (0..3, 1..5, RangeRelation::EndInside),
            (2..4, 0..3, RangeRelation::StartInside),
            (1..2, 0..3, RangeRelation::Inside),
        ];

        for (a, b, expected) in test_cases {
            assert_eq!(
                ByteSpan::from_range(&a.clone()).relative_to(ByteSpan::from_range(&b.clone())),
                expected,
                "wrong relation {a:?} to {b:?}"
            );
        }
    }
}
