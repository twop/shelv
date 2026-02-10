use eframe::{
    egui::{
        Button, Color32, Context, FontFamily, FontId, RichText, ScrollArea, Sides, Ui, Vec2,
        scroll_area::ScrollBarVisibility, vec2,
    },
    emath::Align,
    epaint::{
        PathShape, PathStroke, Pos2, Rect, Shape, Stroke, pos2,
        tessellator::path::add_circle_quadrant,
    },
};
use smallvec::SmallVec;

use crate::{
    app_actions::{AppAction, StepDirection, SwitchToNoteTarget},
    app_state::RenderAction,
    command::{CommandInstruction, CommandList},
    persistent_state::{ExternalFile, NoteId},
    theme::{AppIcon, AppTheme},
    ui_components::{apply_icon_btn_styling, get_button_fg_color, rich_text_tooltip, IconButton, IconButtonSize},
};

// ============================================================================
// Filename truncation
// ============================================================================

/// Configuration for truncating long filenames
#[derive(Debug, Clone, Copy)]
pub struct FilenameCapConfig {
    /// Maximum total characters allowed
    pub max_chars: usize,
    /// Number of characters to preserve from the beginning (before ..)
    pub offset_from_beginning: usize,
}

impl Default for FilenameCapConfig {
    fn default() -> Self {
        Self {
            max_chars: 20,
            offset_from_beginning: 4,
        }
    }
}

