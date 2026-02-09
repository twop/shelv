use eframe::{
    egui::{
        Color32, Context, FontFamily, FontId, InnerResponse, Layout, RichText, ScrollArea, Ui,
        UiBuilder, vec2,
    },
    emath::Align,
    epaint::{
        PathShape, PathStroke, Pos2, Rect, Shape, Stroke, pos2,
        tessellator::path::add_circle_quadrant,
    },
};
use smallvec::SmallVec;
use smol_str::{SmolStr, ToSmolStr};

use crate::{
    app_actions::AppAction,
    app_ui::draw_debug_rect,
    command::{CommandInstruction, CommandList},
    persistent_state::{ExternalFile, NoteId},
    theme::{AppIcon, AppTheme},
    ui_components::{IconButton, IconButtonSize, apply_icon_btn_styling},
};

// ============================================================================
// Picker structures and functions (copied from picker.rs)
// ============================================================================

#[derive(Debug, Clone)]
pub struct PickerVisualStyle {
    pub outline: Stroke,
}

#[derive(Debug)]
pub enum PickerItemKind {
    FontIcon(AppIcon, IconButtonSize),
    ItemName(SmolStr, FontId),
}

#[derive(Debug)]
pub struct PickerItem<Item: PartialEq> {
    pub tooltip: String,
    pub kind: PickerItemKind,
    pub data: Item,
}

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

pub fn render_picker_items_refactored<'a, Item: PartialEq>(
    current: Item,
    items: impl IntoIterator<Item = PickerItem<Item>>,
    style: PickerVisualStyle,
    layout_params: PickerLayoutParams,
    ui: &mut Ui,
    theme: &AppTheme,
) -> InnerResponse<Option<Item>> {
    let mut newly_selected = None;
    let animation_duration = 0.2;
    let picker_id = ui.id().with("simple_picker");

    // This is to track selected item rect for outline drawing
    let mut selected_item_rect_index: Option<(Rect, usize)> = None;

    let response = ui.horizontal_centered(|ui| {
        // draw_debug_rect(ui);
        ui.spacing_mut().item_spacing.x = layout_params.gap;
        // // to make sure that the very left rounding does not exceed the widget bounds
        // we animate the rad to 0.0, hence only margin needs to be adjusted
        ui.add_space(layout_params.outline_margin.0);

        for (i, item) in items.into_iter().enumerate() {
            let is_selected = &item.data == &current;

            let button_response = match &item.kind {
                PickerItemKind::FontIcon(app_icon, icon_size) => {
                    let button = crate::ui_components::IconButton::new(*app_icon, theme)
                        .size(*icon_size)
                        .toggled(is_selected)
                        .tooltip(&item.tooltip, None);

                    ui.add(button)
                }

                PickerItemKind::ItemName(smol_str, font_id) => {
                    apply_icon_btn_styling(ui.style_mut());
                    ui.button(RichText::new(smol_str.as_str()).font(font_id.clone()))
                }
            };

            if button_response.clicked() && !is_selected {
                newly_selected = Some(item.data);
            }

            // Store selected item's rect for outline drawing
            if is_selected {
                selected_item_rect_index = Some((button_response.rect, i));
            }
        }
    });

    if let Some((item_rect, idx)) = selected_item_rect_index {
        let animated_item_width = ui.ctx().animate_value_with_time(
            picker_id.with("width"),
            item_rect.width(),
            animation_duration,
        );

        // Calculate fixed height without vertical bump animation
        let item_height = item_rect.center().y + item_rect.height() / 2.0
            - layout_params.entire_available_rect.top();

        // If first item is selected, animate top left radius to 0, otherwise to normal radius
        let is_first_item = idx == 0;
        let target_top_left_radius = if is_first_item {
            0.0
        } else {
            layout_params.top_rounding
        };
        let animated_top_left_radius = ui.ctx().animate_value_with_time(
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
            stroke: PathStroke::new(style.outline.width, style.outline.color),
        });

        let drop_x = ui.ctx().animate_value_with_time(
            picker_id.with("drop"),
            item_rect.center().x,
            animation_duration,
        );

        drop_shape.translate([drop_x, layout_params.entire_available_rect.top()].into());

        let painter = ui.painter();
        painter.add(drop_shape);

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
            Stroke::new(style.outline.width, style.outline.color),
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
            style.outline.clone(),
        );
    }

    InnerResponse::new(newly_selected, response.response)
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

        // STEP 2: Render default items (settings and notes) + external files in one picker
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
            NoteId::ExternalFileId(external_id) => {
                // Include external files in the main picker
                external_files
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
                    })
            }
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
    };

    actions
}
