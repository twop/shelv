use hyped::*;
use tailwind_fuse::*;

use crate::{BackgroundColor, SpacingSize, TextColor};

// Constants for wave paths (moved from main.rs)
const UP_WAVE_PATH: &str = concat!(
    "M0,128L120,144C240,160,480,192,720,208C960,224,1200,224,1320,224L1440,224L1440,320L1320,320",
    "C1200,320,960,320,720,320C480,320,240,320,120,320L0,320Z"
);
const DOWN_WAVE_PATH: &str = concat!(
    "M0,224L80,186.7C160,149,320,75,480,53.3C640,32,800,64,960,85.3C1120,107,1280,117,1360,122.7L1440,128",
    "L1440,320L1360,320C1280,320,1120,320,960,320C800,320,640,320,480,320C320,320,160,320,80,320L0,320Z"
);

// Theme management enums
#[derive(Clone)]
pub enum ThemeColor {
    Dark,
    Light,
}

// Style struct for theme
#[derive(TwClass)]
#[tw(class = "")]
pub struct ThemeStyle {
    pub bg: BackgroundColor,
    pub text: TextColor,
}

/// Direction for wave (upward or downward)
#[derive(Clone, Copy)]
pub enum WaveDirection {
    Up,
    Down,
}

/// A themed section container with background and text color
pub fn theme(color: ThemeColor, children: impl Render + 'static) -> Element {
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

/// Content wrapper with max width and padding
pub fn content(children: impl Render + 'static) -> Element {
    div(children).class("mx-auto px-4 sm:px-6 max-w-4xl")
}

/// Wave divider between sections
pub fn wave(direction: WaveDirection, top_color: ThemeColor, size: SpacingSize) -> Element {
    let path = match direction {
        WaveDirection::Up => UP_WAVE_PATH,
        WaveDirection::Down => DOWN_WAVE_PATH,
    };

    let bg_color = match top_color {
        ThemeColor::Dark => BackgroundColor::Dark,
        ThemeColor::Light => BackgroundColor::Light,
    };

    let fill_color = match top_color {
        ThemeColor::Dark => "var(--color-nord0-dark)",
        ThemeColor::Light => "var(--color-nord0-darker)",
    };

    div(div(danger(&format!(
        r#"<svg width="100%" height="100%" viewBox="0 0 1440 160" preserveAspectRatio="none" xmlns="http://www.w3.org/2000/svg">
            <defs>
                <filter id="shadow">
                    <feDropShadow dx="0" dy="-20" std-deviation="5"/>
                </filter>
            </defs>
            <g transform="scale(1, 0.5)">
                <path fill="{}" d="{}"/>
            </g>
        </svg>"#,
        fill_color, path
    )))
    .class("w-full h-full"))
    .class(&tw_join!("w-full", bg_color, size))
}

/// Spacer element with configurable size
pub fn space(size: SpacingSize) -> Element {
    div("").class(&size.as_class())
}

/// Complete page shell with theme and content wrapping
pub fn page_shell(theme_color: ThemeColor, children: impl Render + 'static) -> Element {
    theme(theme_color, content(children))
}
