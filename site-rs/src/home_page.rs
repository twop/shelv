use dioxus::prelude::*;

const UP_WAVE_PATH:&str =
  "M0,128L120,144C240,160,480,192,720,208C960,224,1200,224,1320,224L1440,224L1440,320L1320,320C1200,320,960,320,720,320C480,320,240,320,120,320L0,320Z";

const DOWN_WAVE_PATH:&str =
  "M0,224L80,186.7C160,149,320,75,480,53.3C640,32,800,64,960,85.3C1120,107,1280,117,1360,122.7L1440,128L1440,320L1360,320C1280,320,1120,320,960,320C800,320,640,320,480,320C320,320,160,320,80,320L0,320Z";

const IMG_W: usize = 1180;
const IMG_H: usize = 1128;

#[component]
pub fn HomePage() -> Element {
    rsx! {
        Theme { color: ThemeColor::Dark,
            Content { PageHeader {} }
            Content { 
                Block {
                    left: rsx! {
                        SloganAndMacStoreLink {}
                    },
                    right: rsx! {
                        Img {
                            width: IMG_W,
                            height: IMG_H,
                            src: "screenshot-welcome",
                            eager: true,
                            alt: "app screenshot with welcome note"
                        }
                    }
                }
            }
        }
        Wave { path: UP_WAVE_PATH, top_color: ThemeColor::Dark }
        Theme { color: ThemeColor::Light, Content { 
            Block {
                left: rsx! {
                    BlockHeader { "Markdown Native" }
                    BlockText { "Shelv is built on markdown, which means you can quickly format your ideas in an expressive way that is open and portable to where ever they need to go." }
                },
                right: rsx! {
                    Img {
                        width: IMG_W,
                        height: IMG_H,
                        src: "screenshot-markdown",
                        alt: "app screenshot with markdown features"
                    }
                }
            }
            Space { extra_on_large: true }
            Block {
                left: rsx! {
                    Img {
                        width: IMG_W,
                        height: IMG_H,
                        src: "screenshot-shortcuts",
                        alt: "app screenshot with shortcuts"
                    }
                },
                right: rsx! {
                    BlockHeader { "Keyboard shortcuts" }
                    BlockText { "Show/Hide Shelv with a system wide shortcut, so it is there when you need it." }
                    Space { sm: true }
                    BlockText { 
                        "Annotation shortcuts for "
                        b { "Bold" }
                        ", "
                        i { "Italic" }
                        ", Headings and "
                        code { "Code blocks" }
                    }
                },
                main: MainSide::Right
            }
            Space { sm: true }
        } }
        Wave { path: DOWN_WAVE_PATH, top_color: ThemeColor::Light }
        Theme { color: ThemeColor::Dark, Content { 
            div { class: "w-full px-4",
                div { class: "border-solid border-t-1 w-full border-nord-line-break mt-8 mb-6" }
                div {
                    p { class: "mt-3 text-m leading-7",
                        "Done with "
                        Heart {}
                        " by Briskmode Labs"
                    }
                    p { class: "mt-3 text-m leading-7",
                        "Shoot us an email at "
                        LinkTo { to: "mailto:hi@shelv.app", "hi@shelv.app" }
                    }
                    div { class: "py-3 flex justify-end",
                        p { class: "text-xs leading-7",
                            "theme inspired by "
                            LinkTo { to: "https://www.nordtheme.com/", "Nord" }
                        }
                    }
                }
            }
        } }
    }
}

// Define other components (Theme, Content, Block, Img, Space, etc.) here

#[component]
fn Heart() -> Element {
    rsx! {
        svg {
            class: "text-nord-red h-4 w-4 inline",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            path {
                stroke_linecap: "round",
                stroke_linejoin: "round",
                stroke_width: "2",
                d: "M4.318 6.318a4.5 4.5 0 000 6.364L12 20.364l7.682-7.682a4.5 4.5 0 00-6.364-6.364L12 7.636l-1.318-1.318a4.5 4.5 0 00-6.364 0z"
            }
        }
    }
}

