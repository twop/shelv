use eframe::{
    egui::{Context, FontFamily, FontId, Ui},
    epaint::Stroke,
};
use smallvec::SmallVec;
use smol_str::ToSmolStr;

use crate::{
    app_actions::AppAction,
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

    // ui.debug_paint_cursor();
    ui.horizontal_centered(|ui| {
        let items = [PickerItem {
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
        .chain(opened_files.iter().filter_map(|note_id| {
            match note_id {
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
                    }), // Ignore for now

                NoteId::Settings => None,
            }
        }));

        let available_rect = ui.available_rect_before_wrap();

        let picker_response = render_picker_items_refactored(
            selected,
            items,
            PickerVisualStyle {
                outline: Stroke::new(1.0, theme.colors.outline_fg),
            },
            PickerLayoutParams {
                gap: sizes.xs,                             // matches ui.spacing_mut().item_spacing.x
                bottom_rounding: sizes.toolbar_icon / 2.0, // half icon size for nice rounding
                top_rounding: sizes.s,                     // small rounding at top
                outline_margin: (sizes.xxs, 0.),           // small margins
                entire_available_rect: available_rect,
            },
            ui,
            theme,
        );

        if let Some(note_file) = picker_response.inner {
            actions.push(AppAction::SwitchToNote {
                note_file,
                via_shortcut: false,
            });
        }

        // Add spacer to push button to the right
        let available = ui.available_width();
        let button_width = sizes.toolbar_icon + sizes.s * 2.0;
        if available > button_width {
            ui.add_space(available - button_width);
        }

        // Separator
        ui.label(AppIcon::VerticalSeparator.render(sizes.toolbar_icon, theme.colors.outline_fg));

        // Right section: Open file button
        if ui
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
    });

    actions
}
