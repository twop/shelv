use hyped::*;
use tailwind_fuse::*;

use crate::footer::footer_section;
use crate::ui_components::{ThemeColor, WaveDirection, content, space, theme, wave};
use crate::{
    BackgroundColor, BorderStyle, ButtonHeight, ButtonStyle, ButtonVariant, HoverState, IconSize,
    LinkStyle, SpacingSize, StyledText, TextColor, TextStyle,
};

// Constants for media assets
const IMG_HERO_PATH: &str = "assets/media/hero-1132x1376.png";
const SIZE_IMG_HERO: (usize, usize) = (1132, 1376);

const SIZE_VID_HACK_SETTINGS: (usize, usize) = (1126, 1244);
const VID_HACK_SETTINGS_PATH: &str = "assets/media/hack_settings_1126x1244.mov";

const IMG_MARKDOWN_PATH: &str = "assets/media/markdown_and_slash_palette_1132x1376.png";
const SIZE_IMG_MARKDOWN: (usize, usize) = (1132, 1376);

#[derive(Clone)]
enum MainSide {
    Left,
    Right,
}

enum VideoFileType {
    Mov,
    #[allow(dead_code)]
    Webm,
}

/// Strip out newlines from text for cleaner inline formatting
fn strip_out_newlines(text: &str) -> String {
    text.lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Main home page layout
pub fn home_page() -> Element {
    div((
        // First section with hero content
        theme(
            ThemeColor::Dark,
            content((
                page_header(),
                space(SpacingSize::Small),
                block_layout(
                    slogan_and_mac_store_link(),
                    {
                        let (w, h) = SIZE_IMG_HERO;
                        img_component(
                            IMG_HERO_PATH,
                            "Shelv app showing AI-powered quick prompt feature in action",
                            w,
                            h,
                            true,
                        )
                    },
                    MainSide::Left,
                ),
            )),
        ),
        space(SpacingSize::Small),
        wave(WaveDirection::Up, ThemeColor::Dark, SpacingSize::Large),
        // Second section with markdown features
        theme(
            ThemeColor::Light,
            content((
                space(SpacingSize::Small),
                block_layout(
                    {
                        let (w, h) = SIZE_VID_HACK_SETTINGS;
                        video_component(
                            [(VID_HACK_SETTINGS_PATH, VideoFileType::Mov)],
                            "Creating and using a custom 'day' command via shortcuts and slash menu",
                            w,
                            h,
                            false,
                        )
                    },
                    div((
                        block_header("Hack It, Make It Yours").id("features"),
                        p((
                            "Settings in Shelv is just a note, where you can create custom commands with ",
                            link_to("https://kdl.dev/", "KDL"),
                            " and JavaScript, assign and tweak keyboard shortcuts, all with live reload.",
                            br(),
                            br(),
                            "The origin story: at the time I used ",
                            link_to("https://bear.app/", "Bear"),
                            ", which had 4 different formats of dates, but I wanted YYYY/mmm/dd, ",
                            "and I kept thinking: \"if only I could define what I want\". Well, with Shelv you can.",
                        )),
                    ))
                    .class(&tw_join!(
                        TextColor::Subtle,
                        TextStyle::SmallGeneralText
                    )),
                    MainSide::Right,
                ),
                space(SpacingSize::Large),
                block_layout(
                    div((
                        block_header("Markdown essentials and more"),
                        space(SpacingSize::Small),
                        features_bullet_list(),
                    )),
                    {
                        let (w, h) = SIZE_IMG_MARKDOWN;
                        img_component(
                            IMG_MARKDOWN_PATH,
                            "Demo of markdown features and slash command",
                            w,
                            h,
                            false,
                        )
                    },
                    MainSide::Left,
                ),
                space(SpacingSize::Large),
            )),
        ),
        wave(
            WaveDirection::Down,
            ThemeColor::Light,
            SpacingSize::Medium,
        ),
        // FAQ section
        theme(
            ThemeColor::Dark,
            content((
                space(SpacingSize::Large),
                faq_section(),
                space(SpacingSize::Large),
            )),
        ),
        wave(WaveDirection::Up, ThemeColor::Dark, SpacingSize::Medium),
        // Roadmap section
        theme(
            ThemeColor::Light,
            content((
                space(SpacingSize::Large),
                roadmap_section(),
                space(SpacingSize::Large),
            )),
        ),
        // Footer section
        footer_section(),
    ))
    .class(&tw_join!(
        "flex flex-col",
        BackgroundColor::Default.as_class()
    ))
}

// Component functions

fn block_layout(left: Element, right: Element, main: MainSide) -> Element {
    let direction_class = match main {
        MainSide::Left => "flex-col",
        MainSide::Right => "flex-col-reverse",
    };

    div((div(left).class("md:flex-1"), div(right).class("md:flex-1"))).class(&tw_join!(
        "relative flex md:flex-row gap-8 md:gap-10 items-center",
        direction_class
    ))
}

fn img_component(src: &str, alt: &str, width: usize, height: usize, eager: bool) -> Element {
    div(div(img()
        .class("rounded-(--media-radius) w-full h-full")
        .attr("width", &width.to_string())
        .attr("height", &height.to_string())
        .attr("loading", if eager { "eager" } else { "lazy" })
        .attr("alt", alt)
        .attr("src", src))
    .class(tw_join!(
        BorderStyle::MediaBorder,
        "border-1 rounded-(--media-radius) shadow-underglow"
    )))
    .class("py-6 lg:py-0 w-full h-full flex justify-center")
}

fn video_component(
    sources: impl IntoIterator<Item = (&'static str, VideoFileType)>,
    _alt: &str,
    width: usize,
    height: usize,
    _eager: bool,
) -> Element {
    let sources = sources
        .into_iter()
        .map(|(source, file_format)| {
            let file_format = match file_format {
                VideoFileType::Mov => "video/mp4",
                VideoFileType::Webm => "video/webm",
            };

            format!("<source src=\"{source}\" type=\"{file_format}\">")
        })
        .fold("".to_string(), |total, source| {
            total + "\n" + source.as_str()
        });

    let border_class = BorderStyle::MediaBorder.as_class();
    div(div(danger(&format!(
        r#"<video class="rounded-(--media-radius) w-full h-full shadow-underglow border-1 {border_class}" width="{width}" height="{height}" autoplay muted loop playsinline>
            {sources}
            Your browser does not support the video tag.
        </video>"#
    ))))
    .class("py-6 lg:py-0 w-full h-full flex justify-center")
}

fn page_header() -> Element {
    div((
        div(
            a(shelv_logo())
                .href("/")
                .class("inline-flex items-center space-x-2 leading-6 font-medium transition ease-in-out duration-150"),
        ),
        // Desktop navigation - visible on md screens and up
        div(Vec::from([
            ("Features", "#features"),
            ("FAQ", "#faq"),
            ("Roadmap", "#roadmap"),
            ("Updates", "/updates"),
        ])
        .into_iter()
        .map(|(name, link_to)| {
            a(name).href(link_to).class(&tw_join!(
                TextStyle::NavMenu,
                LinkStyle {
                    color: TextColor::Subtle,
                    hover: HoverState::ColorChange
                }
                .to_class()
            ))
        })
        .collect::<Vec<_>>())
        .class("hidden md:flex gap-x-8"),
        // Discord icon - always visible
        div(a(discord_icon(IconSize::Default))
            .href(include_str!("../../distribution/discord_invite.txt").trim())
            .class(&tw_join!(
                ButtonVariant::SecondaryTextOnly,
                TextColor::Subtle
            ))),
    ))
    .class("flex justify-between items-center py-6")
}

fn shelv_logo() -> impl Render {
    let svg_content = include_str!("../assets/icons/shelv-logo.svg");
    danger(svg_content.replace("<class>", "shelv-logo"))
}

fn link_to(to: &str, text: &str) -> Element {
    let link_style = LinkStyle {
        color: TextColor::Primary,
        hover: HoverState::Underline,
    };
    a(text.to_string()).class(&link_style.to_class()).href(to)
}

fn block_header(text: &str) -> Element {
    let header_style = StyledText {
        color: TextColor::SubHeader,
        style: TextStyle::SubHeader,
    };
    h4(text.to_string()).class(&header_style.to_class())
}

fn slogan_and_mac_store_link() -> Element {
    let child = div((
        {
            let h1_style = StyledText {
                color: TextColor::MainHeader,
                style: TextStyle::MainHeader,
            };
            h1("Hackable, Local, AI-powered notes").class(&h1_style.to_class())
        },
        {
            let desc_style = StyledText {
                color: TextColor::Subtle,
                style: TextStyle::LargeGeneralText,
            };
            p((
                "Shelv is a scriptable, plain text notes app with integrated AI features for macOS, written in Rust ",
                span("by the way ™").class(&tw_join!(
                    TextStyle::SmallGeneralText,
                    TextColor::VerySubtle
                )),
            ))
            .class(&tw_join!(
                "mt-4 max-w-md mx-auto md:mt-5 md:max-w-3xl",
                desc_style.to_class()
            ))
        },
    ));
    div((
        // Slogan section
        div(child).class("text-center lg:text-left"),
        // Action buttons panel section
        div(action_buttons_panel())
            .class("mt-8 sm:max-w-lg sm:mx-auto text-center sm:text-center lg:text-left lg:mx-0"),
    ))
}

fn action_buttons_panel() -> Element {
    div((mac_store_link(), github_link()))
        .class("flex flex-col sm:flex-row gap-4 items-center justify-center lg:justify-start")
}

fn mac_store_link() -> Element {
    a(img()
        .attr("src", "/assets/media/mac-app-store-badge.svg")
        .attr("alt", "Download on the Mac App Store")
        .class("home-app-store-buttons-mac h-10")
        .attr("height", "48"))
    .href("https://apps.apple.com/us/app/shelv-notes/id6499478682")
}

fn secondary_button_link(href: &str, content: impl Render + 'static) -> Element {
    let button_style = ButtonStyle {
        height: ButtonHeight::FixedH10,
        variant: ButtonVariant::Secondary,
    };

    a(content).class(&button_style.to_class()).href(href)
}

fn github_link() -> Element {
    secondary_button_link(
        "https://github.com/twop/shelv",
        (
            github_icon(IconSize::Default),
            span("Give us a star").class("ml-2"),
        ),
    )
}

fn github_icon(size: IconSize) -> impl Render {
    let svg_content = include_str!("../assets/icons/github.svg");
    let classes = tw_join!(size, "fill-current inline");
    danger(svg_content.replace("<class>", &classes))
}

fn discord_icon(size: IconSize) -> impl Render {
    let svg_content = include_str!("../assets/icons/discord.svg");
    let classes = tw_join!(size, "fill-current inline");
    danger(svg_content.replace("<class>", &classes))
}

fn faq_section() -> Element {
    div((
        block_header("Frequently Asked Questions"),
        space(SpacingSize::Medium),
        div(faq_items()
            .into_iter()
            .map(|(question, answer)| faq_item(question, answer))
            .collect::<Vec<_>>())
        .class("flex flex-col gap-6"),
    ))
    .class("max-w-3xl mx-auto")
    .id("faq")
}

fn faq_items() -> Vec<(&'static str, Element)> {
    vec![
        (
            "I'm sick of AI hype, is Shelv yet another AI-'something'?",
            p((
                "I hope not, my opinions toward AI (or rather LLMs) are mixed. My current position can be roughly outlined as:",
                br(),
                br(),
                "- AI is not a \"higher-level abstraction\" like programming languages over assembly. For a simple reason: it is not deterministic.",
                br(),
                "- Using AI may and likely will cause skill degradation if used as a solo replacement for typing code or writing prose (like \"vibe coding\").",
                br(),
                "- I think the best use of it (at the moment) is if you can expertly assess the output. So the UX I'm leaning towards will try to emphasize that idea.",
                br(),
                "- Luckily, working with text, adding small scripts, etc. qualifies as such.",
                br(),
                "- But moreover, I think it can be used as a discovery tool - try asking with a quick prompt, \"What are the current keybindings?\"",
                br(),
                br(),
                "I hope I've convinced you to give Shelv a try."
            )).class(&tw_join!(TextStyle::SmallGeneralText, TextColor::Subtle))
        ),
        (
            "Is Shelv coming to Mobile/Window/Web?",
            p((
                "Yes, but with time (and perhaps your help). Shelv is written in Rust + ",
                link_to("https://egui.rs/", "egui"),
                ", which has some cross-platform support."
            )).class(&tw_join!(TextStyle::SmallGeneralText, TextColor::Subtle))
        ),
        (
            "How do you make money?",
            p((
                strip_out_newlines(r#"
                    I don't. I worked on Shelv for over 2 years, and I had a dream to start company (still do),
                    but as of now, it's a labor of love. In the future, there may be a way to purchase AI tokens.
                    Currently, Shelv uses my personal token with a cap, but you can always configure your own provider (including
                "#),
                " ",
                link_to("https://ollama.com/", "Ollama"),
                "."
            )).class(&tw_join!(TextStyle::SmallGeneralText, TextColor::Subtle))
        ),
        (
            "Does Shelv have sync functionality?",
            p((
                "Not yet, I'm a local first movement fan, and wanted to use ",
                link_to("https://github.com/automerge/automerge", "Automerge"),
                " ",
                strip_out_newlines(r#"
                    since forever, ideally with e2e encryption with a pure Rust server. This is something that's being worked on right now,
                    but e2e encrypted scalable sync ain't easy both technically and from a product perspective.
                "#)
            )).class(&tw_join!(TextStyle::SmallGeneralText, TextColor::Subtle))
        ),
        (
            "Is Shelv open source?",
            div((
                p((
                    "Shelv is licensed under the ",
                    link_to("https://github.com/twop/shelv/blob/main/LICENSE.md", "Komorebi License"),
                    " 2.0.0. The overall structure is inspired by the ",
                    link_to("https://github.com/LGUG2Z/komorebi", "komorebi project"),
                    "."
                )).class(&tw_join!(TextStyle::SmallGeneralText, TextColor::Subtle)),
                br(),
                p("Summary:").class(&tw_join!(TextStyle::SmallGeneralText, TextColor::Default, "font-semibold")),
                ul(vec![
                    "You can fork Shelv for the intended purpose of contributing features upstream or for non-commercial use only",
                    "You can use the Mac App Store version for personal use at work or home freely",
                    "In the future, there might be a team license available for purchase separately"
                ].into_iter().map(|item| {
                    li(item.to_string()).class(&tw_join!(TextStyle::SmallGeneralText, TextColor::Subtle))
                }).collect::<Vec<_>>()).class("list-disc list-inside space-y-1 mt-2")
            ))
        ),
        (
            "Is it Native?",
            p((
                "Native is a spectrum, Shelv is written in Rust using ",
                link_to("https://egui.rs/", "egui"),
                " as the gui toolkit, not Swift UI tech stack. Maybe the closest analogy would be ",
                link_to("https://flutter.dev/", "Flutter"),
                " that is painting every pixel. Are Flutter apps native? I think so."
            )).class(&tw_join!(TextStyle::SmallGeneralText, TextColor::Subtle))
        ),
        (
            "Are my beloved vim motions supported?",
            p((
                "I am a ",
                link_to("https://helix-editor.com/", "Helix"),
                " ",
                strip_out_newlines(r#"
                    user myself, but markdown and text are a bit different from code.
                    That said, I would love to support modal editing in the future. Perhaps some features
                    can be added for "insert" mode (which is the only mode at the moment) that can
                    enhance editing, for example: jump to a word, press any buttons with a label(vimium style),
                    expand + shrink semantic selection etc. I need to work on Shelv full-time to justify
                    adding vim or helix motions to egui TextEdit, vote with your money I guess,
                    oh wait, I don't have a way to actually recieve money...
                "#)
            )).class(&tw_join!(TextStyle::SmallGeneralText, TextColor::Subtle))
        ),
        (
            "Are you collecting any analytics?",
            p(strip_out_newlines(r#"
                Not at the moment (besides crash reporting), but I'm not fundamentally opposed
                to collecting statistics, because it is hard to know if a feature is even used
                without some observability. I do think it can be done with privacy in mind
                (at least anonymizing and being mindful of where the data is stored).
                Probably in the future, however, when and if I add monetization,
                I'll likely start collecting emails associated with a purchase and/or install
            "#)).class(&tw_join!(TextStyle::SmallGeneralText, TextColor::Subtle))
        )
    ]
}

fn faq_item(question: &str, answer: Element) -> Element {
    let button_style = ButtonStyle {
        height: ButtonHeight::ContentBased,
        variant: ButtonVariant::Secondary,
    };

    div((
        button(
            div((
                span(question.to_string()).class(tw_join!(
                    "text-left flex-1",
                    TextStyle::LargeGeneralText
                )),
                faq_chevron(),
            ))
            .class("flex items-center justify-between w-full"),
        )
        .class(&tw_join!("w-full text-left", button_style.to_class()))
        .attr("onclick", "this.nextElementSibling.classList.toggle('hidden'); this.querySelector('.faq-chevron').classList.toggle('rotate-180')"),
        div(answer).class("hidden p-3"),
    ))
}

fn faq_chevron() -> impl Render {
    danger(
        r#"<svg class="w-5 h-5 transition-transform duration-200 faq-chevron" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path>
    </svg>"#,
    )
}

fn roadmap_section() -> Element {
    div((
        block_header("Roadmap").id("roadmap"),
        space(SpacingSize::Medium),
        ol(roadmap_items()
            .into_iter()
            .map(|(date, completed, name, description)| {
                roadmap_item(date, completed, name, description)
            })
            .collect::<Vec<_>>())
        .class("relative border-s border-nord3"),
    ))
    .class("max-w-4xl mx-auto")
}

fn roadmap_items() -> Vec<(Option<&'static str>, bool, &'static str, Vec<&'static str>)> {
    vec![
        (
            Some("Aug 2025"),
            true,
            "Initial launch on macOS",
            vec![
                "Barebones editing with 4 notes",
                "Optimized for quick capture",
                "No Shelv API exposed to JS scripts",
            ],
        ),
        (
            None,
            false,
            "Multi-file + workspace support",
            vec![
                "Workspace folder with notes inside",
                "Import from Obsidian",
                "File tree + workspace viewer",
            ],
        ),
        (
            None,
            false,
            "Agentic mode",
            vec![
                "Tools/MCP that allow to search/move/create/edit notes",
                "UI for having agentic workflows, probably just a chat that is going to be just another file",
                "Files that define custom workflows, similar to Claude Code",
            ],
        ),
        (
            None,
            false,
            "Core editing features",
            vec![
                "Semantic selection: expand and shrink cursor selection with markdown AST nodes",
                "Jump to an element, jump to any word on the screen with a couple of keystrokes (similar to Vimium and Helix)",
                "Search, Redo etc",
            ],
        ),
        (None, false, "Support for pasting/rendering images", vec![]),
        (
            None,
            false,
            "Rich API exposed to JS + better scripting capabilities",
            vec!["Sharing code among notes"],
        ),
        (
            None,
            false,
            "Sync",
            vec![
                "I plan to use Automerge for personal syncing, which can be also used for collaboration",
                "Dump to git, e.g. backup all the notes to git, potentially with AI-generated change summary",
            ],
        ),
        (
            None,
            false,
            "Web version",
            vec!["Mobile (including web) version is TBD"],
        ),
        (
            None,
            false,
            "Collaboration",
            vec![
                "Share a note via link (co-editing on the web)",
                "Share workspace, that is, co-ownership of a collection of folder+notes",
            ],
        ),
    ]
}

fn roadmap_item(
    date: Option<&str>,
    completed: bool,
    name: &str,
    description: Vec<&str>,
) -> Element {
    li((
        // Timeline circle with icon
        span(roadmap_icon(completed))
            .class(&format!("absolute flex items-center justify-center w-6 h-6 {} rounded-full -start-3 ring-8 ring-nord0-dark",
                if completed { "bg-nord14" } else { "bg-nord3" })),

        // Content
        div((
            // Collapsible header with title, chevron, and date
            button(div((
                // Title with chevron right next to it
                h3((
                    span(name.to_string()).class(&tw_join!("font-semibold", TextStyle::LargeGeneralText.as_class(), if completed { TextColor::Default.as_class() } else { TextColor::Subtle.as_class() })),
                    roadmap_chevron()
                )).class("flex items-center gap-2 mb-1"),

                // Optional date
               if let Some(date_str) = date {
                    div(date_str.to_string())
                        .class(&tw_join!("block text-sm font-normal leading-none", TextColor::Subtle.as_class()))
                } else {
                    div("")
                }
            )).class("w-full"))
            .class("w-full text-left mb-3")
            .attr("onclick", "this.nextElementSibling.classList.toggle('hidden'); this.querySelector('.roadmap-chevron').classList.toggle('rotate-180')"),

            // Collapsible description
            div(ul(description.into_iter().map(|item| {
                li(item.to_string()).class(&tw_join!("mb-1", TextStyle::SmallGeneralText.as_class(), TextColor::Subtle.as_class()))
            }).collect::<Vec<_>>()).class("list-disc list-inside space-y-1")).class("hidden")
        )).class("-translate-y-2")
    )).class("mb-6 ms-6")
}

fn roadmap_chevron() -> Element {
    let button_style = ButtonStyle {
        height: ButtonHeight::FixedH10,
        variant: ButtonVariant::SecondaryTextOnly,
    };

    div(danger(r#"<svg class="w-5 h-5 transition-transform duration-200 roadmap-chevron" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path>
    </svg>"#)).class(&button_style.to_class())
}

fn roadmap_icon(completed: bool) -> impl Render {
    if completed {
        danger(
            r#"<svg class="w-3 h-3 text-white" fill="currentColor" viewBox="0 0 20 20">
            <path d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"/>
        </svg>"#,
        )
    } else {
        danger(
            r#"<svg class="w-3 h-3 text-nord4" fill="currentColor" viewBox="0 0 20 20">
            <path d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"/>
        </svg>"#,
        )
    }
}

fn features_bullet_list() -> Element {
    let features = vec![
        (
            "Markdown Support",
            "Full CommonMark with extensions, including TODOs",
        ),
        (
            "Code Syntax Highlighting",
            "A lot of languages are supported",
        ),
        (
            "Live JavaScript Blocks",
            "Execute JS code directly in notes",
        ),
        ("Slash Menu", "Quick access to all commands and features"),
        (
            "Keyboard optimized",
            "Everything is available via shortcuts",
        ),
    ];

    ul(features
        .into_iter()
        .map(|(title, description)| {
            li((
                span(title.to_string()).class(&tw_join!(
                    "font-bold",
                    TextColor::Default,
                    TextStyle::SmallGeneralText
                )),
                br(),
                span(description.to_string()).class(&tw_join!(
                    TextColor::Subtle,
                    TextStyle::SmallGeneralText,
                    "pl-4"
                )),
            ))
        })
        .collect::<Vec<_>>())
    .class("list-disc flex flex-col gap-4 list-inside pl-2")
}
