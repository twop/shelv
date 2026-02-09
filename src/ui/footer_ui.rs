use eframe::{
    egui::{Color32, Context, FontFamily, FontId, RichText, ScrollArea, Sides, Ui, vec2},
    emath::Align,
    epaint::{
        PathShape, PathStroke, Pos2, Rect, Shape, Stroke, pos2,
        tessellator::path::add_circle_quadrant,
    },
};
use smallvec::SmallVec;
use smol_str::ToSmolStr;

use crate::{
    app_actions::AppAction,
    command::{CommandInstruction, CommandList},
    persistent_state::{ExternalFile, NoteId},
    theme::{AppIcon, AppTheme},
    ui_components::{IconButton, IconButtonSize, apply_icon_btn_styling},
};

// ============================================================================
// Picker structures and functions
// ============================================================================

#[derive(Debug, Clone)]
pub struct PickerLayoutParams {
    pub gap: f32,
    pub bottom_rounding: f32,
    pub top_rounding: f32,
    pub outline_margin: (f32, f32),
    pub entire_available_rect: Rect,
}

struct SelectionOutlineDesc {
    bottom_radius: f32,
    /// Top rounding radii: (left, right)
    top_radious: (f32, f32),
    item_width: f32,
    item_height: f32,
    /// That (x,y) margins to be applied to the outline
    margin: (f32, f32),
}

fn selection_outline(
    SelectionOutlineDesc {
        bottom_radius,
        top_radious,
        item_width,
        item_height,
        margin,
    }: SelectionOutlineDesc,
) -> Vec<Pos2> {
    let mut path: Vec<Pos2> = vec![];
    let (margin_x, margin_y) = margin;
    let (top_left_radius, top_right_radius) = top_radious;

    let top_left_circle_center_x = item_width / 2.0 + margin_x + top_left_radius;
    let top_right_circle_center_x = item_width / 2.0 + margin_x + top_right_radius;

    let bottom_segment_width = (item_width - 2. * bottom_radius).max(0.0);
    let bottom_circle_center_y = item_height - bottom_radius + margin_y;
    let bottom_circle_center_x = bottom_segment_width / 2.0 + margin_x;

    // top left rounding
    add_circle_quadrant(
        &mut path,
        pos2(-top_left_circle_center_x, top_left_radius),
        top_left_radius,
        3.0,
    );

    // TODO: avoid extra allocation here, just remember the index and then swap
    // original smile for circular items
    let mut smile: Vec<Pos2> = vec![];

    // right bottom
    add_circle_quadrant(
        &mut smile,
        pos2(bottom_circle_center_x, bottom_circle_center_y),
        bottom_radius,
        0.0,
    );

    // left bottom
    add_circle_quadrant(
        &mut smile,
        pos2(-bottom_circle_center_x, bottom_circle_center_y),
        bottom_radius,
        1.0,
    );

    // it has to be in reverse order, because it goes counterclockwise
    path.extend(smile.into_iter().rev());

    // top right rounding
    add_circle_quadrant(
        &mut path,
        pos2(top_right_circle_center_x, top_right_radius),
        top_right_radius,
        2.0,
    );

    path
}

