use eframe::{
    egui::{Context, FontId, Frame, Label, RichText, TopBottomPanel, UiKind, pos2},
    epaint::Stroke,
};
use egui_taffy::{
    TuiBuilderLogic,
    taffy::{AlignItems, JustifyContent},
    tui,
};
use shared::Version;
use smallvec::SmallVec;

use crate::{
    app_actions::AppAction,
    app_state::VersionState,
    command::{CommandInstruction, CommandList},
    persistent_state::NoteId,
    taffy_styles::{StyleBuilder, flex_row},
    theme::{AppIcon, AppTheme},
    ui_components::{IconButton, IconButtonSize, apply_icon_btn_styling},
};

pub fn render_header_panel(
    ctx: &Context,
    theme: &AppTheme,
    command_list: &CommandList,
    selected_note: NoteId,
    is_window_pinned: bool,
    feedback_sent: bool,
    version_state: &VersionState,
    dev_tools_show: bool,
) -> SmallVec<[AppAction; 1]> {
    TopBottomPanel::top("top_panel")
        .show_separator_line(false)
        // .exact_height(theme.sizes.header_footer)
        .frame(Frame::new().fill(theme.colors.main_bg))
        .show(ctx, |ui| {
            ui.style_mut().wrap_mode = Some(eframe::egui::TextWrapMode::Extend);
            let mut resulting_actions: SmallVec<[AppAction; 1]> = Default::default();
            let sizes = &theme.sizes;
            ui.set_height(sizes.header_footer_height);
            let avail_width = ui.available_width();
            let avail_rect = ui.available_rect_before_wrap();
            ui.painter().line_segment(
                [avail_rect.left() + theme.sizes.s, avail_rect.right() - theme.sizes.s]
                    .map(|x| pos2(x, avail_rect.top() + sizes.header_footer_height)),
                Stroke::new(1.0, theme.colors.outline_fg),
            );
            // ui.set_min_size(vec2(avail_width, sizes.header_footer));

            let header_ui_id = ui.id().with("header");

            let tooltip_animation_id = header_ui_id.with("feedback_sent_tooltip");
            let tooltip_value =
                ctx.animate_bool_with_time(tooltip_animation_id, feedback_sent, 2.0);

            let tui_result = tui(ui, header_ui_id)
                .style(
                    flex_row()
                        .width(avail_width)
                        .height(sizes.header_footer_height)
                        .align_items(AlignItems::Center)
                        .justify_content(JustifyContent::SpaceBetween)
                        .padding_horizontal(sizes.s),
                )
                .show(|t| {
                    // Left section: Close button and title
                    t.style(flex_row().align_items(AlignItems::Center).gap(sizes.m))
                        .add(|t| {
                            // Close button
                            if t.ui_add(
                                IconButton::new(AppIcon::Close, theme)
                                    .size(IconButtonSize::Large)
                                    .tooltip("Hide Shelv", None),
                            )
                            .clicked()
                            {
                                resulting_actions.push(AppAction::HideApp);
                            }

                            // Title
                            t.ui_add(
                                Label::new(
                                    RichText::new(format!(
                                        "Shelv - {}",
                                        match selected_note {
                                            NoteId::Note(index)=>format!("note {}",index+1),
                                            NoteId::Settings=>"settings".to_string(),
                                            NoteId::ExternalFileId(external_file_id) => format!("TODO: external {:?}",external_file_id),
                                        }
                                    ))
                                    .color(theme.colors.subtle_text_color)
                                    .font(FontId {
                                        size: theme.fonts.size.normal,
                                        family: theme.fonts.family.bold.clone(),
                                    }),
                                )
                                .extend(),
                            );
                        });

                    // Right section: Feedback button, pin button, separator, and menu

                    t.style(flex_row().align_items(AlignItems::Center).gap(sizes.s))
                        .add(|t| {
                            // Update button
                            let update_btn = match version_state {
                                VersionState::UpToDate => None,
                                VersionState::UpdateAvailable(Version(version)) => Some(
                                    IconButton::new(AppIcon::Download, theme)
                                        .color(theme.colors.hyperlink_color)
                                        .size(IconButtonSize::Large)
                                        .text("Update Available")
                                        .tooltip(format!("v{} available, click to open App Store to update.", version), None),
                                ),
                                VersionState::RequiredUpdateAvailable(Version(version)) => Some(
                                    IconButton::new(AppIcon::Download, theme)
                                        .color(theme.colors.warn_fg_color)
                                        .size(IconButtonSize::Large)
                                        .text("Required Update")
                                        .tooltip(format!("Version ≥{} required, click to open App Store to update.", version), None),
                                ),
                            };

                            if let Some(update_btn) = update_btn {
                                if t.ui_add(update_btn).clicked() {
                                    resulting_actions.push(AppAction::AppUpdateClicked);
                                };
                            }

                            // TEST: Open external file button (temporary)
                            #[cfg(debug_assertions)]
                            if t.ui_add(
                                IconButton::new(AppIcon::Folder, theme)
                                    .size(IconButtonSize::Large)
                                    .tooltip("TEST: Open test.md", None),
                            )
                            .clicked()
                            {
                                use std::path::PathBuf;
                                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                                let readme_path = PathBuf::from(home).join("work/shelv/test.md");
                                resulting_actions.push(AppAction::OpenExternalFile(readme_path));
                            }

                            // Feedback button
                            if t.ui_add(
                                IconButton::new(AppIcon::Feedback, theme)
                                    .size(IconButtonSize::Large)
                                    .tooltip(
                                        "Send this note to report a bug or share feedback.",
                                        None,
                                    ),
                            )
                            .clicked()
                            {
                                resulting_actions.push(AppAction::OpenFeedbackWindow);
                            }

                            // Dev tools button (debug builds only)
                            #[cfg(debug_assertions)]
                            if t.ui_add(
                                IconButton::new(AppIcon::Bug, theme)
                                    .size(IconButtonSize::Large)
                                    .toggled(dev_tools_show)
                                    .tooltip(
                                        if dev_tools_show {
                                            "Hide dev tools"
                                        } else {
                                            "Show dev tools"
                                        },
                                        None,
                                    ),
                            )
                            .clicked()
                            {
                                resulting_actions.push(AppAction::ToggleDevTools);
                            }

                            // Pin button with tooltip and keyboard shortcut
                            if t.ui_add(
                                IconButton::new(AppIcon::Pin, theme)
                                    .size(IconButtonSize::Large)
                                    .toggled(is_window_pinned)
                                    .tooltip(
                                        if is_window_pinned {
                                            "Unpin window"
                                        } else {
                                            "Pin window"
                                        },
                                        command_list
                                            .find(CommandInstruction::PinWindow)
                                            .and_then(|cmd| cmd.shortcut),
                                    ),
                            )
                            .clicked()
                            {
                                resulting_actions
                                    .push(AppAction::SetWindowPinned(!is_window_pinned));
                            }

                            // Separator
                            t.label(
                                AppIcon::VerticalSeparator
                                    .render(sizes.toolbar_icon, theme.colors.outline_fg),
                            );

                            // Menu button - use ui_add_manual to embed the original menu_button
                            t.ui_finite(
                                |ui| {
                                    apply_icon_btn_styling(ui.style_mut());
                                    ui.menu_button(
                                        AppIcon::Menu.render(
                                            sizes.toolbar_icon,
                                            theme.colors.subtle_text_color,
                                        ),
                                        |ui| {
                                            ui.set_max_width(200.0);

                                            if ui
                                                .button(AppIcon::Tutorial.render_with_text(
                                                    (theme.colors.normal_text_color, theme.colors.normal_text_color),
                                                    "Start tutorial",
                                                    theme.fonts.size.normal,
                                                ))
                                                .clicked()
                                            {
                                                ui.close_kind(UiKind::Menu);
                                                resulting_actions.push(AppAction::StartTutorial);
                                            }

                                            ui.separator();

                                            for (icon, text, link) in [
                                                (
                                                    &AppIcon::HomeSite,
                                                    "Visit https://shelv.app",
                                                    "https://shelv.app",
                                                ),
                                                (
                                                    &AppIcon::Discord,
                                                    "Join our Discord",
                                                    include_str!("../../distribution/discord_invite.txt")
                                                        .trim(),
                                                ),
                                                (
                                                    &AppIcon::Github,
                                                    "Give as a Star or file an issue",
                                                    "https://github.com/twop/shelv",
                                                ),
                                            ] {
                                                if ui
                                                    .button(icon.render_with_text(
                                                        (theme.colors.normal_text_color, theme.colors.normal_text_color),
                                                        text,
                                                        theme.fonts.size.normal,
                                                    ))
                                                    .clicked()
                                                {
                                                    ui.close_menu();
                                                    resulting_actions.push(AppAction::OpenLink(
                                                        link.to_string(),
                                                    ));
                                                }
                                            }

                                            ui.separator();

                                            if ui
                                                .button(AppIcon::Folder.render_with_text(
                                                    (theme.colors.normal_text_color, theme.colors.normal_text_color),
                                                    "Open notes folder",
                                                    theme.fonts.size.normal,
                                                ))
                                                .clicked()
                                            {
                                                ui.close_menu();
                                                resulting_actions
                                                    .push(AppAction::OpenNotesInFinder);
                                            }
                                        },
                                    )
                                    .response
                                },
                                // |mut val, _ui| {
                                //     // Menu button can grow minimally
                                //     val.max_size = val.min_size;
                                //     val.infinite = egui::Vec2b::FALSE;
                                //     val
                                // },
                            );
                        });
                });

            // Handle feedback sent animation - show tooltip if feedback was recently sent
            if feedback_sent && tooltip_value < 1. {
                // We need to show the tooltip on the feedback button, but since we're outside the taffy context,
                // we'll just let the animation run for now. The tooltip will be handled by the button's hover state
                // in future iterations if needed.
            }

            resulting_actions
        })
        .inner
}