#[derive(Props, PartialEq, Clone)]
struct SpaceProps {
    #[props(default = false)]
    sm: bool,
    #[props(default = false)]
    md: bool,
    #[props(default = false)]
    lg: bool,
    #[props(default = false)]
    extra_on_large: bool,
}

#[component]
fn Space(props: SpaceProps) -> Element {
    let size_class = if props.sm {
        "h-4 sm:h-8"
    } else if props.md {
        "h-8 sm:h-12"
    } else if props.lg {
        "h-12 sm:h-16"
    } else {
        "h-8 sm:h-12"
    };

    let extra_class = if props.extra_on_large { "lg:my-6" } else { "" };

    rsx! {
        div { class: "w-full {size_class} {extra_class}" }
    }
}

#[component]
fn LinkTo(to: String, children: Element) -> Element {
    rsx! {
        a { class: "text-nord-text-primary hover:underline", href: "{to}", {children} }
    }
}

#[component]
fn SvgIconLink(link_to: String, path: Element, size: Option<String>) -> Element {
    let size_class = match size.unwrap_or_else(|| "small".to_string()).as_str() {
        "small" => "h-4 w-4",
        "large" => "h-8 w-8",
        _ => "h-4 w-4",
    };

    rsx! {
        a { href: "{link_to}",
            svg {
                class: "inline fill-current text-nord-text-primary hover:text-nord-bg-btn-hovered {size_class}",
                view_box: "0 0 24 24",
                enable_background: "new 0 0 24 24",
                {path}
            }
        }
    }
}

#[derive(PartialEq, Clone)]
pub enum ThemeColor {
    Dark,
    Light,
}

#[component]
fn Wave(path: String, top_color: ThemeColor) -> Element {
    let (top_color, fill_color) = match top_color {
        ThemeColor::Dark => ("bg-nord-bg-dark", "#1a202c"),
        ThemeColor::Light => ("bg-nord-bg", "#0f1521"),
    };

    rsx! {
        div { class: "{top_color}",
            svg { view_box: "0 0 1440 160", xmlns: "http://www.w3.org/2000/svg",
                defs {
                    filter { id: "shadow",
                        feDropShadow { dx: "0", dy: "-20", std_deviation: "5" }
                    }
                }
                g { transform: "scale(1, 0.5)",
                    path { fill: "{fill_color}", d: "{path}" }
                }
            }
        }
    }
}

#[component]
fn Theme(color: ThemeColor, children: Element) -> Element {
    let (bg_class, text_class) = match color {
        ThemeColor::Light => ("bg-nord-bg", "text-nord-text"),
        ThemeColor::Dark => ("bg-nord-bg-dark", "text-nord-text"),
    };

    rsx! {
        div { class: "relative text-base {text_class} {bg_class}", {children} }
    }
}

#[component]
fn Content(children: Element) -> Element {
    rsx! {
        div { class: "mx-auto px-4 sm:px-6 max-w-6xl", {children} }
    }
}

/// Enum to represent which side is the main content
#[derive(PartialEq, Clone)]
enum MainSide {
    Left,
    Right,
}

#[derive(Props, Clone, PartialEq)]
struct BlockProps {
    left: Element,
    right: Element,
    #[props(default = MainSide::Left)]
    main: MainSide,
}