/// Render default picker (Settings + Notes)
fn render_default_notes_picker(
    selected: NoteId,
    opened_files: &SmallVec<[NoteId; 8]>,
    command_list: &CommandList,
    layout_params: &PickerLayoutParams,
    ui: &mut Ui,
    ctx: &Context,
    theme: &AppTheme,
) -> (SmallVec<[AppAction; 1]>, Option<Rect>, Option<usize>) {
    let mut actions = SmallVec::new();
    let mut selected_item_rect: Option<Rect> = None;
    let mut selected_item_index: Option<usize> = None;

    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = layout_params.gap;
        ui.add_space(layout_params.outline_margin.0);

        // Settings button
        let is_settings_selected = selected == NoteId::Settings;
        let settings_button = IconButton::new(AppIcon::Settings, theme)
            .size(IconButtonSize::Large)
            .toggled(is_settings_selected)
            .tooltip(
                {
                    let tooltip_text = "Settings";
                    command_list
                        .find(CommandInstruction::SwitchToSettings)
                        .and_then(|cmd| cmd.shortcut)
                        .map(|shortcut| {
                            format!("{} {}", tooltip_text, ctx.format_shortcut(&shortcut))
                        })
                        .unwrap_or_else(|| tooltip_text.to_string())
                },
                None,
            );

        let settings_response = ui.add(settings_button);
        if settings_response.clicked() && !is_settings_selected {
            actions.push(AppAction::SwitchToNote {
                note_file: NoteId::Settings,
                via_shortcut: false,
            });
        }
        if is_settings_selected {
            selected_item_rect = Some(settings_response.rect);
            selected_item_index = Some(0);
        }

        // Note buttons
        let mut current_index = 1; // Settings is index 0
        for note_id in opened_files.iter() {
            if let NoteId::Note(index) = note_id {
                let index = *index;
                let is_selected = selected == *note_id;
                let cmd = command_list.find(CommandInstruction::SwitchToNote(index as u8));
                let tooltip = match cmd.and_then(|cmd| cmd.shortcut) {
                    Some(shortcut) => format!("Shelf {}", ctx.format_shortcut(&shortcut)),
                    None => format!("Shelf {}", index + 1),
                };

                let icon = match index {
                    0 => AppIcon::One,
                    1 => AppIcon::Two,
                    2 => AppIcon::Three,
                    3 => AppIcon::Four,
                    _ => AppIcon::More,
                };

                let button = IconButton::new(icon, theme)
                    .size(IconButtonSize::Large)
                    .toggled(is_selected)
                    .tooltip(tooltip, None);

                let button_response = ui.add(button);
                if button_response.clicked() && !is_selected {
                    actions.push(AppAction::SwitchToNote {
                        note_file: *note_id,
                        via_shortcut: false,
                    });
                }

                if is_selected {
                    selected_item_rect = Some(button_response.rect);
                    selected_item_index = Some(current_index);
                }

                current_index += 1;
            }
        }
    });

    (actions, selected_item_rect, selected_item_index)
}

/// Render external files picker
fn render_external_files_picker(
    selected: NoteId,
    opened_files: &SmallVec<[NoteId; 8]>,
    external_files: &[ExternalFile],
    layout_params: &PickerLayoutParams,
    ui: &mut Ui,
    theme: &AppTheme,
) -> (SmallVec<[AppAction; 1]>, Option<Rect>) {
    let mut actions = SmallVec::new();
    let mut selected_item_rect: Option<Rect> = None;

    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = layout_params.gap;
        ui.add_space(layout_params.outline_margin.0);

        for note_id in opened_files.iter() {
            if let NoteId::ExternalFileId(external_id) = note_id {
                if let Some(ext_file) = external_files.iter().find(|ef| ef.id == *external_id) {
                    let is_selected = selected == *note_id;
                    let filename = ext_file
                        .path
                        .file_name()
                        .map(|name| name.to_string_lossy().to_smolstr())
                        .unwrap_or_else(|| external_id.to_6_digit_smol_str());

                    apply_icon_btn_styling(ui.style_mut());
                    let button_response = ui.button(RichText::new(filename.as_str()).font(
                        FontId::new(theme.fonts.size.normal, FontFamily::Proportional),
                    ));

                    if button_response.clicked() && !is_selected {
                        actions.push(AppAction::SwitchToNote {
                            note_file: *note_id,
                            via_shortcut: false,
                        });
                    }

                    if is_selected {
                        selected_item_rect = Some(button_response.rect);
                    }
                }
            }
        }
    });

    (actions, selected_item_rect)
}

