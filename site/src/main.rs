use axum::response::Html;
use enum_router::router;
use hyped::*;
use std::net::SocketAddr;
use tailwind_fuse::*;
use tower_http::services::ServeDir;

// Constants from original dioxus site
const UP_WAVE_PATH: &str = "M0,128L120,144C240,160,480,192,720,208C960,224,1200,224,1320,224L1440,224L1440,320L1320,320C1200,320,960,320,720,320C480,320,240,320,120,320L0,320Z";
const DOWN_WAVE_PATH: &str = "M0,224L80,186.7C160,149,320,75,480,53.3C640,32,800,64,960,85.3C1120,107,1280,117,1360,122.7L1440,128L1440,320L1360,320C1280,320,1120,320,960,320C800,320,640,320,480,320C320,320,160,320,80,320L0,320Z";
const IMG_W: usize = 1180;
const IMG_H: usize = 1128;

// Semantic color variants using tailwind_fuse
#[derive(TwVariant)]
pub enum TextColor {
    #[tw(default, class = "text-nord5")]
    Default,

    #[tw(class = "text-nord4-darker")]
    Subtle,

    #[tw(class = "text-nord8")]
    Primary,

    #[tw(class = "text-nord6")]
    SubHeader,

    #[tw(class = "text-nord6")]
    MainHeader,

    #[tw(class = "text-nord11")]
    Red,
}

#[derive(TwVariant)]
pub enum BackgroundColor {
    #[tw(default, class = "bg-nord0-darker")]
    Default,
    #[tw(class = "bg-nord0-darker")]
    Dark,
    #[tw(class = "bg-nord0-dark")]
    Light,
    #[tw(class = "bg-transparent")]
    Transparent,
}

#[derive(TwVariant)]
pub enum BorderStyle {
    #[tw(default, class = "border-nord3")]
    Default,
    #[tw(class = "border-nord3")]
    LineBreak,
}

// Theme management enums
#[derive(Clone)]
pub enum ThemeColor {
    Dark,
    Light,
}

#[derive(Clone)]
enum MainSide {
    Left,
    Right,
}

// Style structs for different component types
#[derive(TwClass)]
#[tw(class = "")]
pub struct StyledText {
    color: TextColor,
    style: TextStyle,
}

#[derive(TwVariant)]
pub enum TextStyle {
    #[tw(class = "text-4xl leading-tight font-semibold sm:text-5xl sm:leading-none")]
    MainHeader,
    
    #[tw(default, class = "text-2xl mb-4 leading-8 font-semibold sm:text-3xl sm:leading-9")]
    SubHeader,
    
    #[tw(class = "text-base sm:text-lg leading-7 sm:leading-8")]
    Paragraph,
    
    #[tw(class = "text-sm leading-6 sm:text-sm sm:leading-7")]
    Footnote,
}

#[derive(TwClass)]
#[tw(class = "")]
pub struct ThemeStyle {
    bg: BackgroundColor,
    text: TextColor,
}

#[derive(TwClass)]
#[tw(class = "")]
pub struct LinkStyle {
    color: TextColor,
    hover: HoverState,
}

#[derive(TwVariant)]
pub enum HoverState {
    #[tw(default, class = "hover:underline")]
    Underline,
    #[tw(class = "hover:text-nord10")]
    ColorChange,
}

#[derive(TwVariant)]
pub enum SpacingSize {
    #[tw(class = "w-full h-4 sm:h-8")]
    Small,
    
    #[tw(default, class = "w-full h-8 sm:h-16")]
    Medium,
   
    #[tw(class = "w-full h-16 sm:h-32")]
    Large,
}

#[derive(TwClass)]
#[tw(class = "border-solid border-t-1 w-full")]
pub struct DividerStyle {
    color: BorderStyle,
}

#[derive(TwVariant)]
enum ButtonVariant {
    #[tw(default,class = r#"
        border-1 border-nord4-darker hover:border-nord7 active:border-nord8
        text-nord4 hover:text-nord7 active:text-nord8"#)]
    Secondary,
    
    #[tw(class = r#"
        text-nord4 hover:text-nord7 active:text-nord8"#)]
    SecondaryTextOnly
}

#[derive(TwVariant)]
enum ButtonHeight{
    #[tw(default,class = r#" h-10"#)]
    FixedH10,

    #[tw(class = r#"py-3"#)]
    ContentBased
}