/// A component that creates a responsive two-column layout
#[component]
fn Block(props: BlockProps) -> Element {
    // Determine the flex direction based on which side is the main content
    let flex_direction = match props.main {
        MainSide::Right => "flex-col-reverse",
        MainSide::Left => "flex-col",
    };

    rsx! {
        // Outer container for relative positioning
        div { class: "relative",
            // Inner container for the two-column layout
            div {
                // Classes for responsive behavior:
                // - Flex column on small screens
                // - Two-column grid on large screens
                class: "flex {flex_direction} lg:flex-none lg:grid lg:grid-flow-row-dense lg:grid-cols-2 lg:gap-10 lg:items-center",
                // Left column
                div { class: "lg:pr-4", {props.left} }
                // Right column
                div { class: "lg:pl-4", {props.right} }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct ImgProps {
    src: String,
    alt: String,
    width: usize,
    height: usize,
    #[props(default = false)]
    eager: bool,
    #[props(default = "".to_string())]
    extra_style: String,
}

/// A component for rendering images with optional lazy loading
#[component]
fn Img(props: ImgProps) -> Element {
    rsx! {
        // Container div for centering the image
        div { class: "py-6 lg:py-0 w-full h-full flex justify-center",
            img {
                // Apply extra styles if provided
                class: "{props.extra_style}",
                width: "{props.width}",
                height: "{props.height}",
                // Use eager loading if specified, otherwise lazy load
                loading: if props.eager { "eager" } else { "lazy" },
                alt: "{props.alt}",
                // Assume images are in a specific directory with .png extension
                src: "images/{props.src}.png"
            }
        }
    }
}

/// Component for rendering the page header
#[component]
fn PageHeader() -> Element {
    rsx! {
        div { class: "flex justify-between items-center py-6",
            // Logo section
            div { class: "inline-flex items-center space-x-2 leading-6 font-medium transition ease-in-out duration-150",
                ShelvLogo {}
            }
            // Navigation links
            div { class: "flex gap-x-12",
                // Array of navigation items
                for item in [("FAQ", "#"), ("License", "#")] {
                    a {
                        key: "{item.0}",
                        href: "{item.1}",
                        class: "text-sm leading-6 text-nord-text-subtle",
                        "{item.0}"
                    }
                }
            }
        }
    }
}

#[component]
fn ShelvLogo() -> Element {
    rsx!(
        svg {
            "fill": "none",
            "viewBox": "0 0 181 51",
            width: "181",
            height: "51",
            "xmlns": "http://www.w3.org/2000/svg",
            g { "filter": "url(#filter0_d_140_26)",
                path {
                    "fill": "white",
                    "d": "M14.251 7.24903C16.6514 4.84857 19.9072 3.5 23.3019 3.5H63L51.749 14.751C49.3486 17.1514 46.0928 18.5 42.6981 18.5H3L14.251 7.24903Z"
                }
            }
            g { "filter": "url(#filter1_d_140_26)",
                path {
                    "fill": "white",
                    "d": "M14.251 33.249C16.6514 30.8486 19.9072 29.5 23.3019 29.5H63L51.749 40.751C49.3486 43.1514 46.0928 44.5 42.6981 44.5H3L14.251 33.249Z"
                }
            }
            path {
                "d": "M91.679 17.9062C91.5464 16.6657 90.9877 15.6998 90.0028 15.0085C89.0275 14.3172 87.7585 13.9716 86.196 13.9716C85.0975 13.9716 84.1553 14.1373 83.3693 14.4688C82.5833 14.8002 81.982 15.25 81.5653 15.8182C81.1487 16.3864 80.9356 17.035 80.9261 17.7642C80.9261 18.3703 81.0634 18.8958 81.3381 19.3409C81.6222 19.786 82.0057 20.1648 82.4886 20.4773C82.9716 20.7803 83.5066 21.036 84.0938 21.2443C84.6809 21.4527 85.2727 21.6278 85.8693 21.7699L88.5966 22.4517C89.6951 22.7074 90.7509 23.053 91.7642 23.4886C92.7869 23.9242 93.7008 24.4735 94.5057 25.1364C95.3201 25.7992 95.964 26.5994 96.4375 27.5369C96.911 28.4744 97.1477 29.5729 97.1477 30.8324C97.1477 32.5369 96.7121 34.0379 95.8409 35.3352C94.9697 36.6231 93.7102 37.6316 92.0625 38.3608C90.4242 39.0805 88.4403 39.4403 86.1108 39.4403C83.8475 39.4403 81.8826 39.09 80.2159 38.3892C78.5587 37.6884 77.2614 36.6657 76.3239 35.321C75.3958 33.9763 74.8939 32.3381 74.8182 30.4062H80.0028C80.0786 31.4195 80.3911 32.2623 80.9403 32.9347C81.4896 33.607 82.2045 34.1089 83.0852 34.4403C83.9754 34.7718 84.9697 34.9375 86.0682 34.9375C87.214 34.9375 88.2178 34.767 89.0795 34.4261C89.9508 34.0758 90.6326 33.5928 91.125 32.9773C91.6174 32.3523 91.8684 31.6231 91.8778 30.7898C91.8684 30.0322 91.6458 29.4072 91.2102 28.9148C90.7746 28.4129 90.1638 27.9962 89.3778 27.6648C88.6013 27.3239 87.6922 27.0208 86.6506 26.7557L83.3409 25.9034C80.9451 25.2879 79.0511 24.3551 77.6591 23.1051C76.2765 21.8456 75.5852 20.1742 75.5852 18.0909C75.5852 16.3769 76.0492 14.8759 76.9773 13.5881C77.9148 12.3002 79.1884 11.3011 80.7983 10.5909C82.4081 9.87121 84.2311 9.51136 86.267 9.51136C88.3314 9.51136 90.1402 9.87121 91.6932 10.5909C93.2557 11.3011 94.482 12.2907 95.3722 13.5597C96.2623 14.8191 96.7216 16.268 96.75 17.9062H91.679ZM106.761 26.2159V39H101.619V9.90909H106.647V20.8892H106.903C107.414 19.6581 108.205 18.6875 109.275 17.9773C110.354 17.2576 111.728 16.8977 113.394 16.8977C114.909 16.8977 116.23 17.215 117.357 17.8494C118.484 18.4839 119.355 19.4119 119.971 20.6335C120.596 21.8551 120.908 23.3466 120.908 25.108V39H115.766V25.9034C115.766 24.4356 115.388 23.2945 114.63 22.4801C113.882 21.6562 112.831 21.2443 111.477 21.2443C110.567 21.2443 109.753 21.4432 109.033 21.8409C108.323 22.2292 107.764 22.7926 107.357 23.5312C106.96 24.2699 106.761 25.1648 106.761 26.2159ZM135.809 39.4261C133.621 39.4261 131.732 38.9716 130.141 38.0625C128.56 37.1439 127.343 35.8466 126.491 34.1705C125.638 32.4848 125.212 30.5009 125.212 28.2188C125.212 25.9744 125.638 24.0047 126.491 22.3097C127.353 20.6051 128.555 19.2794 130.099 18.3324C131.642 17.3759 133.456 16.8977 135.539 16.8977C136.884 16.8977 138.153 17.1155 139.346 17.5511C140.549 17.9773 141.609 18.6402 142.528 19.5398C143.456 20.4394 144.185 21.5852 144.715 22.9773C145.246 24.3598 145.511 26.0076 145.511 27.9205V29.4972H127.627V26.0312H140.582C140.572 25.0464 140.359 24.1705 139.942 23.4034C139.526 22.6269 138.943 22.0161 138.195 21.571C137.457 21.1259 136.595 20.9034 135.61 20.9034C134.559 20.9034 133.636 21.1591 132.84 21.6705C132.045 22.1723 131.424 22.8352 130.979 23.6591C130.544 24.4735 130.321 25.3684 130.312 26.3438V29.3693C130.312 30.6383 130.544 31.7273 131.008 32.6364C131.472 33.536 132.121 34.2273 132.954 34.7102C133.787 35.1837 134.763 35.4205 135.88 35.4205C136.628 35.4205 137.305 35.3163 137.911 35.108C138.517 34.8902 139.043 34.5729 139.488 34.1562C139.933 33.7396 140.269 33.2235 140.496 32.608L145.298 33.1477C144.995 34.4167 144.417 35.5246 143.565 36.4716C142.722 37.4091 141.642 38.1383 140.326 38.6591C139.01 39.1705 137.504 39.4261 135.809 39.4261ZM155.003 9.90909V39H149.861V9.90909H155.003ZM179.779 17.1818L172.009 39H166.327L158.558 17.1818H164.04L169.055 33.3892H169.282L174.31 17.1818H179.779Z",
                "fill": "white"
            }
            defs {
                filter {
                    "y": "1.5",
                    width: "68",
                    "x": "0",
                    "color-interpolation-filters": "sRGB",
                    height: "23",
                    "filterUnits": "userSpaceOnUse",
                    id: "filter0_d_140_26",
                    feFlood { "flood-opacity": "0", "result": "BackgroundImageFix" }
                    feColorMatrix {
                        "values": "0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 127 0",
                        "in": "SourceAlpha",
                        "result": "hardAlpha",
                        r#type: "matrix"
                    }
                    feOffset { "dx": "1", "dy": "2" }
                    feGaussianBlur { "stdDeviation": "2" }
                    feComposite { "operator": "out", "in2": "hardAlpha" }
                    feColorMatrix {
                        r#type: "matrix",
                        "values": "0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0.67 0"
                    }
                    feBlend {
                        "in2": "BackgroundImageFix",
                        "mode": "normal",
                        "result": "effect1_dropShadow_140_26"
                    }
                    feBlend {
                        "mode": "normal",
                        "in": "SourceGraphic",
                        "result": "shape",
                        "in2": "effect1_dropShadow_140_26"
                    }
                }
                filter {
                    "x": "0",
                    width: "68",
                    "filterUnits": "userSpaceOnUse",
                    "y": "27.5",
                    height: "23",
                    "color-interpolation-filters": "sRGB",
                    id: "filter1_d_140_26",
                    feFlood { "result": "BackgroundImageFix", "flood-opacity": "0" }
                    feColorMatrix {
                        "in": "SourceAlpha",
                        r#type: "matrix",
                        "result": "hardAlpha",
                        "values": "0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 127 0"
                    }
                    feOffset { "dy": "2", "dx": "1" }
                    feGaussianBlur { "stdDeviation": "2" }
                    feComposite { "in2": "hardAlpha", "operator": "out" }
                    feColorMatrix {
                        r#type: "matrix",
                        "values": "0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0.67 0"
                    }
                    feBlend {
                        "in2": "BackgroundImageFix",
                        "mode": "normal",
                        "result": "effect1_dropShadow_140_26"
                    }
                    feBlend {
                        "in": "SourceGraphic",
                        "in2": "effect1_dropShadow_140_26",
                        "mode": "normal",
                        "result": "shape"
                    }
                }
            }
        }
    )
}

/// A component for rendering a block of text with consistent styling
#[component]
fn BlockText(children: Element) -> Element {
    rsx! {
        p { class: "text-lg leading-7", {children} }
    }
}

/// A component for rendering a header with consistent styling
#[component]
fn BlockHeader(children: Element) -> Element {
    rsx! {
        h4 { class: "text-2xl mb-4 leading-8 font-semibold sm:text-3xl sm:leading-9 text-nord-h2",
            {children}
        }
    }
}

/// Component for rendering the main slogan and Mac App Store link
#[component]
fn SloganAndMacStoreLink() -> Element {
    rsx! {
        // Slogan section
        div { class: "text-center lg:text-left",
            div {
                h2 { class: "text-4xl leading-10 font-semibold sm:text-5xl sm:leading-none lg:text-5xl text-nord-h1",
                    "The ultimate playground for your ideas"
                }
                p { class: "mt-4 max-w-md mx-auto text-lg sm:text-xl md:mt-5 md:max-w-3xl",
                    "Capture your top-of-mind using ready-to-go shelvs. Whether you're planning a trip, organizing your daily tasks, or brainstorming your next big idea, our Markdown-enabled shelves allow for a fun and efficient way to capture your thoughts without taking you out of your task."
                }
            }
        }

        // Mac App Store link section
        div { class: "mt-8 sm:max-w-lg sm:mx-auto text-center sm:text-center lg:text-left lg:mx-0",
            MacStoreLink {}
        }
    }
}

/// Component for rendering the Mac App Store link
#[component]
fn MacStoreLink() -> Element {
    rsx! {
        a { href: "https://testflight.apple.com/join/38OBZSRD",
            img {
                src: "/images/mac-app-store-badge.svg",
                alt: "Download Shelv on the Mac Test Flight",
                class: "home-app-store-buttons-mac",
                height: "64"
            }
        }
    }
}

// Define SVG paths as functions
fn twitter_svg_path() -> Element {
    rsx! {
        path { d: "M17.316,6.246c0.008,0.162,0.011,0.326,0.011,0.488c0,4.99-3.797,10.742-10.74,10.742c-2.133,0-4.116-0.625-5.787-1.697 c0.296,0.035,0.596,0.053,0.9,0.053c1.77,0,3.397-0.604,4.688-1.615c-1.651-0.031-3.046-1.121-3.526-2.621 c0.23,0.043,0.467,0.066,0.71,0.066c0.345,0,0.679-0.045,0.995-0.131c-1.727-0.348-3.028-1.873-3.028-3.703c0-0.016,0-0.031,0-0.047 c0.509,0.283,1.092,0.453,1.71,0.473c-1.013-0.678-1.68-1.832-1.68-3.143c0-0.691,0.186-1.34,0.512-1.898 C3.942,5.498,6.725,7,9.862,7.158C9.798,6.881,9.765,6.594,9.765,6.297c0-2.084,1.689-3.773,3.774-3.773 c1.086,0,2.067,0.457,2.756,1.191c0.859-0.17,1.667-0.484,2.397-0.916c-0.282,0.881-0.881,1.621-1.66,2.088 c0.764-0.092,1.49-0.293,2.168-0.594C18.694,5.051,18.054,5.715,17.316,6.246z" }
    }
}

fn github_svg_path() -> Element {
    rsx! {
        path { d: "M13.18,11.309c-0.718,0-1.3,0.807-1.3,1.799c0,0.994,0.582,1.801,1.3,1.801s1.3-0.807,1.3-1.801 C14.479,12.116,13.898,11.309,13.18,11.309z M17.706,6.626c0.149-0.365,0.155-2.439-0.635-4.426c0,0-1.811,0.199-4.551,2.08 c-0.575-0.16-1.548-0.238-2.519-0.238c-0.973,0-1.945,0.078-2.52,0.238C4.74,2.399,2.929,2.2,2.929,2.2 C2.14,4.187,2.148,6.261,2.295,6.626C1.367,7.634,0.8,8.845,0.8,10.497c0,7.186,5.963,7.301,7.467,7.301 c0.342,0,1.018,0.002,1.734,0.002c0.715,0,1.392-0.002,1.732-0.002c1.506,0,7.467-0.115,7.467-7.301 C19.2,8.845,18.634,7.634,17.706,6.626z M10.028,16.915H9.972c-3.771,0-6.709-0.449-6.709-4.115c0-0.879,0.31-1.693,1.047-2.369 c1.227-1.127,3.305-0.531,5.662-0.531c0.01,0,0.02,0,0.029,0c0.01,0,0.018,0,0.027,0c2.357,0,4.436-0.596,5.664,0.531 c0.735,0.676,1.045,1.49,1.045,2.369C16.737,16.466,13.8,16.915,10.028,16.915z M6.821,11.309c-0.718,0-1.3,0.807-1.3,1.799 c0,0.994,0.582,1.801,1.3,1.801c0.719,0,1.301-0.807,1.301-1.801C8.122,12.116,7.54,11.309,6.821,11.309z" }
    }
}