// ============================================================================
// Footer panel rendering
// ============================================================================

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

    let layout_params = PickerLayoutParams {
        gap: sizes.xs,
        bottom_rounding: sizes.toolbar_icon / 2.0,
        top_rounding: sizes.s,
        outline_margin: (sizes.xxs, 0.),
        entire_available_rect: original_available_space,
    };

    // Use Sides widget to layout left (pickers) and right (buttons)
    let sides_result = Sides::new()
        .height(original_available_space.height())
        .shrink_left()
        .spacing(layout_params.gap)
        .show(
            ui,
            |ui| {
                // Left side: pickers
                let (default_actions, default_rect, default_idx) = render_default_notes_picker(
                    selected,
                    &opened_files,
                    command_list,
                    &layout_params,
                    ui,
                    ctx,
                    theme,
                );

                // External files picker in horizontal scroll
                let response = ScrollArea::horizontal()
                    .id_salt("footer_external_files")
                    .scroll_bar_visibility(
                        eframe::egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                    )
                    .show(ui, |ui| {
                        let (actions, rect) = render_external_files_picker(
                            selected,
                            &opened_files,
                            external_files,
                            &layout_params,
                            ui,
                            theme,
                        );

                        if let Some(item_rect) = rect {
                            ui.scroll_to_rect(item_rect, Some(eframe::emath::Align::Center));
                        }

                        (actions, rect)
                    });

                let (external_actions, external_rect) = response.inner;
                // Combine actions and determine selected item
                let mut left_side_actions = default_actions;
                left_side_actions.extend(external_actions);

                let selected_from_default = default_rect.is_some();
                let (selected_rect, is_first_item_overall) = if selected_from_default {
                    (default_rect, default_idx == Some(0))
                } else {
                    (
                        external_rect.map(|r| response.inner_rect.intersect(r)),
                        false,
                    )
                };

                (left_side_actions, selected_rect, is_first_item_overall)
            },
            |ui| {
                // Right side: buttons
                let mut right_actions: SmallVec<[AppAction; 1]> = SmallVec::new();

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
                    right_actions.push(AppAction::OpenFileDialog);
                }

                // Separator
                ui.label(
                    AppIcon::VerticalSeparator.render(sizes.toolbar_icon, theme.colors.outline_fg),
                );

                right_actions
            },
        );

    // Extract results from Sides
    let ((left_actions, selected_rect, is_first_item_overall), right_actions) = sides_result;

    actions.extend(left_actions);
    actions.extend(right_actions);

    if let Some(item_rect) = selected_rect {
        let animation_duration = 0.2;
        let picker_id = ui.id().with("simple_picker");

        let animated_item_width = ctx.animate_value_with_time(
            picker_id.with("width"),
            item_rect.width(),
            animation_duration,
        );

        // Calculate fixed height without vertical bump animation
        let item_height = item_rect.center().y + item_rect.height() / 2.0
            - layout_params.entire_available_rect.top();

        // If first item overall is selected, animate top left radius to 0, otherwise to normal radius
        let target_top_left_radius = if is_first_item_overall {
            0.0
        } else {
            layout_params.top_rounding
        };
        let animated_top_left_radius = ctx.animate_value_with_time(
            picker_id.with("top_left_radius"),
            target_top_left_radius,
            animation_duration,
        );

        let mut drop_shape = Shape::Path(PathShape {
            points: selection_outline(SelectionOutlineDesc {
                bottom_radius: layout_params.bottom_rounding,
                top_radious: (animated_top_left_radius, layout_params.top_rounding),
                item_width: animated_item_width,
                item_height: item_height,
                margin: layout_params.outline_margin,
            }),
            closed: false,
            fill: Color32::TRANSPARENT,
            stroke: PathStroke::new(1.0, theme.colors.outline_fg),
        });

        let drop_x = ctx.animate_value_with_time(
            picker_id.with("drop"),
            item_rect.center().x,
            animation_duration,
        );

        drop_shape.translate([drop_x, layout_params.entire_available_rect.top()].into());

        let painter = ui.painter();
        // Clip the outline to the available rect to prevent overflow
        painter
            .with_clip_rect(layout_params.entire_available_rect)
            .add(drop_shape);

        let (margin_x, _) = layout_params.outline_margin;
        let item_outline_width = animated_item_width
            + 2. * margin_x
            + animated_top_left_radius
            + layout_params.top_rounding;

        // Draw left side of the break line
        painter.line_segment(
            [
                layout_params.entire_available_rect.left_top(),
                pos2(
                    (drop_x - item_outline_width / 2.0)
                        .max(layout_params.entire_available_rect.left()),
                    layout_params.entire_available_rect.top(),
                ),
            ],
            Stroke::new(1.0, theme.colors.outline_fg),
        );

        // Draw right side of the break line
        painter.line_segment(
            [
                pos2(
                    (drop_x + item_outline_width / 2.0)
                        .min(layout_params.entire_available_rect.right()),
                    layout_params.entire_available_rect.top(),
                ),
                layout_params.entire_available_rect.right_top(),
            ],
            Stroke::new(1.0, theme.colors.outline_fg),
        );
    }

    actions
}