/// Truncates a filename to a maximum length while preserving the extension.
///
/// Examples:
/// - `too_very_long.md` -> `too_..long.md` (with offset=4, max=15)
/// - `short.md` -> `short.md` (no truncation needed)
/// - `no_extension` -> `no_e..ion_here` (no extension to preserve)
fn truncate_filename(filename: &str, config: FilenameCapConfig) -> String {
    let FilenameCapConfig {
        max_chars,
        offset_from_beginning,
    } = config;

    if filename.len() <= max_chars {
        return filename.to_string();
    }

    let separator = "..";

    // Find the last dot to identify extension
    if let Some(dot_pos) = filename.rfind('.') {
        let name = &filename[..dot_pos];
        let ext = &filename[dot_pos..]; // includes the dot

        // Check if extension is too long to preserve
        let min_required = offset_from_beginning + separator.len() + ext.len();

        if min_required > max_chars {
            // Extension is too long, truncate without preserving it
            let end_chars = max_chars.saturating_sub(offset_from_beginning + separator.len());
            let start = &filename[..offset_from_beginning.min(filename.len())];
            let end = if end_chars > 0 && filename.len() > offset_from_beginning + separator.len() {
                &filename[filename.len().saturating_sub(end_chars)..]
            } else {
                ""
            };
            format!("{}{}{}", start, separator, end)
        } else {
            // We can preserve the extension
            // Calculate how many chars we have left for the name (excluding separator and extension)
            let available_for_name = max_chars.saturating_sub(separator.len() + ext.len());

            // Start takes offset_from_beginning chars, but not more than available
            let start_len = offset_from_beginning
                .min(available_for_name)
                .min(name.len());

            // End takes the remaining available chars from the end of the name
            let end_len = available_for_name.saturating_sub(start_len);

            let start = &name[..start_len];
            let end = if end_len > 0 && name.len() > start_len {
                let end_start_pos = name.len().saturating_sub(end_len);
                &name[end_start_pos..]
            } else {
                ""
            };

            format!("{}{}{}{}", start, separator, end, ext)
        }
    } else {
        // No extension found
        let end_chars = max_chars.saturating_sub(offset_from_beginning + separator.len());
        let start = &filename[..offset_from_beginning.min(filename.len())];
        let end = if end_chars > 0 && filename.len() > offset_from_beginning + separator.len() {
            &filename[filename.len().saturating_sub(end_chars)..]
        } else {
            ""
        };
        format!("{}{}{}", start, separator, end)
    }
}

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
            actions.push(AppAction::SwitchToNote(SwitchToNoteTarget::TargetNote {
                note_file: NoteId::Settings,
                via_shortcut: false,
            }));
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
                    .tooltip(format!("Shelf {}", index + 1), cmd.and_then(|cmd| cmd.shortcut) );

                let button_response = ui.add(button);
                if button_response.clicked() && !is_selected {
                    actions.push(AppAction::SwitchToNote(SwitchToNoteTarget::TargetNote {
                        note_file: *note_id,
                        via_shortcut: false,
                    }));
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
    command_list: &CommandList,
    ui: &mut Ui,
    ctx: &Context,
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
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| external_id.to_6_digit_smol_str().to_string());

                    let display_name = truncate_filename(&filename, FilenameCapConfig::default());

                    // Create unique ID for this external file's hover animation
                    let hover_id = ui.id().with("external_file_hover").with(*external_id);

                    // Create a horizontal UI that always contains both the filename and close button space
                    let response = ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = theme.sizes.xxs;
                        ui.add_space(theme.sizes.xxs);

                        apply_icon_btn_styling(ui.style_mut());
                        let button_response = {
                            ui.add(
                                Button::new(
                                    RichText::new(display_name)
                                        .font(FontId::new(
                                            theme.fonts.size.normal,
                                            FontFamily::Proportional,
                                        ))
                                        .color(get_button_fg_color(is_selected, theme, None)),
                                )
                                .min_size(Vec2::splat(
                                    IconButtonSize::Large.get_icon_font_size(theme)
                                        + theme.sizes.xxs,
                                )),
                            )
                        };

                        // Check if the button is hovered
                        let button_hovered = button_response.hovered();


                        let button_response =
                        button_response.on_hover_ui(|ui| {
                            ui.set_max_width(theme.sizes.menu_width);
                            ui.label(rich_text_tooltip(&ext_file.path.display().to_string(), None, theme));
                        });

                        // let button_response =
                        //     button_response.on_hover_text(ext_file.path.display().to_string());
                        let mut close_button_clicked = false;

                        let size = IconButtonSize::Small;

                        // This is needed to show "x" button
                        // The idea here is once we hovered over the file button it gives 200ms to hover over "x" button
                        let current_hover_progress = ctx.animate_value_with_time(
                            hover_id,
                            if button_hovered { 1.0 } else { 0.0 },
                            0.2,
                        );

                        // Only render the close button if we're hovering or animating
                        if current_hover_progress > 0.0 {
                            let close_button =
                                IconButton::new(AppIcon::Close, theme).size(size).tooltip(
                                    "Close file",
                                    command_list
                                        .find(CommandInstruction::CloseCurrentNote)
                                        .and_then(|i| i.shortcut),
                                );

                            let close_response = ui.add(close_button);
                            // Note that I'm using animate_value_with_time vs bool, due to egui tracks toggle time
                            // hence the "x" button will work as expected: if hovered is stays hovered
                            if close_response.hovered() {
                                ctx.animate_value_with_time(hover_id, 1.0, 0.2);
                            }
                            close_button_clicked = close_response.clicked();
                        }
                        ui.add_space(theme.sizes.xxs);

                        (button_response, close_button_clicked)
                    });

                    let (button_response, close_button_clicked) = response.inner;
                    let group_rect = response.response.rect;

                    // Only process the original button click if close button wasn't clicked
                    if !close_button_clicked && button_response.clicked() && !is_selected {
                        actions.push(AppAction::SwitchToNote(SwitchToNoteTarget::TargetNote {
                            note_file: *note_id,
                            via_shortcut: false,
                        }));
                    }

                    // If close button was clicked, add close action
                    if close_button_clicked {
                        actions.push(AppAction::CloseExternalFile(*external_id));
                    }

                    if is_selected {
                        selected_item_rect = Some(group_rect);
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
    render_actions: &mut SmallVec<[RenderAction; 2]>,
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
        .spacing(theme.sizes.s)
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

                ui.add_space(theme.sizes.s);

                // External files picker in horizontal scroll
                let response = ScrollArea::horizontal()
                    .id_salt("footer_external_files")
                    .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        let (actions, rect) = render_external_files_picker(
                            selected,
                            &opened_files,
                            external_files,
                            &layout_params,
                            command_list,
                            ui,
                            ctx,
                            theme,
                        );

                        // Check if we have a scroll action for the selected external file
                        if let NoteId::ExternalFileId(file_id) = selected {
                            let should_scroll = render_actions.iter().any(|action| {
                                matches!(action, RenderAction::ScrollToExternalFile(id) if *id == file_id)
                            });
                            
                            if should_scroll {
                                if let Some(item_rect) = rect {
                                    ui.scroll_to_rect(item_rect, Some(eframe::emath::Align::Center));
                                }
                            }
                        }

                        (actions, rect)
                    });

                let (external_actions, external_rect) = response.inner;
                
                // Remove scroll actions for external files after processing
                if let NoteId::ExternalFileId(file_id) = selected {
                    render_actions.retain(|action| {
                        !matches!(action, RenderAction::ScrollToExternalFile(id) if *id == file_id)
                    });
                }
                
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

                // Count external files to determine if we should show stepper buttons
                let external_file_count = opened_files
                    .iter()
                    .filter(|id| matches!(id, NoteId::ExternalFileId(_)))
                    .count();

                // Show stepper buttons if there are multiple external files
                if external_file_count > 1 {
                    if ui
                        .add(
                            IconButton::new(AppIcon::ChevronRight, theme)
                                .size(IconButtonSize::Large)
                                .tooltip(
                                    "Next Note",
                                    command_list
                                        .find(CommandInstruction::SwitchToNextNote)
                                        .and_then(|cmd| cmd.shortcut),
                                ),
                        )
                        .clicked()
                    {
                        right_actions.push(AppAction::SwitchToNote(SwitchToNoteTarget::StepNote(
                            StepDirection::Right,
                        )));
                    }
                    if ui
                        .add(
                            IconButton::new(AppIcon::ChevronLeft, theme)
                                .size(IconButtonSize::Large)
                                .tooltip(
                                    "Previous Note",
                                    command_list
                                        .find(CommandInstruction::SwitchToPrevNote)
                                        .and_then(|cmd| cmd.shortcut),
                                ),
                        )
                        .clicked()
                    {
                        right_actions.push(AppAction::SwitchToNote(SwitchToNoteTarget::StepNote(
                            StepDirection::Left,
                        )));
                    }

                    // // Separator
                    // ui.label(
                    //     AppIcon::VerticalSeparator
                    //         .render(sizes.toolbar_icon, theme.colors.outline_fg),
                    // );
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_filename_no_truncation_needed() {
        let config = FilenameCapConfig {
            max_chars: 20,
            offset_from_beginning: 4,
        };
        assert_eq!(truncate_filename("short.md", config), "short.md");
        assert_eq!(
            truncate_filename("exact_length.md", config),
            "exact_length.md"
        );
    }

    #[test]
    fn test_truncate_filename_with_extension() {
        let config = FilenameCapConfig {
            max_chars: 15,
            offset_from_beginning: 4,
        };
        // "too_very_long" is the name (13 chars), ".md" is extension (3 chars)
        // max_chars=15, minus ".." (2) minus ".md" (3) = 10 chars for name parts
        // start gets 4 chars: "too_", end gets 6 chars from end of name: "y_long"
        assert_eq!(
            truncate_filename("too_very_long.md", config),
            "too_..y_long.md"
        );
    }

    #[test]
    fn test_truncate_filename_different_offset() {
        let config = FilenameCapConfig {
            max_chars: 20,
            offset_from_beginning: 6,
        };
        // "this_is_a_very_long_filename" is the name (28 chars), ".md" is extension (3 chars)
        // max_chars=20, minus ".." (2) minus ".md" (3) = 15 chars for name parts
        // start gets 6 chars: "this_i", end gets 9 chars from end: "_filename"
        assert_eq!(
            truncate_filename("this_is_a_very_long_filename.md", config),
            "this_i.._filename.md"
        );
    }

    #[test]
    fn test_truncate_filename_no_extension() {
        let config = FilenameCapConfig {
            max_chars: 15,
            offset_from_beginning: 4,
        };
        // "no_extension_here" is 17 chars total
        // max_chars=15, minus ".." (2) = 13 chars available
        // start gets 4 chars: "no_e", end gets 9 chars: "sion_here"
        assert_eq!(
            truncate_filename("no_extension_here", config),
            "no_e..sion_here"
        );
    }

    #[test]
    fn test_truncate_filename_very_long_extension() {
        let config = FilenameCapConfig {
            max_chars: 15,
            offset_from_beginning: 4,
        };
        // Extension is too long to preserve with offset
        assert_eq!(
            truncate_filename("file.verylongextension", config),
            "file..extension"
        );
    }

    #[test]
    fn test_truncate_filename_edge_cases() {
        let config = FilenameCapConfig {
            max_chars: 10,
            offset_from_beginning: 3,
        };
        assert_eq!(truncate_filename("a.md", config), "a.md");
        assert_eq!(truncate_filename("abc.md", config), "abc.md");
        // "abcdefgh" is name (8 chars), ".md" is extension (3 chars)
        // max_chars=10, minus ".." (2) minus ".md" (3) = 5 chars for name parts
        // start gets 3 chars: "abc", end gets 2 chars: "gh"
        assert_eq!(truncate_filename("abcdefgh.md", config), "abc..gh.md");
    }

    #[test]
    fn test_truncate_filename_default_config() {
        let config = FilenameCapConfig::default();
        // max_chars=20, offset=4
        // "this_is_a_very_long_filename" is name (28 chars), ".md" is extension (3 chars)
        // max_chars=20, minus ".." (2) minus ".md" (3) = 15 chars for name parts
        // start gets 4 chars: "this", end gets 11 chars: "ng_filename"
        assert_eq!(
            truncate_filename("this_is_a_very_long_filename.md", config),
            "this..ng_filename.md"
        );
    }

    #[test]
    fn test_truncate_filename_multiple_dots() {
        let config = FilenameCapConfig {
            max_chars: 20,
            offset_from_beginning: 5,
        };
        // Should use the last dot for extension
        // "my.file.name.is.long" is name (20 chars), ".md" is extension (3 chars)
        // max_chars=20, minus ".." (2) minus ".md" (3) = 15 chars for name parts
        // start gets 5 chars: "my.fi", end gets 10 chars: "ame.is.long" -> wait, that's 11
        // Actually: end gets 10 chars from end of name: "me.is.long"
        assert_eq!(
            truncate_filename("my.file.name.is.long.md", config),
            "my.fi..me.is.long.md"
        );
    }
}
