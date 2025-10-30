use hyped::*;
use tailwind_fuse::*;

use crate::ui_components::{ThemeColor, WaveDirection, content, space, theme, wave};
use crate::{
    BorderStyle, ButtonHeight, ButtonStyle, ButtonVariant, DividerStyle, HoverState, IconSize,
    LinkStyle, SpacingSize, TextColor, TextStyle,
};

/// Complete footer section with wave, promo links, and credits
pub fn footer_section() -> Element {
    div((
        wave(WaveDirection::Down, ThemeColor::Light, SpacingSize::Medium),
        theme(
            ThemeColor::Dark,
            content(
                div((
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
                    {
                        let link_style = tw_join!(
                            "inline",
                            LinkStyle {
                                color: TextColor::Primary,
                                hover: HoverState::Underline
                            }
                            .to_class()
                        );
                        div((
                            p((
                                span("Done with "),
                                heart(),
                                " by Simon Korzunov ",
                                a(github_icon(IconSize::Small))
                                    .href("https://github.com/twop")
                                    .class(&link_style),
                                " ",
                                a(linkedin_icon(IconSize::Small))
                                    .href("https://www.linkedin.com/in/skorzunov")
                                    .class(&link_style),
                                " and Mirza Pasalic ",
                                a(github_icon(IconSize::Small))
                                    .href("https://github.com/mpasalic")
                                    .class(&link_style),
                                " ",
                                a(linkedin_icon(IconSize::Small))
                                    .href("https://www.linkedin.com/in/mpasalic")
                                    .class(&link_style),
                            ))
                            .class(&tw_join!("mt-3", TextStyle::SmallGeneralText)),
                            p((
                                "Shoot an email at ",
                                link_to("mailto:hi@shelv.app", "hi@shelv.app"),
                            ))
                            .class(&tw_join!("mt-3", TextStyle::SmallGeneralText)),
                            div(p((
                                "theme inspired by ",
                                link_to("https://www.nordtheme.com/", "Nord"),
                            ))
                            .class(&TextStyle::SmallGeneralText.as_class()))
                            .class("py-3 flex justify-end"),
                        ))
                    },
                ))
                .class("w-full px-4"),
            ),
        ),
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

fn heart() -> impl Render {
    let heart_color = TextColor::Red;
    let svg_content = include_str!("../assets/icons/heart.svg");
    let classes = format!("h-4 w-4 inline {}", heart_color.as_class());
    danger(svg_content.replace("<class>", &classes))
}

fn link_to(to: &str, text: &str) -> Element {
    let link_style = LinkStyle {
        color: TextColor::Primary,
        hover: HoverState::Underline,
    };
    a(text.to_string()).class(&link_style.to_class()).href(to)
}

fn github_icon(size: IconSize) -> impl Render {
    let svg_content = include_str!("../assets/icons/github.svg");
    let classes = tw_join!(size, "fill-current inline");
    danger(svg_content.replace("<class>", &classes))
}

fn linkedin_icon(size: IconSize) -> impl Render {
    let svg_content = include_str!("../assets/icons/linkedin.svg");
    let classes = tw_join!(size, "fill-current inline");
    danger(svg_content.replace("<class>", &classes))
}
