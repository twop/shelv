use eframe::{
    egui::{Color32, Context, FontFamily, FontId, Layout, ScrollArea, Ui, UiBuilder, vec2},
    emath::Align,
    epaint::Stroke,
};
use smallvec::SmallVec;
use smol_str::ToSmolStr;

use crate::{
    app_actions::AppAction,
    app_ui::draw_debug_rect,
    command::{CommandInstruction, CommandList},
    persistent_state::{ExternalFile, NoteId},
    theme::{AppIcon, AppTheme},
    ui::picker::{
        PickerItem, PickerItemKind, PickerLayoutParams, PickerVisualStyle,
        render_picker_items_refactored,
    },
    ui_components::{IconButton, IconButtonSize},
};

pub fn render_footer_panel(
    ui: &mut Ui,
    selected: NoteId,
    opened_files: SmallVec<[NoteId; 8]>,
    external_files: &[ExternalFile],
    command_list: &CommandList,
    ctx: &Context,
    theme: &AppTheme,
) -> SmallVec<[AppAction; 1]> {
    let mut actions = SmallVec::new();
    let sizes = &theme.sizes;

    let original_available_space = ui.available_rect_before_wrap();

    let space_after_right_side = {
        // STEP 1: Render right side first using right-to-left layout
        // Allocate UI for the right section and render it first
        let mut right_side_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(ui.available_rect_before_wrap())
                .layout(Layout::right_to_left(Align::Center)),
        );

        // draw_debug_rect(&right_side_ui, Color32::LIGHT_YELLOW.gamma_multiply(0.5));
        // Right section: Open file button
        if right_side_ui
            .add(
                IconButton::new(AppIcon::Folder, theme)
                    .size(IconButtonSize::Large)
                    .tooltip(
                        "Open file",
                        command_list
                            .find(CommandInstruction::OpenFileDialog)
                            .and_then(|cmd| cmd.shortcut),
                    ),
            )
            .clicked()
        {
            actions.push(AppAction::OpenFileDialog);
        }

        // Separator
        right_side_ui
            .label(AppIcon::VerticalSeparator.render(sizes.toolbar_icon, theme.colors.outline_fg));

        // draw_debug_rect(&right_side_ui, Color32::LIGHT_GREEN.gamma_multiply(0.5));
        right_side_ui.available_rect_before_wrap()
    };

    let mut left_side = ui.new_child(
        UiBuilder::new()
            .max_rect(space_after_right_side)
            .layout(Layout::left_to_right(Align::Center)),
    );

    {
        let mut ui = &mut left_side;
        // draw_debug_rect(&ui, Color32::LIGHT_YELLOW.gamma_multiply(0.5));

        let total_available = ui.available_width();

        // Calculate the width needed for right section (folder button + separator)
        let button_width = sizes.toolbar_icon + sizes.s * 2.0;
        let separator_width = sizes.toolbar_icon + sizes.s;
        let right_section_width = button_width + separator_width;

        // STEP 2: Render default items (settings and notes)
        // Build the default items list
        let default_items: SmallVec<[PickerItem<NoteId>; 5]> = [PickerItem {
            tooltip: {
                let tooltip_text = "Settings";
                command_list
                    .find(CommandInstruction::SwitchToSettings)
                    .and_then(|cmd| cmd.shortcut)
                    .map(|shortcut| format!("{} {}", tooltip_text, ctx.format_shortcut(&shortcut)))
                    .unwrap_or_else(|| tooltip_text.to_string())
            },
            kind: PickerItemKind::FontIcon(AppIcon::Settings, IconButtonSize::Large),
            data: NoteId::Settings,
        }]
        .into_iter()
        .chain(opened_files.iter().filter_map(|note_id| match note_id {
            NoteId::Note(index) => {
                let index = *index;
                let cmd = command_list.find(CommandInstruction::SwitchToNote(index as u8));
                let tooltip = match cmd.and_then(|cmd| cmd.shortcut) {
                    Some(shortcut) => {
                        format!("Shelf {}", ctx.format_shortcut(&shortcut))
                    }
                    None => format!("Shelf {}", index + 1),
                };

                Some(PickerItem {
                    tooltip,
                    kind: PickerItemKind::FontIcon(
                        match index {
                            0 => AppIcon::One,
                            1 => AppIcon::Two,
                            2 => AppIcon::Three,
                            3 => AppIcon::Four,
                            _ => AppIcon::More,
                        },
                        IconButtonSize::Large,
                    ),
                    data: *note_id,
                })
            }
            NoteId::Settings => None,
            NoteId::ExternalFileId(_) => None, // Handle external files separately
        }))
        .collect();

        let available_rect = ui.available_rect_before_wrap();

        let picker_response = render_picker_items_refactored(
            selected,
            default_items.into_iter(),
            PickerVisualStyle {
                outline: Stroke::new(1.0, theme.colors.outline_fg),
            },
            PickerLayoutParams {
                gap: sizes.xs,                             // matches ui.spacing_mut().item_spacing.x
                bottom_rounding: sizes.toolbar_icon / 2.0, // half icon size for nice rounding
                top_rounding: sizes.s,                     // small rounding at top
                outline_margin: (sizes.xxs, 0.),           // small margins
                entire_available_rect: original_available_space,
                height_bump: sizes.xxs,
            },
            &mut ui,
            theme,
        );

        if let Some(note_file) = picker_response.inner {
            actions.push(AppAction::SwitchToNote {
                note_file,
                via_shortcut: false,
            });
        }

        // STEP 3: Render external files in a horizontal scroll container
        let external_file_items: SmallVec<[PickerItem<NoteId>; 8]> = opened_files
            .iter()
            .filter_map(|note_id| match note_id {
                NoteId::ExternalFileId(external_id) => external_files
                    .iter()
                    .find(|ext_file| ext_file.id == *external_id)
                    .map(|ext_file| PickerItem {
                        tooltip: ext_file.path.to_string_lossy().to_string(),
                        kind: PickerItemKind::ItemName(
                            ext_file
                                .path
                                .file_name()
                                .map(|name| name.to_string_lossy().to_smolstr())
                                .unwrap_or_else(|| external_id.to_6_digit_smol_str()),
                            FontId::new(theme.fonts.size.normal, FontFamily::Proportional),
                        ),
                        data: *note_id,
                    }),
                _ => None,
            })
            .collect();

        if !external_file_items.is_empty() {
            // Use horizontal scroll area for external files
            ScrollArea::horizontal()
                .id_salt("footer_external_files")
                .scroll_bar_visibility(
                    eframe::egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                )
                .show(&mut ui, |ui| {
                    let available_rect = ui.available_rect_before_wrap();

                    let external_picker_response = render_picker_items_refactored(
                        selected,
                        external_file_items.into_iter(),
                        PickerVisualStyle {
                            outline: Stroke::new(1.0, theme.colors.outline_fg),
                        },
                        PickerLayoutParams {
                            gap: sizes.xs,
                            bottom_rounding: sizes.toolbar_icon / 2.0,
                            top_rounding: sizes.s,
                            outline_margin: (sizes.xxs, 0.),
                            entire_available_rect: original_available_space,
                            height_bump: sizes.xxs,
                        },
                        ui,
                        theme,
                    );

                    if let Some(note_file) = external_picker_response.inner {
                        actions.push(AppAction::SwitchToNote {
                            note_file,
                            via_shortcut: false,
                        });
                    }
                });
        }
    };

    actions
}
