use eframe::egui::{
    self, Checkbox, FontId, FontSelection, Id, Label, Modifiers, RichText, TextEdit, Ui,
};
use egui_taffy::{
    TuiBuilderLogic,
    taffy::{AlignContent, AlignItems},
    tui,
};

use crate::{
    settings_parsing::format_mac_shortcut_with_symbols,
    taffy_styles::{StyleBuilder, flex_column, flex_row, style},
    theme::{AppIcon, AppTheme},
};

#[derive(Debug, PartialEq, Eq, Copy, Clone, serde::Serialize)]
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

    pub fn show(mut self, ui: &mut Ui) -> Option<FeedbackResult> {
        let sizes = &self.theme.sizes;
        let colors = &self.theme.colors;
        let fonts = &self.theme.fonts;

        ui.set_width(ui.available_width());
        ui.set_height(ui.available_height());

        let avail_rect = ui.available_rect_before_wrap();

        let bold = FontId {
            size: fonts.size.normal,
            family: fonts.family.extra_bold.clone(),
        };
        let italic = FontId {
            size: fonts.size.normal,
            family: fonts.family.italic.clone(),
        };

        let id = Id::new("feedback form");
        // let is_focused = ui.memory(|mem| mem.has_focus(id));
        let _resp = ui.scope(|ui| {
            // Required because the flex layout trips out from any wrapping
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

            tui(ui, id)
                .style(
                    flex_column()
                        .width(avail_rect.width())
                        .height(avail_rect.height())
                        .align_content(AlignContent::Stretch)
                        .gap(sizes.s)
                        .padding(sizes.xs),
                )
                .reserve_available_space()
                .show(|t| {
                    t.ui_add(Label::new(
                        RichText::new("We appreciate you and your feedback <3")
                            .font(bold.clone())
                            .color(colors.subtle_text_color),
                    ));

                    // t.label(format!("is focused = {is_focused}"));
                    t.add_empty();
                    t.add_empty();
                    // t.separator();
                    t.style(flex_row().gap(sizes.xs).align_items(AlignItems::End))
                        .add(|t| {
                            t.label(
                                RichText::new("Contact Info")
                                    .font(bold.clone())
                                    .color(colors.subtle_text_color),
                            );
                            t.label(
                                RichText::new("(optional)")
                                    .font(italic.clone())
                                    .color(colors.subtle_text_color),
                            );
                        });

                    t.ui_add(
                        TextEdit::singleline(&mut self.data.contact_info)
                            .hint_text(format!("Name (your@email.com / discord@ / etc)"))
                            .hint_text_font(FontSelection::FontId(italic.clone()))
                            .desired_width(f32::INFINITY),
                    );
                    t.add_empty();
                    t.add_empty();
                    // t.separator();

                    t.label(
                        RichText::new("You are enjoying Shelv, aren't you?")
                            .font(bold.clone())
                            .color(colors.subtle_text_color),
                    );

                    t.style(flex_row().gap(sizes.s)).add(|t| {
                        let [positive_clicked, negative_clicked] = [
                            (FeedbackType::Positive, AppIcon::Feedback),
                            (FeedbackType::Negative, AppIcon::NegFeedback),
                        ]
                        .map(|(feedback_type, icon)| {
                            let selected = self.data.feedback_type == Some(feedback_type);
                            t.selectable(selected, |t| {
                                t.label(icon.render(
                                    sizes.xl,
                                    match selected {
                                        true => colors.button_pressed_fg,
                                        false => colors.subtle_text_color,
                                    },
                                ));
                            })
                            .clicked()
                        });

                        if positive_clicked {
                            self.data.feedback_type = Some(FeedbackType::Positive);
                        } else if negative_clicked {
                            self.data.feedback_type = Some(FeedbackType::Negative);
                        }
                    });
                    t.add_empty();
                    t.add_empty();
                    // t.separator();

                    t.style(
                        flex_column()
                            .gap(sizes.s)
                            .align_items(AlignItems::Stretch)
                            .grow(1.),
                    )
                    .add(|t| {
                        t.label(
                            RichText::new("What's on your mind?")
                                .font(bold.clone())
                                .color(colors.subtle_text_color),
                        );

                        t.ui_add(Checkbox::new(
                            &mut self.data.include_current_note,
                            RichText::new("Include current note")
                                .font(FontId {
                                    size: fonts.size.normal,
                                    family: fonts.family.normal.clone(),
                                })
                                .color(colors.subtle_text_color),
                        ));

                        t.style(flex_column().grow(1.)).ui_add(
                            TextEdit::multiline(&mut self.data.feedback_text)
                                .hint_text(format!(
                                    "Describe any issues you encountered, or any general feedback."
                                ))
                                .hint_text_font(FontSelection::FontId(italic.clone()))
                                .desired_width(f32::INFINITY)
                                .desired_rows(6),
                        );
                    });

                    t.add_empty();
                    t.add_empty();
                    // t.separator();

                    t.label(
                        RichText::new(format!(
                            "Hint: '{}' to send, 'Esc' to cancel",
                            format_mac_shortcut_with_symbols(egui::KeyboardShortcut::new(
                                Modifiers::COMMAND,
                                egui::Key::Enter,
                            ))
                        ))
                        .font(italic.clone())
                        .color(colors.subtle_text_color),
                    );
                    t.style(flex_row().gap(sizes.s)).add(|t| {
                        let send_btn_res = t.style(style().padding(sizes.xs)).button(|t| {
                            t.label(AppIcon::Send.render_with_text(
                                fonts.size.normal,
                                colors.md_body,
                                "Send Feedback",
                            ));
                        });
                        if send_btn_res.clicked() {
                            self.result = Some(FeedbackResult::SubmitFeedback);
                        }

                        let cancel_btn_res = t.style(style().padding(sizes.xs)).button(|t| {
                            t.label(AppIcon::Close.render_with_text(
                                fonts.size.normal,
                                colors.md_body,
                                "Cancel",
                            ));
                        });

                        if cancel_btn_res.clicked() {
                            self.result = Some(FeedbackResult::Cancel);
                        }
                        //
                    })
                });
        });

        self.result
    }
}