#[derive(TwClass)]
#[tw(class = r#"
    inline-flex items-center
    font-medium text-center no-underline align-middle whitespace-nowrap
    rounded-lg select-none px-3 transition-all duration-150"#)]
pub struct ButtonStyle {
    variant: ButtonVariant,
    height: ButtonHeight
}

// Enum router definition
#[router]
pub enum Route {
    #[get("/")]
    Root,
}

// Route handlers
async fn root() -> Html<String> {
    Html(render_to_string(home_page()))
}

// Main HomePage component (converted from dioxus)
fn home_page() -> Element {
    div((
        // First section with hero content
        theme(ThemeColor::Dark, content((
                page_header(),
                space(SpacingSize::Small),
                block_layout(
                    slogan_and_mac_store_link(),
                    // Screenshot: Prompt, code block, Markdown, TBD the exact content
            img_component("screenshot-ai-prompt", "Shelv app showing AI-powered quick prompt feature in action", IMG_W, IMG_H, true),
                    MainSide::Left
                )
            ))),
        
        space(SpacingSize::Small),

        // Wave separator
        wave(UP_WAVE_PATH.to_string(), ThemeColor::Dark, SpacingSize::Large),
        

        // Second section with markdown features
        theme(ThemeColor::Light, content((
                space(SpacingSize::Small),
                block_layout(
                    // Animated GIF: Demo showing:
                    // 1. Quick prompt to create a "day" insert feature
                    // 2. Triggering the new feature via keyboard shortcut  
                    // 3. Using the same feature via slash menu
                    // Alt Text: "Creating and using a custom 'day' command via shortcuts and slash menu"
                    // TODO: Record this demo GIF
                    img_component("screenshot-custom-commands", "Creating and using custom commands via shortcuts and slash menu", IMG_W, IMG_H, false),
                    div((
                        block_header("Hack It, Make It Yours"),
                        p((
                            "Settings in Shelv is just a note, where you can create custom commands with ",
                            link_to("https://kdl.dev/", "KDL"),
                            " and JavaScript, assign and tweak keyboard shortcuts, all with live reload."
                        )).class(&tw_join!(TextColor::Subtle, TextStyle::Footnote))
                    )),
                    MainSide::Right
                ),
                space(SpacingSize::Large),
                block_layout(
                    div((
                        block_header("Markdown essentials and more"),
                        space(SpacingSize::Small),
                        features_bullet_list()
                    )),
                    // Animated GIF: Demo showing:
                    // 1. Creating a live JavaScript block via slash menu
                    // 2. Writing and executing JavaScript code
                    // 3. Quick prompt to convert bullet list to numbered list
                    // Alt Text: "Creating live JavaScript code and converting list formats with AI"
                    // TODO: Record this demo GIF
                    img_component("screenshot-live-code", "Creating live JavaScript code blocks and AI-powered list conversion", IMG_W, IMG_H, false),
                    MainSide::Left
                ),
                space(SpacingSize::Large)
            ))),
        
        // Wave separator
        wave(DOWN_WAVE_PATH.to_string(), ThemeColor::Light, SpacingSize::Medium),
        
        // FAQ section
        theme(ThemeColor::Dark, content((
            space(SpacingSize::Large),
            faq_section(),
            space(SpacingSize::Large)
        ))),
        
        // Wave separator
        wave(UP_WAVE_PATH.to_string(), ThemeColor::Dark, SpacingSize::Medium),
        
        // Roadmap section
        theme(ThemeColor::Light, content((
            space(SpacingSize::Large),
            roadmap_section(),
            space(SpacingSize::Large)
        ))),
        
        // Wave separator
        wave(DOWN_WAVE_PATH.to_string(), ThemeColor::Light, SpacingSize::Medium),
        
        // Footer section
        theme(ThemeColor::Dark, content(div((
                    space(SpacingSize::Small),
                    action_buttons_panel(),
                    space(SpacingSize::Small),
                    space(SpacingSize::Small),
                    {
                        let divider_style = DividerStyle {
                            color: BorderStyle::LineBreak,
                        };
                        div("").class(divider_style.to_class())
                    },
                    space(SpacingSize::Small),
                    div((
                        p((
                            span("Done with "),
                            heart(),
                            link_to("twop.me", "Simon Korzunov")
                        )).class(&tw_join!("mt-3", TextStyle::Footnote)),
                        p((
                            "Shoot us an email at ",
                            link_to("mailto:hi@shelv.app", "hi@shelv.app")
                        )).class(&tw_join!("mt-3", TextStyle::Footnote)),
                        div(p((
                                "theme inspired by ",
                                link_to("https://www.nordtheme.com/", "Nord")
                            )).class(&TextStyle::Footnote.as_class())).class("py-3 flex justify-end")
                    ))
                )).class("w-full px-4")))
    )).class(&tw_join!("flex flex-col", BackgroundColor::Default.as_class()))
}

