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

pub enum Ev {
    Strike,
    Bold,
    Emphasis,
    Text,
    Heading(HeadingLevel),
    Paragraph,
    CodeBlock { lang: String },
    Code,
    ListItem,
    TaskMarker(bool),
    ListStart(Option<u64>),
    ListEnd,
    RawLink { url: String },
    MdLink,
    Image,
    Html,
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
    Heading(HeadingLevel),
    CodeBlock,
    Code,
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
    RawLink,
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
    Link {
        url: String,
    },
}

#[derive(Debug, Clone)]
pub struct ListItemDesc {
    pub item_index: u32,
    pub depth: i32,
    pub starting_index: Option<u64>,
    pub item_byte_pos: Range<usize>,
}

impl ListItemDesc {
    pub fn is_numbered(&self) -> bool {
        self.starting_index.is_some()
    }
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

#[derive(Debug)]
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

                    let list_item_pos = trim_trailing_new_lines(&self.text, &pos);
                    self.add_with_meta(
                        SpanKind::ListItem,
                        list_item_pos.clone(),
                        SpanMeta::ListItem(ListItemDesc {
                            item_index,
                            depth,
                            starting_index,
                            item_byte_pos: list_item_pos,
                        }),
                    );
                }
            }

            Ev::TaskMarker(checked) => {
                // the last one to add would be the most nested, thus the one we need
                let list_item =
                    self.spans
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, (kind, list_item_range))| match kind {
                            SpanKind::ListItem => is_sub_range(list_item_range, &pos),
                            _ => false,
                        });

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

            Ev::ListStart(starting_index) => self.list_stack.push(ListDesc {
                starting_index,
                items_count: 0,
            }),

            Ev::ListEnd => {
                self.list_stack.pop();
            }

            Ev::Text => match self.last_code_block.clone() {
                // Text can be within a code block
                Some((block_span, block_pos, lang)) if is_sub_range(&block_pos, &pos) => {
                    // trim the last \n if any, it look odd,
                    // but \n is parsed as a part of the body, thus bandage that
                    let pos = trim_trailing_new_lines(&self.text, &pos);

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
                self.add_with_meta(SpanKind::RawLink, pos, SpanMeta::Link { url });
            }

            Ev::Heading(level) => {
                self.add(
                    SpanKind::Heading(level),
                    trim_trailing_new_lines(&self.text, &pos),
                );
            }
            Ev::Strike => {
                self.add(SpanKind::Strike, pos);
            }
            Ev::Bold => {
                self.add(SpanKind::Bold, pos);
            }
            Ev::Emphasis => {
                self.add(SpanKind::Emphasis, pos);
            }
            Ev::Paragraph => {
                self.add(
                    SpanKind::Paragraph,
                    trim_trailing_new_lines(&self.text, &pos),
                );
            }
            Ev::Code => {
                self.add(SpanKind::Code, pos);
            }
            Ev::MdLink => {
                self.add(SpanKind::MdLink, pos);
            }
            Ev::Image => {
                self.add(SpanKind::Image, pos);
            }
            Ev::Html => {
                self.add(SpanKind::Html, pos);
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

    pub fn finish(self) -> TextStructure {
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

    fn print_structure(&self) {
        println!("\n\n-----parser-----");
        println!("text: {:?}", self.text);
        println!("-----text-end-----\n");

        for (ev, range) in self.spans.iter() {
            // if let pulldown_cmark::Event::End(_) = &ev {
            //     depth -= 1;
            // }

            println!(
                "{:?}: [{}-{}]-> {:?}",
                ev,
                range.start,
                range.end,
                &self.text[range.start..range.end]
            );

            // if let pulldown_cmark::Event::Start(_) = &ev {
            //     depth += 1;
            // }
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
            builder.event(
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

        for (ev, range) in parser.into_offset_iter() {
            use pulldown_cmark::Event::*;
            use pulldown_cmark::Tag::*;
            match ev {
                Start(tag) => match tag {
                    Strong => builder.event(Ev::Bold, range),
                    Emphasis => builder.event(Ev::Emphasis, range),
                    Strikethrough => builder.event(Ev::Strike, range),
                    CodeBlock(CodeBlockKind::Fenced(lang)) => builder.event(
                        Ev::CodeBlock {
                            lang: lang.as_ref().to_string(),
                        },
                        range,
                    ),

                    Item => builder.event(Ev::ListItem, range),
                    Heading(level, _, _) => builder.event(Ev::Heading(level), range),
                    List(starting_index) => builder.event(Ev::ListStart(starting_index), range),
                    Paragraph => builder.event(Ev::Paragraph, range),
                    Link(_, _, _) => builder.event(Ev::MdLink, range),
                    Image(_, _, _) => builder.event(Ev::Image, range),

                    // We explicitly don't support these containers
                    Table(_)
                    | TableHead
                    | CodeBlock(CodeBlockKind::Indented)
                    | TableRow
                    | TableCell
                    | FootnoteDefinition(_)
                    | BlockQuote => (),
                },
                // we care only about list ending for now due to working with shortcuts
                End(List(_)) => builder.event(Ev::ListEnd, range),

                Text(_) => builder.event(Ev::Text, range),

                TaskListMarker(checked) => {
                    builder.event(Ev::TaskMarker(checked), range.clone());
                }
                Code(_) => builder.event(Ev::Code, range),
                Html(_) => builder.event(Ev::Html, range),

                End(_) | FootnoteReference(_) | SoftBreak | HardBreak | Rule => (),
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
                    Annotation::Heading(level) => {
                        state.heading[*level as usize] += delta;
                    }
                    Annotation::CodeBlock => (),
                    Annotation::Code => todo!(),
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
            .find_map(|(index, (kind, part_pos))| match kind {
                SpanKind::TaskMarker | SpanKind::RawLink if part_pos.contains(&byte_cursor_pos) => {
                    find_metadata(SpanIndex(index), &self.metadata).and_then(|meta| {
                        match (kind, meta) {
                            (SpanKind::TaskMarker, SpanMeta::TaskMarker { checked, .. }) => {
                                Some(InteractiveTextPart::TaskMarker {
                                    byte_range: part_pos.clone(),
                                    checked: *checked,
                                })
                            }
                            (SpanKind::RawLink, SpanMeta::Link { url }) => {
                                Some(InteractiveTextPart::Link(url.as_str()))
                            }
                            _ => None,
                        }
                    })
                }
                _ => None,
            })
    }

    pub fn find_surrounding_list_item(&self, byte_cursor: Range<usize>) -> Option<ListItemDesc> {
        self.spans
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, (kind, item_pos))| match kind {
                SpanKind::ListItem if is_sub_range(item_pos, &byte_cursor) => {
                    Some(SpanIndex(index))
                }
                _ => None,
            })
            .and_then(|index| find_metadata(index, &self.metadata))
            .and_then(|meta| match meta {
                SpanMeta::ListItem(desc) => Some(desc.clone()),
                _ => None,
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
            .find_map(|(i, (kind, item_pos))| {
                if *kind == span_kind && is_sub_range(item_pos, &byte_cursor) {
                    Some((item_pos.clone(), SpanIndex(i)))
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
            .find_map(|(i, (_, item_pos))| {
                if is_sub_range(item_pos, &byte_cursor) {
                    Some((item_pos.clone(), SpanIndex(i)))
                } else {
                    None
                }
            })
    }

    pub fn get_span_inner_content(&self, idx: SpanIndex) -> Range<usize> {
        let SpanIndex(index) = idx;
        let (kind, pos) = self.spans[index].clone();
        match kind {
            SpanKind::Strike => pos.start + 2..pos.end - 2, //~~{}~~
            SpanKind::Bold => pos.start + 2..pos.end - 2,   //**{}**
            SpanKind::Emphasis | SpanKind::Code => pos.start + 1..pos.end - 1, //*{}* or `{}`

            // these can be considered as atomic, thus return itself
            SpanKind::Text
            | SpanKind::TaskMarker
            | SpanKind::RawLink
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
                .take_while(|(_, item_pos)| item_pos.end <= pos.end)
                .fold(None, |area, (kind, item_pos)| match area {
                    None => Some(item_pos.clone()),
                    Some(area) => Some(area.start.min(item_pos.start)..area.end.max(item_pos.end)),
                })
                .unwrap_or(pos.start..pos.start),
        }
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
            SpanKind::RawLink => smallvec![(Annotation::Link, pos)],
            SpanKind::Heading(level) => smallvec![(Annotation::Heading(*level), pos)],
            SpanKind::CodeBlockContent => smallvec![(Annotation::CodeBlock, pos)],
            SpanKind::Code => smallvec![(Annotation::Code, pos)],

            SpanKind::MdLink
            | SpanKind::CodeBlock
            | SpanKind::List
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
}

fn is_sub_range(outer: &Range<usize>, inner: &Range<usize>) -> bool {
    outer.start <= inner.start && outer.end >= inner.end
}
