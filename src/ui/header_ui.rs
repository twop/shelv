use eframe::egui::{Color32, Context, FontId, Label, RichText, Sides, Ui, UiKind, pos2};
use eframe::epaint::Stroke;
use shared::Version;
use smallvec::SmallVec;

use crate::app_ui::draw_debug_rect;
use crate::{
    app_actions::AppAction,
    app_state::VersionState,
    command::{CommandInstruction, CommandList},
    persistent_state::NoteId,
    theme::{AppIcon, AppTheme},
    ui_components::{IconButton, IconButtonSize, apply_icon_btn_styling},
};

pub fn render_header_panel(
    ui: &mut Ui,
    ctx: &Context,
    theme: &AppTheme,
    command_list: &CommandList,
    selected_note: NoteId,
    is_window_pinned: bool,
    feedback_sent: bool,
    version_state: &VersionState,
    dev_tools_show: bool,
) -> SmallVec<[AppAction; 1]> {
    let mut resulting_actions: SmallVec<[AppAction; 1]> = Default::default();
    let sizes = &theme.sizes;
    let avail_rect = ui.available_rect_before_wrap();

    // Draw bottom border line
    ui.painter().line_segment(
        [avail_rect.left_bottom(), avail_rect.right_bottom()],
        Stroke::new(1.0, theme.colors.outline_fg),
    );

    let header_ui_id = ui.id().with("header");
    let tooltip_animation_id = header_ui_id.with("feedback_sent_tooltip");
    let tooltip_value = ctx.animate_bool_with_time(tooltip_animation_id, feedback_sent, 2.0);

    // Use Sides widget to layout left (title) and right (buttons)
    let sides_result = Sides::new()
        .height(avail_rect.height())
        .shrink_left()
        .spacing(sizes.s)
        .show(
            ui,
            |ui| {
                // draw_debug_rect(ui, Color32::LIGHT_YELLOW);
                // Left side: Close button and title
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = sizes.m;

                    // Close button
                    if ui
                        .add(
                            IconButton::new(AppIcon::Close, theme)
                                .size(IconButtonSize::Large)
                                .tooltip("Hide Shelv", None),
                        )
                        .clicked()
                    {
                        resulting_actions.push(AppAction::HideApp);
                    }

                    // Title
                    ui.add(Label::new(
                        RichText::new(format!(
                            "Shelv - {}",
                            match selected_note {
                                NoteId::Note(index) => format!("note {}", index + 1),
                                NoteId::Settings => "settings".to_string(),
                                NoteId::ExternalFileId(external_file_id) =>
                                    format!("TODO: external {:?}", external_file_id),
                            }
                        ))
                        .color(theme.colors.subtle_text_color)
                        .font(FontId {
                            size: theme.fonts.size.normal,
                            family: theme.fonts.family.bold.clone(),
                        }),
                    ));
                });

                resulting_actions.clone()
            },
            |ui| {
                // Right side: buttons
                let mut right_actions: SmallVec<[AppAction; 1]> = SmallVec::new();

                // draw_debug_rect(ui, Color32::LIGHT_YELLOW);
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = sizes.xs;

                    // Menu button (rightmost, so added first due to Sides right-to-left layout)
                    apply_icon_btn_styling(ui.style_mut());
                    ui.menu_button(
                        AppIcon::Menu.render(sizes.toolbar_icon, theme.colors.subtle_text_color),
                        |ui| {
                            ui.set_max_width(200.0);

                            if ui
                                .button(AppIcon::Tutorial.render_with_text(
                                    (
                                        theme.colors.normal_text_color,
                                        theme.colors.normal_text_color,
                                    ),
                                    "Start tutorial",
                                    theme.fonts.size.normal,
                                ))
                                .clicked()
                            {
                                ui.close_kind(UiKind::Menu);
                                right_actions.push(AppAction::StartTutorial);
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
                                    include_str!("../../distribution/discord_invite.txt").trim(),
                                ),
                                (
                                    &AppIcon::Github,
                                    "Give as a Star or file an issue",
                                    "https://github.com/twop/shelv",
                                ),
                            ] {
                                if ui
                                    .button(icon.render_with_text(
                                        (
                                            theme.colors.normal_text_color,
                                            theme.colors.normal_text_color,
                                        ),
                                        text,
                                        theme.fonts.size.normal,
                                    ))
                                    .clicked()
                                {
                                    ui.close_menu();
                                    right_actions.push(AppAction::OpenLink(link.to_string()));
                                }
                            }

                            ui.separator();

                            if ui
                                .button(AppIcon::Folder.render_with_text(
                                    (
                                        theme.colors.normal_text_color,
                                        theme.colors.normal_text_color,
                                    ),
                                    "Open notes folder",
                                    theme.fonts.size.normal,
                                ))
                                .clicked()
                            {
                                ui.close_menu();
                                right_actions.push(AppAction::OpenNotesInFinder);
                            }
                        },
                    );

                    // Separator
                    ui.label(
                        AppIcon::VerticalSeparator
                            .render(sizes.toolbar_icon, theme.colors.outline_fg),
                    );

                    // Pin button with tooltip and keyboard shortcut
                    if ui
                        .add(
                            IconButton::new(AppIcon::Pin, theme)
                                .size(IconButtonSize::Large)
                                .toggled(is_window_pinned)
                                .tooltip(
                                    if is_window_pinned {
                                        "Unpin window"
                                    } else {
                                        "Pin window"
                                    },
                                    command_list.shortcut_for(CommandInstruction::PinWindow),
                                ),
                        )
                        .clicked()
                    {
                        right_actions.push(AppAction::SetWindowPinned(!is_window_pinned));
                    }

                    // Dev tools button (debug builds only)
                    #[cfg(debug_assertions)]
                    if ui
                        .add(
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
                        right_actions.push(AppAction::ToggleDevTools);
                    }

                    // Feedback button
                    if ui
                        .add(
                            IconButton::new(AppIcon::Feedback, theme)
                                .size(IconButtonSize::Large)
                                .tooltip("Send this note to report a bug or share feedback.", None),
                        )
                        .clicked()
                    {
                        right_actions.push(AppAction::OpenFeedbackWindow);
                    }

                    // TEST: Open external file button (temporary)
                    #[cfg(debug_assertions)]
                    if ui
                        .add(
                            IconButton::new(AppIcon::Folder, theme)
                                .size(IconButtonSize::Large)
                                .tooltip("TEST: Open test.md", None),
                        )
                        .clicked()
                    {
                        use std::path::PathBuf;
                        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                        let readme_path = PathBuf::from(home).join("work/shelv/test.md");
                        right_actions.push(AppAction::OpenExternalFile(readme_path));
                    }

                    // Update button (leftmost on right side, so added last)
                    let update_btn = match version_state {
                        VersionState::UpToDate => None,
                        VersionState::UpdateAvailable(Version(version)) => Some(
                            IconButton::new(AppIcon::Download, theme)
                                .color(theme.colors.hyperlink_color)
                                .size(IconButtonSize::Large)
                                .text("Update Available")
                                .tooltip(
                                    format!(
                                        "v{} available, click to open App Store to update.",
                                        version
                                    ),
                                    None,
                                ),
                        ),
                        VersionState::RequiredUpdateAvailable(Version(version)) => Some(
                            IconButton::new(AppIcon::Download, theme)
                                .color(theme.colors.warn_fg_color)
                                .size(IconButtonSize::Large)
                                .text("Required Update")
                                .tooltip(
                                    format!(
                                        "Version ≥{} required, click to open App Store to update.",
                                        version
                                    ),
                                    None,
                                ),
                        ),
                    };

                    if let Some(update_btn) = update_btn {
                        if ui.add(update_btn).clicked() {
                            right_actions.push(AppAction::AppUpdateClicked);
                        };
                    }
                });

                right_actions
            },
        );

    // Extract results from Sides
    let (left_actions, right_actions) = sides_result;

    resulting_actions.extend(left_actions);
    resulting_actions.extend(right_actions);

    // Handle feedback sent animation - show tooltip if feedback was recently sent
    if feedback_sent && tooltip_value < 1. {
        // We need to show the tooltip on the feedback button, but since we're outside the taffy context,
        // we'll just let the animation run for now. The tooltip will be handled by the button's hover state
        // in future iterations if needed.
    }

    resulting_actions
}