// Component functions (converted from dioxus components)

fn theme(color: ThemeColor, children: impl Render + 'static) -> Element {
    let theme_style = match color {
        ThemeColor::Light => ThemeStyle {
            bg: BackgroundColor::Light,
            text: TextColor::Default,
        },
        ThemeColor::Dark => ThemeStyle {
            bg: BackgroundColor::Dark,
            text: TextColor::Default,
        },
    };
    
    div(children).class(&tw_join!("relative text-base", theme_style.to_class()))
}

fn content(children: impl Render + 'static) -> Element {
    div(children).class("mx-auto px-4 sm:px-6 max-w-4xl")
}

fn block_layout(left: Element, right: Element, main: MainSide) -> Element {
    let direction_class = match main {
        MainSide::Left => "flex-col",
        MainSide::Right => "flex-col-reverse",
    };

    div((
        div(left).class("md:flex-1"),
        div(right).class("md:flex-1")
    )).class(&tw_join!("relative flex md:flex-row gap-8 md:gap-10 items-center", direction_class))
}

fn space(size: SpacingSize) -> Element {
    div("").class(&size.as_class())
}

fn img_component(src: &str, alt: &str, width: usize, height: usize, eager: bool) -> Element {
    div(div(img()
                .class("rounded-lg w-full h-full")
                .attr("width", &width.to_string())
                .attr("height", &height.to_string())
                .attr("loading", if eager { "eager" } else { "lazy" })
                .attr("alt", alt)
                .attr("src", &format!("/assets/images/{}.png", src)))
            .class("rounded-lg"/* shadow-(--shadow-underglow) */)).class("py-6 lg:py-0 w-full h-full flex justify-center")
}

fn page_header() -> Element {
    div((
        div(shelv_logo()).class("inline-flex items-center space-x-2 leading-6 font-medium transition ease-in-out duration-150"),
        div((
            {
                let nav_text_color = TextColor::Subtle;
                a("FAQ").href("#faq").class(&tw_join!("text-sm leading-6", nav_text_color))
            },
            {
                let nav_text_color = TextColor::Subtle;
                a("Discord").href("#").class(&tw_join!("text-sm leading-6", nav_text_color))
            },
            {
                let nav_text_color = TextColor::Subtle;
                a("License").href("#").class(&tw_join!("text-sm leading-6", nav_text_color))
            }
        )).class("flex gap-x-12")
    )).class("flex justify-between items-center py-6")
}

