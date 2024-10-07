use eframe::egui::{
    self, pos2, vec2, Button, Checkbox, Color32, FontId, InnerResponse, Label, Response, RichText,
    Stroke, TextEdit, Ui, Widget,
};
use egui_flex::*;

use crate::theme::{AppIcon, AppTheme, ColorManipulation};

#[derive(Debug, Copy, Clone, serde::Serialize)]
pub enum FeedbackType {
    Positive,
    Negative,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FeedbackData {
    pub feedback_text: String,
    pub contact_info: String,
    pub include_current_note: bool,
    pub feedback_type: Option<FeedbackType>,
}

impl Default for FeedbackData {
    fn default() -> Self {
        Self {
            feedback_text: String::new(),
            contact_info: String::new(),
            include_current_note: true,
            feedback_type: None,
        }
    }
}

pub enum FeedbackResult {
    Cancel,
    SubmitFeedback,
}

pub struct Feedback<'a> {
    theme: &'a AppTheme,
    data: &'a mut FeedbackData,
    result: Option<FeedbackResult>,
}

impl<'a> Feedback<'a> {
    pub fn new(theme: &'a AppTheme, data: &'a mut FeedbackData) -> Self {
        Self {
            theme,
            data,
            result: None,
        }
    }

    pub fn show(mut self, ui: &mut Ui) -> InnerResponse<Option<FeedbackResult>> {
        let window_bg = self.theme.colors.main_bg.shade(0.9);

        ui.set_width(ui.available_width());
        ui.set_height(ui.available_height());

        let avail_rect = ui.available_rect_before_wrap();

        ui.painter().line_segment(
            [avail_rect.left(), avail_rect.right()].map(|x| pos2(x, avail_rect.top() + 22.)),
            Stroke::new(1.0, self.theme.colors.outline_fg),
        );

        fn flex_spacer(flex: &mut FlexInstance) -> InnerResponse<()> {
            flex.add_simple(item().grow(1.0).basis(0.0), |_| {})
        }

        fn add_flex_justified<W: egui::Widget>(
            flex: &mut FlexInstance,
            justification: FlexJustify,
            widgets: Vec<W>,
        ) -> InnerResponse<()>
        where
            W: FlexWidget,
        {
            let widgets: Vec<_> = match justification {
                FlexJustify::Start => widgets.into_iter().map(Some).chain([None]).collect(),
                FlexJustify::End => [None]
                    .into_iter()
                    .chain(widgets.into_iter().map(Some))
                    .collect(),
                FlexJustify::Center => [None]
                    .into_iter()
                    .chain(widgets.into_iter().map(Some))
                    .chain(None)
                    .collect(),
                FlexJustify::SpaceBetween => {
                    Iterator::intersperse_with(widgets.into_iter().map(Some), || None)
                        .collect::<Vec<_>>()
                }
                FlexJustify::SpaceAround => todo!(),
                FlexJustify::SpaceEvenly => [None]
                    .into_iter()
                    .chain(Iterator::intersperse_with(
                        widgets.into_iter().map(Some),
                        || None,
                    ))
                    .chain(None)
                    .collect::<Vec<_>>(),
            };

            flex.add_flex(item(), Flex::horizontal(), |flex| {
                for widget_fn in widgets {
                    match widget_fn {
                        Some(widget) => {
                            flex.add(item(), widget);
                        }
                        None => {
                            flex.add_simple(item().grow(1.0).basis(0.0), |_| {});
                        }
                    }
                }
            })
        }

        let resp = ui.scope(|ui| {
            // Required because the flex layout trips out from any wrapping
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

            Flex::vertical()
                .align_content(egui_flex::FlexAlignContent::Stretch)
                .gap(vec2(8., 16.))
                .show(ui, |flex| {
                    flex.add_flex(item(), Flex::horizontal(), |flex| {
                        flex_spacer(flex);
                        flex.add(
                            item().grow(1.0),
                            Label::new(
                                RichText::new("Feedback")
                                    .font(FontId {
                                        size: self.theme.fonts.size.normal,
                                        family: self.theme.fonts.family.extra_bold.clone(),
                                    })
                                    .color(self.theme.colors.subtle_text_color),
                            ),
                        );
                        flex.add_flex(item().grow(1.0).basis(0.0), Flex::horizontal(), |flex| {
                            flex_spacer(flex);
                            let close_btn = flex
                                .add(
                                    item(),
                                    Button::new(AppIcon::Close.render_thin(
                                        self.theme.sizes.toolbar_icon,
                                        self.theme.colors.subtle_text_color,
                                    ))
                                    .fill(window_bg),
                                )
                                .inner;

                            if close_btn.clicked() {
                                self.result = Some(FeedbackResult::Cancel)
                            }
                        });
                    });

                    flex.add_flex(
                        item(),
                        Flex::vertical().align_content(egui_flex::FlexAlignContent::Stretch),
                        |flex| {
                            flex.add_flex(
                                item(),
                                Flex::horizontal().align_items(egui_flex::FlexAlign::End),
                                |flex| {
                                    flex.add(
                                        item(),
                                        Label::new(
                                            RichText::new("Name / Email")
                                                .font(FontId {
                                                    size: 12.,
                                                    family: self.theme.fonts.family.bold.clone(),
                                                })
                                                .color(self.theme.colors.subtle_text_color),
                                        ),
                                    );
                                    flex_spacer(flex);
                                },
                            );

                            flex.add(
                                item().grow(1.),
                                TextEdit::singleline(&mut self.data.contact_info),
                            );
                        },
                    );

                    flex.add_flex(
                        item().grow(1.),
                        Flex::vertical().align_content(egui_flex::FlexAlignContent::Stretch),
                        |flex| {
                            flex.add_flex(
                                item(),
                                Flex::horizontal().align_items(egui_flex::FlexAlign::End),
                                |flex| {
                                    flex.add(
                                        item(),
                                        Label::new(
                                            RichText::new("Feedback")
                                                .font(FontId {
                                                    size: 12.,
                                                    family: self.theme.fonts.family.bold.clone(),
                                                })
                                                .color(self.theme.colors.subtle_text_color),
                                        ),
                                    );

                                    flex_spacer(flex);

                                    flex.add(
                                        item(),
                                        Checkbox::new(
                                            &mut self.data.include_current_note,
                                            RichText::new("Include current note")
                                                .font(FontId {
                                                    size: 10.,
                                                    family: self.theme.fonts.family.normal.clone(),
                                                })
                                                .color(self.theme.colors.subtle_text_color),
                                        ),
                                    );
                                },
                            );

                            flex.add(
                                item().grow(1.),
                                TextEdit::multiline(&mut self.data.feedback_text),
                            );
                        },
                    );

                    flex.add_flex(
                        item(),
                        Flex::horizontal().align_items(egui_flex::FlexAlign::Center),
                        |flex| {
                            flex.add_flex(item().grow(1.).basis(1.), Flex::horizontal(), |flex| {
                                flex.add(
                                    item(),
                                    Label::new(
                                        RichText::new("Experience?")
                                            .font(FontId {
                                                size: 12.,
                                                family: self.theme.fonts.family.bold.clone(),
                                            })
                                            .color(self.theme.colors.subtle_text_color),
                                    ),
                                );
                            });

                            flex.add_flex(
                                item().grow(1.),
                                Flex::horizontal().align_items(egui_flex::FlexAlign::Center),
                                |flex| {
                                    flex_spacer(flex);

                                    let happy_btn = flex
                                        .add(
                                            item(),
                                            Button::new(AppIcon::Feedback.render(
                                                self.theme.sizes.xl,
                                                match self.data.feedback_type {
                                                    Some(FeedbackType::Positive) => {
                                                        self.theme.colors.button_pressed_fg
                                                    }
                                                    _ => self.theme.colors.subtle_text_color,
                                                },
                                            ))
                                            .fill(window_bg),
                                        )
                                        .inner;

                                    let sad_btn = flex
                                        .add(
                                            item(),
                                            Button::new(AppIcon::NegFeedback.render(
                                                self.theme.sizes.xl,
                                                match self.data.feedback_type {
                                                    Some(FeedbackType::Negative) => {
                                                        self.theme.colors.button_pressed_fg
                                                    }
                                                    _ => self.theme.colors.subtle_text_color,
                                                },
                                            ))
                                            .fill(window_bg),
                                        )
                                        .inner;

                                    if happy_btn.clicked() {
                                        self.data.feedback_type = Some(FeedbackType::Positive);
                                    } else if sad_btn.clicked() {
                                        self.data.feedback_type = Some(FeedbackType::Negative);
                                    }

                                    flex_spacer(flex);
                                },
                            );

                            flex_spacer(flex);
                        },
                    );

                    flex.add_flex(
                        item(),
                        Flex::horizontal().align_items(egui_flex::FlexAlign::Center),
                        |flex| {
                            flex_spacer(flex);

                            let send_btn_res = flex.add(
                                item(),
                                Button::new(AppIcon::Send.render_with_text(
                                    12.,
                                    self.theme.colors.md_body,
                                    "Send Feedback",
                                ))
                                .fill(Color32::TRANSPARENT),
                            );
                            let send_btn = send_btn_res.inner;

                            if send_btn.clicked() {
                                self.result = Some(FeedbackResult::SubmitFeedback);
                            }
                        },
                    );
                });
        });

        InnerResponse {
            inner: self.result,
            response: resp.response,
        }
    }
}

impl<'a> Widget for Feedback<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.show(ui).response
    }
}
