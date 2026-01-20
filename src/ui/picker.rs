use std::ops::Range;

use eframe::{
    egui::{
        self, Align2, FontFamily, FontId, InnerResponse, Response, RichText, Sense, Ui, Vec2,
        Widget, WidgetInfo, WidgetType,
    },
    emath::TSTransform,
    epaint::{
        Color32, PathShape, PathStroke, Pos2, Rect, Shape, Stroke, pos2,
        tessellator::path::add_circle_quadrant, vec2,
    },
};
use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::{
    theme::{AppIcon, AppTheme},
    ui_components::{IconButtonSize, apply_icon_btn_styling},
};

const PREALLOCATED_PICKER_ITEMS: usize = 5;

#[derive(Debug, Clone)]
struct PickerItemLayout<'a, Item: PartialEq> {
    item: &'a PickerItem<Item>,
    size: Vec2,
    offset_x: f32,
}

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

fn measure_text_size(painter: &egui::Painter, text: &str, font_id: FontId) -> Vec2 {
    let galley = painter.layout_no_wrap(text.to_string(), font_id, Color32::WHITE);
    galley.size()
}

fn interpolate_color(from: Color32, to: Color32, progress: f32) -> Color32 {
    let f = from.linear_multiply(1.0 - progress);
    let t = to.linear_multiply(progress);

    let [fr, fg, fb, fa] = f.to_normalized_gamma_f32();
    let [tr, tg, tb, ta] = t.to_normalized_gamma_f32();

    Color32::from_rgba_premultiplied(
        ((fr + tr) * 255.) as u8,
        ((fg + tg) * 255.) as u8,
        ((fb + tb) * 255.) as u8,
        ((fa + ta) * 255.) as u8,
    )
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

            let selection_y_jump = layout_params.gap;
            let item_id = picker_id.with(i);
            let selection_progress =
                ui.ctx()
                    .animate_bool_with_time(item_id, is_selected, animation_duration);

            // Calculate vertical offset (negative to move up)
            let y_offset = -selection_progress * selection_y_jump;
            let transform = TSTransform::from_translation((0.0, y_offset).into());

            let button_response = match &item.kind {
                PickerItemKind::FontIcon(app_icon, icon_size) => {
                    let button = crate::ui_components::IconButton::new(*app_icon, theme)
                        .size(*icon_size)
                        .toggled(is_selected)
                        .tooltip(&item.tooltip, None);

                    ui.with_visual_transform(transform, |ui| ui.add(button))
                }

                PickerItemKind::ItemName(smol_str, font_id) => {
                    ui.with_visual_transform(transform, |ui| {
                        apply_icon_btn_styling(ui.style_mut());
                        ui.button(RichText::new(smol_str.as_str()).font(font_id.clone()))
                    })
                }
            };

            if button_response.inner.clicked() && !is_selected {
                newly_selected = Some(item.data);
            }

            // Store selected item's rect for outline drawing
            if is_selected {
                selected_item_rect_index = Some((button_response.response.rect, i));
            }
        }
    });

    if let Some((item_rect, idx)) = selected_item_rect_index {
        let animated_item_width = ui.ctx().animate_value_with_time(
            picker_id.with("width"),
            item_rect.width(),
            animation_duration,
        );

        let selection_y_jump = layout_params.gap;
        let animated_height = ui.ctx().animate_value_with_time(
            picker_id.with("height"),
            item_rect.center().y + item_rect.height() / 2.0
                - layout_params.entire_available_rect.top()
                - selection_y_jump,
            animation_duration,
        );

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
                item_height: animated_height,
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