fn shelv_logo() -> impl Render {
    danger(r#"<svg 
            fill="none" 
            viewBox="0 0 181 51" 
            width="110" 
            height="30" 
            xmlns="http://www.w3.org/2000/svg"
            class="shelv-logo">
        <g filter="url(#filter0_d_140_26)">
            <path 
                fill="white" 
                d="M14.251 7.24903C16.6514 4.84857 19.9072 3.5 23.3019 3.5H63L51.749 14.751C49.3486 17.1514 46.0928 18.5 42.6981 18.5H3L14.251 7.24903Z"/>
        </g>
        <g filter="url(#filter1_d_140_26)">
            <path 
                fill="white" 
                d="M14.251 33.249C16.6514 30.8486 19.9072 29.5 23.3019 29.5H63L51.749 40.751C49.3486 43.1514 46.0928 44.5 42.6981 44.5H3L14.251 33.249Z"/>
        </g>
        <path 
            fill="white"
            d="M91.679 17.9062C91.5464 16.6657 90.9877 15.6998 90.0028 15.0085C89.0275 14.3172 87.7585 13.9716 86.196 13.9716C85.0975 13.9716 84.1553 14.1373 83.3693 14.4688C82.5833 14.8002 81.982 15.25 81.5653 15.8182C81.1487 16.3864 80.9356 17.035 80.9261 17.7642C80.9261 18.3703 81.0634 18.8958 81.3381 19.3409C81.6222 19.786 82.0057 20.1648 82.4886 20.4773C82.9716 20.7803 83.5066 21.036 84.0938 21.2443C84.6809 21.4527 85.2727 21.6278 85.8693 21.7699L88.5966 22.4517C89.6951 22.7074 90.7509 23.053 91.7642 23.4886C92.7869 23.9242 93.7008 24.4735 94.5057 25.1364C95.3201 25.7992 95.964 26.5994 96.4375 27.5369C96.911 28.4744 97.1477 29.5729 97.1477 30.8324C97.1477 32.5369 96.7121 34.0379 95.8409 35.3352C94.9697 36.6231 93.7102 37.6316 92.0625 38.3608C90.4242 39.0805 88.4403 39.4403 86.1108 39.4403C83.8475 39.4403 81.8826 39.09 80.2159 38.3892C78.5587 37.6884 77.2614 36.6657 76.3239 35.321C75.3958 33.9763 74.8939 32.3381 74.8182 30.4062H80.0028C80.0786 31.4195 80.3911 32.2623 80.9403 32.9347C81.4896 33.607 82.2045 34.1089 83.0852 34.4403C83.9754 34.7718 84.9697 34.9375 86.0682 34.9375C87.214 34.9375 88.2178 34.767 89.0795 34.4261C89.9508 34.0758 90.6326 33.5928 91.125 32.9773C91.6174 32.3523 91.8684 31.6231 91.8778 30.7898C91.8684 30.0322 91.6458 29.4072 91.2102 28.9148C90.7746 28.4129 90.1638 27.9962 89.3778 27.6648C88.6013 27.3239 87.6922 27.0208 86.6506 26.7557L83.3409 25.9034C80.9451 25.2879 79.0511 24.3551 77.6591 23.1051C76.2765 21.8456 75.5852 20.1742 75.5852 18.0909C75.5852 16.3769 76.0492 14.8759 76.9773 13.5881C77.9148 12.3002 79.1884 11.3011 80.7983 10.5909C82.4081 9.87121 84.2311 9.51136 86.267 9.51136C88.3314 9.51136 90.1402 9.87121 91.6932 10.5909C93.2557 11.3011 94.482 12.2907 95.3722 13.5597C96.2623 14.8191 96.7216 16.268 96.75 17.9062H91.679ZM106.761 26.2159V39H101.619V9.90909H106.647V20.8892H106.903C107.414 19.6581 108.205 18.6875 109.275 17.9773C110.354 17.2576 111.728 16.8977 113.394 16.8977C114.909 16.8977 116.23 17.215 117.357 17.8494C118.484 18.4839 119.355 19.4119 119.971 20.6335C120.596 21.8551 120.908 23.3466 120.908 25.108V39H115.766V25.9034C115.766 24.4356 115.388 23.2945 114.63 22.4801C113.882 21.6562 112.831 21.2443 111.477 21.2443C110.567 21.2443 109.753 21.4432 109.033 21.8409C108.323 22.2292 107.764 22.7926 107.357 23.5312C106.96 24.2699 106.761 25.1648 106.761 26.2159ZM135.809 39.4261C133.621 39.4261 131.732 38.9716 130.141 38.0625C128.56 37.1439 127.343 35.8466 126.491 34.1705C125.638 32.4848 125.212 30.5009 125.212 28.2188C125.212 25.9744 125.638 24.0047 126.491 22.3097C127.353 20.6051 128.555 19.2794 130.099 18.3324C131.642 17.3759 133.456 16.8977 135.539 16.8977C136.884 16.8977 138.153 17.1155 139.346 17.5511C140.549 17.9773 141.609 18.6402 142.528 19.5398C143.456 20.4394 144.185 21.5852 144.715 22.9773C145.246 24.3598 145.511 26.0076 145.511 27.9205V29.4972H127.627V26.0312H140.582C140.572 25.0464 140.359 24.1705 139.942 23.4034C139.526 22.6269 138.943 22.0161 138.195 21.571C137.457 21.1259 136.595 20.9034 135.61 20.9034C134.559 20.9034 133.636 21.1591 132.84 21.6705C132.045 22.1723 131.424 22.8352 130.979 23.6591C130.544 24.4735 130.321 25.3684 130.312 26.3438V29.3693C130.312 30.6383 130.544 31.7273 131.008 32.6364C131.472 33.536 132.121 34.2273 132.954 34.7102C133.787 35.1837 134.763 35.4205 135.88 35.4205C136.628 35.4205 137.305 35.3163 137.911 35.108C138.517 34.8902 139.043 34.5729 139.488 34.1562C139.933 33.7396 140.269 33.2235 140.496 32.608L145.298 33.1477C144.995 34.4167 144.417 35.5246 143.565 36.4716C142.722 37.4091 141.642 38.1383 140.326 38.6591C139.01 39.1705 137.504 39.4261 135.809 39.4261ZM155.003 9.90909V39H149.861V9.90909H155.003ZM179.779 17.1818L172.009 39H166.327L158.558 17.1818H164.04L169.055 33.3892H169.282L174.31 17.1818H179.779Z"/>
        <defs>
            <filter 
                id="filter0_d_140_26" 
                x="0" 
                width="68" 
                filterUnits="userSpaceOnUse" 
                y="1.5" 
                height="23" 
                color-interpolation-filters="sRGB">
                <feFlood flood-opacity="0" result="BackgroundImageFix"/>
                <feColorMatrix 
                    values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 127 0" 
                    in="SourceAlpha" 
                    result="hardAlpha" 
                    type="matrix"/>
                <feOffset dx="1" dy="2"/>
                <feGaussianBlur stdDeviation="2"/>
                <feComposite operator="out" in2="hardAlpha"/>
                <feColorMatrix 
                    type="matrix" 
                    values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0.67 0"/>
                <feBlend 
                    in2="BackgroundImageFix" 
                    mode="normal" 
                    result="effect1_dropShadow_140_26"/>
                <feBlend 
                    mode="normal" 
                    in="SourceGraphic" 
                    result="shape" 
                    in2="effect1_dropShadow_140_26"/>
            </filter>
            <filter 
                x="0" 
                width="68" 
                filterUnits="userSpaceOnUse" 
                y="27.5" 
                height="23" 
                color-interpolation-filters="sRGB" 
                id="filter1_d_140_26">
                <feFlood result="BackgroundImageFix" flood-opacity="0"/>
                <feColorMatrix 
                    in="SourceAlpha" 
                    type="matrix" 
                    result="hardAlpha" 
                    values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 127 0"/>
                <feOffset dy="2" dx="1"/>
                <feGaussianBlur stdDeviation="2"/>
                <feComposite in2="hardAlpha" operator="out"/>
                <feColorMatrix 
                    type="matrix" 
                    values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0.67 0"/>
                <feBlend 
                    in2="BackgroundImageFix" 
                    mode="normal" 
                    result="effect1_dropShadow_140_26"/>
                <feBlend 
                    in="SourceGraphic" 
                    in2="effect1_dropShadow_140_26" 
                    mode="normal" 
                    result="shape"/>
            </filter>
        </defs>
    </svg>"#)
}

fn heart() -> impl Render {
    let heart_color = TextColor::Red;
    danger(format!(r#"<svg class="h-4 w-4 inline {color}" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4.318 6.318a4.5 4.5 0 000 6.364L12 20.364l7.682-7.682a4.5 4.5 0 00-6.364-6.364L12 7.636l-1.318-1.318a4.5 4.5 0 00-6.364 0z"></path>
        </svg>"#, color=heart_color.as_class()))
}

fn wave(path: String, top_color: ThemeColor, size: SpacingSize) -> Element {
    let bg_color = match top_color {
        ThemeColor::Dark => BackgroundColor::Dark,
        ThemeColor::Light => BackgroundColor::Light,
    };
    
    let fill_color = match top_color {
        ThemeColor::Dark => "var(--color-nord0-dark)",
        ThemeColor::Light => "var(--color-nord0-darker)",
    };

    div(div(danger(&format!(r#"<svg width="100%" height="100%" viewBox="0 0 1440 160" preserveAspectRatio="none" xmlns="http://www.w3.org/2000/svg">
            <defs>
                <filter id="shadow">
                    <feDropShadow dx="0" dy="-20" std-deviation="5"/>
                </filter>
            </defs>
            <g transform="scale(1, 0.5)">
                <path fill="{}" d="{}"/>
            </g>
        </svg>"#, fill_color, path))).class("w-full h-full")).class(&tw_join!("w-full", bg_color, size))
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

fn block_text(text: &str) -> Element {
    let text_style = StyledText {
        color: TextColor::Subtle,
        style: TextStyle::Paragraph,
    };
    p(text.to_string()).class(&text_style.to_class())
}

fn slogan_and_mac_store_link() -> Element {
    let child = div((
                {
                    let h1_style = StyledText {
                        color: TextColor::MainHeader,
                        style: TextStyle::MainHeader,
                    };
                    h1("Hackable, Local, AI-powered notes")
                        .class(&h1_style.to_class())
                },
                {
                    let desc_style = StyledText {
                        color: TextColor::Subtle,
                        style: TextStyle::Paragraph,
                    };
                    p("Shelv is a scriptable, plain text notes app with integrated AI features for macOS, written in Rust (by the way (tm)).")
                        .class(&tw_join!("mt-4 max-w-md mx-auto md:mt-5 md:max-w-3xl", desc_style.to_class()))
                }
            ));
    div((
        // Slogan section
        div(child).class("text-center lg:text-left"),
        
        // Action buttons panel section
        div(action_buttons_panel()).class("mt-8 sm:max-w-lg sm:mx-auto text-center sm:text-center lg:text-left lg:mx-0")
    ))
}

fn action_buttons_panel() -> Element {
    div((
        mac_store_link(),
        github_link()
    )).class("flex flex-col sm:flex-row gap-4 items-center justify-center lg:justify-start")
}

fn mac_store_link() -> Element {
    a(img().attr("src", "/assets/images/mac-app-store-badge.svg")
            .attr("alt", "Coming Soon on Mac")
            .class("home-app-store-buttons-mac h-10")
            .attr("height", "48")).href("https://testflight.apple.com/join/38OBZSRD")
}

fn secondary_button_link(href: &str, content: impl Render + 'static) -> Element {
    let button_style = ButtonStyle {
        height: ButtonHeight::FixedH10,
        variant:ButtonVariant::Secondary
    };
    
    a(content).class(&button_style.to_class()).href(href)
}

fn github_link() -> Element {
    secondary_button_link("https://github.com/briskmode/shelv", (
        github_icon(),
        span("Give us a star").class("ml-2")
    ))
}

fn github_icon() -> impl Render {
    danger(r#"<svg viewBox="0 0 20 20" class="size-5 fill-current">
        <path d="M10 0C4.475 0 0 4.475 0 10a9.994 9.994 0 006.838 9.488c.5.087.687-.213.687-.476 0-.237-.013-1.024-.013-1.862-2.512.463-3.162-.612-3.362-1.175-.113-.287-.6-1.175-1.025-1.412-.35-.188-.85-.65-.013-.663.788-.013 1.35.725 1.538 1.025.9 1.512 2.337 1.087 2.912.825.088-.65.35-1.088.638-1.338-2.225-.25-4.55-1.112-4.55-4.937 0-1.088.387-1.987 1.025-2.688-.1-.25-.45-1.274.1-2.65 0 0 .837-.262 2.75 1.026a9.28 9.28 0 012.5-.338c.85 0 1.7.112 2.5.337 1.912-1.3 2.75-1.024 2.75-1.024.55 1.375.2 2.4.1 2.65.637.7 1.025 1.587 1.025 2.687 0 3.838-2.337 4.688-4.562 4.938.362.312.675.912.675 1.85 0 1.337-.013 2.412-.013 2.75 0 .262.188.574.688.474A10.016 10.016 0 0020 10c0-5.525-4.475-10-10-10z"></path>
    </svg>"#)
}

fn faq_section() -> Element {
    div((
        block_header("Frequently Asked Questions"),
        space(SpacingSize::Medium),
        div(faq_items().into_iter().map(|(question, answer)| {
            faq_item(question, answer)
        }).collect::<Vec<_>>()).class("flex flex-col gap-6")
    )).class("max-w-3xl mx-auto").id("faq")
}

fn faq_items() -> Vec<(&'static str, Element)> {
    vec![
        (
            "Is Shelv coming to Mobile/Window/Web?",
            p((
                "Yes, but with time. Shelv is written in Rust + ",
                link_to("https://egui.rs/", "egui"),
                ", so it is possible to port it as is on all these platforms."
            )).class(&tw_join!(TextStyle::Paragraph, TextColor::Subtle))
        ),
        (
            "How do you make money?",
            p((
                "I don't. I worked on Shelv for over 2 years, and I had a dream to start company(still do), but as of now, it is a labor of love, because I couldn't find a good business model, if you have ideas please let me know. Tentatively I plan to add ability just to buy tokens, but that seems lame. I plan to cap to $20/month the claude account assosiated with the app, but you can choose your providers for AI features, includind ",
                link_to("https://ollama.com/", "Ollama"),
                "."
            )).class(&tw_join!(TextStyle::Paragraph, TextColor::Subtle))
        ),
        (
            "Do you have sync?",
            p((
                "Not yet, I'm a local first movement fan, and wanted to use ",
                link_to("https://github.com/automerge/automerge", "Automerge"),
                " forever, but I want to implement e2e encryption with Rust sever, which is being worked on right now, and it is darn had to do an e2e encrypted scalable sync technically and from product point of view."
            )).class(&tw_join!(TextStyle::Paragraph, TextColor::Subtle))
        ),
        (
            "Is Shelv open source?",
            p((
                "Yes and no, it has a licence inspired by ",
                link_to("https://polyformproject.org/licenses/strict/1.0.0", "PolyForm Strict 1.0.0 license"),
                ". Which means that you cannot use Shelv compiled from source for work or repackage it to a new app. However that applies to the \"build from\" source option, you can (and hopefully will) just use the version from the app store."
            )).class(&tw_join!(TextStyle::Paragraph, TextColor::Subtle))
        ),
        (
            "Is it Native?",
            p((
                "Native is a spectrum, shelv is written in Rust using ",
                link_to("https://egui.rs/", "egui"),
                " as the gui toolkit, which in turn is using wgpu, not Swift UI tech stack. Maybe the closest analogy would be ",
                link_to("https://flutter.dev/", "Flutter"),
                " that is painting every pixel. Are Flutter apps native? I think so."
            )).class(&tw_join!(TextStyle::Paragraph, TextColor::Subtle))
        ),
        (
            "Are my beloved vim motions supported?",
            p((
                "I am a ",
                link_to("https://helix-editor.com/", "Helix"),
                " user myself, but markdown and text are a bit different from code, that said, I would love to support modal editing in the future. I do think that some features can be added for just \"insert\" mode (which is the only mode at the moment) that can enhance editing, for example: jump to a word, press any buttons with a label(vimium style), expand + shrink semantic selection etc. I need to work on Shelv full-time to justify adding vim or helix motions to egui TextEdit, vote with you money I guess, oh wait, I don't have a way to actually recieve money..."
            )).class(&tw_join!(TextStyle::Paragraph, TextColor::Subtle))
        ),
        (
            "Are you collecting any analytics?",
            p((
                "Not at the moment (besides crash reporting), I'm not fundamentally opposed to collecting statistics, because it is hard to know if a feature is even used without some observability, but I do think it can be done with privacy in mind (at least anonymizing and being mindful of where the data is stored). Probably in the future, however, when and if I add monetization, I'll likely start collecting emails associated with a purchase and/or install"
            )).class(&tw_join!(TextStyle::Paragraph, TextColor::Subtle))
        )
    ]
}

fn faq_item(question: &str, answer: Element) -> Element {
    // answer is now an Element, not a string

    let button_style = ButtonStyle {
        height: ButtonHeight::ContentBased,
        variant: ButtonVariant::Secondary
    };

    div((
        button(div((
            span(question.to_string()).class(tw_join!("text-left flex-1", TextStyle::Paragraph)),
            faq_chevron()
        )).class("flex items-center justify-between w-full"))
        .class(&tw_join!("w-full text-left", button_style.to_class()))
        .attr("onclick", "this.nextElementSibling.classList.toggle('hidden'); this.querySelector('.faq-chevron').classList.toggle('rotate-180')"),
        
        div(answer)
        .class("hidden p-3")
    ))
}

fn faq_chevron() -> impl Render {
    danger(r#"<svg class="w-5 h-5 transition-transform duration-200 faq-chevron" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path>
    </svg>"#)
}

fn roadmap_section() -> Element {
    div((
        block_header("Roadmap"),
        space(SpacingSize::Medium),
        ol(roadmap_items().into_iter().map(|(date, completed, name, description)| {
            roadmap_item(date, completed, name, description)
        }).collect::<Vec<_>>()).class("relative border-s border-nord3")
    )).class("max-w-4xl mx-auto")
}

fn roadmap_items() -> Vec<(Option<&'static str>, bool, &'static str, Vec<&'static str>)> {
    vec![
        (Some("Aug 2025"), true, "Initial launch on macOS", vec![
            "Barebones editing with 4 notes",
            "Optimized for quick capture",
            "No API exposed to JS scripts"
        ]),
        (None, false, "Multi-file + workspace support", vec![
            "Workspace folder with notes inside",
            "Import from Obsidian",
            "File tree + workspace viewer"
        ]),
        (None, false, "Agentic mode", vec![
            "Tools/MCP that allow to search/move/create/edit notes",
            "UI for having agentic workflows, probably just a chat that is going to be just another file",
            "Files that define custom workflows, similar to Claude Code"
        ]),
        (None, false, "Core editing features", vec![
            "Semantic selection: expand and shrink cursor selection with markdown AST nodes",
            "Jump to an element, jump to any word on the screen with a couple of keystrokes (similar to Vimium and Helix)",
            "Search, Redo etc"
        ]),
        (None, false, "Support for pasting/rendering images", vec![]),
        (None, false, "Rich API exposed to JS + better scripting capabilities", vec![
            "Sharing code among notes"
        ]),
        (None, false, "Sync", vec![
            "I plan to use Automerge for personal syncing, which can be also used for collaboration",
            "Dump to git, e.g. backup all the notes to git, potentially with AI-generated change summary"
        ]),
        (None, false, "Web version", vec![
            "Mobile (including web) version is TBD"
        ]),
        (None, false, "Collaboration", vec![
            "Share a note via link (co-editing on the web)",
            "Share workspace, that is, co-ownership of a collection of folder+notes"
        ])
    ]
}

fn roadmap_item(date: Option<&str>, completed: bool, name: &str, description: Vec<&str>) -> Element {
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
                    span(name.to_string()).class(&tw_join!("font-semibold", TextStyle::Paragraph.as_class(), if completed { TextColor::Default.as_class() } else { TextColor::Subtle.as_class() })),
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
                li(item.to_string()).class(&tw_join!("mb-1", TextStyle::Footnote.as_class(), TextColor::Subtle.as_class()))
            }).collect::<Vec<_>>()).class("list-disc list-inside space-y-1")).class("hidden")
        )).class("-translate-y-2")
    )).class("mb-6 ms-6")
}

fn roadmap_chevron() -> Element {
    let button_style = ButtonStyle {
        height: ButtonHeight::FixedH10,
        variant: ButtonVariant::SecondaryTextOnly
    };
    
    div(danger(r#"<svg class="w-5 h-5 transition-transform duration-200 roadmap-chevron" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path>
    </svg>"#)).class(&button_style.to_class())
}

fn roadmap_icon(completed: bool) -> impl Render {
    if completed {
        danger(r#"<svg class="w-3 h-3 text-white" fill="currentColor" viewBox="0 0 20 20">
            <path d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"/>
        </svg>"#)
    } else {
        danger(r#"<svg class="w-3 h-3 text-nord4" fill="currentColor" viewBox="0 0 20 20">
            <path d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"/>
        </svg>"#)
    }
}

fn features_bullet_list() -> Element {
    let features = vec![
        ("Markdown Support", "Full CommonMark with extensions, including TODOs"),
        ("Code Syntax Highlighting", "A lot of languages are supported"),
        ("Live JavaScript Blocks", "Execute JS code directly in notes"),
        ("Slash Menu", "Quick access to all commands and features"),
        ("Keyboard optimized", "Everything is available via shortcuts"),
    ];

    ul(features.into_iter().map(|(title, description)| {
        li((
            span(title.to_string()).class(&tw_join!("font-bold", TextColor::Default, TextStyle::Footnote)),
            br(),
            span(description.to_string()).class(&tw_join!(TextColor::Subtle, TextStyle::Footnote, "pl-4"))
        ))
    }).collect::<Vec<_>>()).class("list-disc flex flex-col gap-4 list-inside pl-2")
}

// HTML rendering helper
fn render_to_string(element: Element) -> String {
    render((
        doctype(),
        html((
            head((
                title("Shelv - Hackable Playground for Ephemeral Thoughts"),
                meta().charset("utf-8"),
                meta().name("viewport").content("width=device-width, initial-scale=1"),
                link("").rel("preconnect").href("https://rsms.me/"),
                link("").rel("stylesheet").href("https://rsms.me/inter/inter.css"),
                link("").rel("stylesheet").href("/assets/app.css"),
                link("").rel("stylesheet").href("/assets/main.css"),
            )),
            body(element).class(BackgroundColor::Default.as_class()),
        )),
    ))
}

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([127, 0, 0, 1], 4000));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    
    println!("Server running on http://127.0.0.1:4000");
    
    // Create the main router with enum_router
    let app_router = Route::router();
    
    // Add static file serving for assets
    let router = app_router.nest_service("/assets", ServeDir::new("assets"));
    
    axum::serve(listener, router).await.unwrap();
}
