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

use crate::app_ui::draw_debug_rect;

const PREALLOCATED_PICKER_ITEMS: usize = 5;

#[derive(Debug, Clone)]
struct PickerItemLayout<'a, Item: PartialEq> {
    item: &'a PickerItem<Item>,
    size: Vec2,
    offset_x: f32,
}

#[derive(Debug, Clone)]
struct PickerLayout<'a, Item: PartialEq> {
    items: SmallVec<[PickerItemLayout<'a, Item>; PREALLOCATED_PICKER_ITEMS]>,
    total_width: f32,
    layout_params: PickerLayoutParams,
    // available_rect: Rect,
}

#[derive(Debug, Clone)]
pub struct PickerVisualStyle {
    pub inactive_color: Color32,
    pub hover_color: Color32,
    pub pressed_color: Color32,
    pub selected_stroke_color: Color32,
    pub selected_fill_color: Color32,
    pub tooltip_text_color: Color32,
    pub outline: Stroke,
}

#[derive(Debug)]
pub enum PickerItemKind {
    FontIcon(SmolStr, FontId),
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

pub struct Picker<'a, Item: PartialEq> {
    pub current: Item,
    pub items: &'a [PickerItem<Item>],
    pub style: PickerVisualStyle,
    pub layout_params: PickerLayoutParams,
}

impl<'a, Item: PartialEq> Picker<'a, Item> {
    pub fn show(self, ui: &mut Ui) -> InnerResponse<Option<&'a Item>> {
        let mut result = None;
        let response = ui.add(PickerResultWrapper(&mut result, self));
        InnerResponse::new(result, response)
    }
}

struct PickerResultWrapper<'a, 'b, Item: PartialEq>(&'b mut Option<&'a Item>, Picker<'a, Item>);

fn calculate_picker_layout<'a, Item: PartialEq>(
    items: &'a [PickerItem<Item>],
    layout_params: PickerLayoutParams,
    painter: &egui::Painter,
    // available_rect: Rect,
) -> PickerLayout<'a, Item> {
    let mut layout_items = SmallVec::new();
    // just have some safe space
    // the idea is that the top arc should just end at relative x:0
    let mut offset = layout_params.bottom_rounding + layout_params.gap / 2.0;

    for item in items {
        let item_size = match &item.kind {
            PickerItemKind::FontIcon(text, font_id) | PickerItemKind::ItemName(text, font_id) => {
                measure_text_size(painter, text, font_id.clone())
            }
        };

        let offset_x = offset + item_size.x / 2.0;

        layout_items.push(PickerItemLayout {
            item,
            offset_x,
            size: item_size,
        });

        offset += item_size.x + layout_params.gap;
    }

    let total_width = offset - layout_params.gap; // Remove the last gap

    PickerLayout {
        items: layout_items,
        layout_params,
        total_width,
        // available_rect,
    }
}

impl<'a, 'b, Item: PartialEq> Widget for PickerResultWrapper<'a, 'b, Item> {
    fn ui(self, ui: &mut Ui) -> Response {
        let PickerResultWrapper(
            result,
            Picker {
                items,
                layout_params,
                current: original_current,
                style,
            },
        ) = self;

        let mut current = original_current;
        let radius = layout_params.bottom_rounding;
        // let available_rect = ui.available_rect_before_wrap();
        // ui.painter()
        //     .debug_rect(available_rect, Color32::RED, "picker space");

        // ui.painter().debug_rect(
        //     ui.available_rect_before_wrap(),
        //     Color32::LIGHT_GREEN,
        //     format!("available_rect={:?}", ui.available_rect_before_wrap()),
        // );

        let layout = calculate_picker_layout(items, layout_params, &ui.painter());

        let desired_size = vec2(layout.total_width, radius * 2.);
        ui.add_space(radius * 2.);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::hover());

        response.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, true, ""));

        let newly_selected = ui
            .is_rect_visible(rect)
            .then(|| render_picker_items(&layout, &mut current, ui, &style))
            .flatten();

        *result = newly_selected;

        response
    }
}

fn render_picker_items<'items, Item: PartialEq>(
    layout: &PickerLayout<'items, Item>,
    current: &Item,
    ui: &mut Ui,
    style: &PickerVisualStyle,
) -> Option<&'items Item> {
    let ctx = ui.ctx();
    let painter = ui.painter();
    let animation_duration = 0.2;
    let mut newly_selected = None;

    let picker_id = ui.id().with("picker");
    for (i, item_layout) in layout.items.iter().enumerate() {
        let item = item_layout.item;

        let center = pos2(
            layout.layout_params.entire_available_rect.left() + item_layout.offset_x,
            layout.layout_params.entire_available_rect.center().y,
        );

        let point_id = picker_id.with(i);
        let mut point_response = ui.interact(
            Rect::from_center_size(center, item_layout.size),
            point_id,
            Sense::click(),
        );

        let is_selected = &item_layout.item.data == current;
        if point_response.clicked() && !is_selected {
            // that means that the selection changed
            newly_selected = Some(&item_layout.item.data);
        }

        if !is_selected {
            let tooltip_ui = |ui: &mut egui::Ui| {
                ui.label(RichText::new(&item.tooltip).color(style.tooltip_text_color));
            };

            point_response = point_response.on_hover_ui(tooltip_ui);
        }

        let selection_progress =
            ctx.animate_bool_with_time(point_id, is_selected, animation_duration);
        // let width_ = ctx.animate_bool_with_time(point_id, is_selected, 0.2);

        let _fill = interpolate_color(
            Color32::TRANSPARENT,
            style.selected_fill_color,
            selection_progress,
        );

        let stroke = match (
            is_selected,
            point_response.hovered(),
            point_response.is_pointer_button_down_on(),
        ) {
            (true, _, _) => Stroke::new(1.5, style.selected_stroke_color),
            (_, true, false) => Stroke::new(1.5, style.hover_color),
            (_, _, true) => Stroke::new(2.0, style.pressed_color),
            _ => Stroke::new(1.0, style.inactive_color),
        };

        let selection_y_jump = layout.layout_params.gap / 1.0;

        let animated_text_center = pos2(center.x, center.y - selection_progress * selection_y_jump);

        match &item.kind {
            PickerItemKind::FontIcon(icon_symbol, font_id) => {
                painter.text(
                    animated_text_center,
                    Align2::CENTER_CENTER,
                    icon_symbol,
                    font_id.clone(),
                    stroke.color,
                );
            }
            PickerItemKind::ItemName(text, font_id) => {
                painter.text(
                    animated_text_center,
                    Align2::CENTER_CENTER,
                    text,
                    font_id.clone(),
                    stroke.color,
                );
            }
        }

        if is_selected {
            let animated_item_width = ctx.animate_value_with_time(
                picker_id.with("width"),
                item_layout.size.x,
                animation_duration,
            );

            let animated_height = ctx.animate_value_with_time(
                picker_id.with("height"),
                center.y + item_layout.size.y / 2.0
                    - layout.layout_params.entire_available_rect.top()
                    - selection_y_jump,
                animation_duration,
            );

            let mut drop_shape = Shape::Path(PathShape {
                points: selection_outline(SelectionOutlineDesc {
                    bottom_radius: layout.layout_params.bottom_rounding,
                    top_radious: layout.layout_params.top_rounding,
                    item_width: animated_item_width,
                    item_height: animated_height,
                    margin: layout.layout_params.outline_margin,
                }),
                closed: false,
                fill: Color32::TRANSPARENT,
                stroke: PathStroke::new(style.outline.width, style.outline.color),
            });

            let drop_x = ctx.animate_value_with_time(
                picker_id.with("drop"),
                animated_text_center.x,
                animation_duration,
            );

            drop_shape.translate([drop_x, layout.layout_params.entire_available_rect.top()].into());
            painter.add(drop_shape);

            let (margin_x, _) = layout.layout_params.outline_margin;

            let item_outline_width =
                animated_item_width + 2. * (margin_x + layout.layout_params.top_rounding);

            // left side of the break line
            painter.line_segment(
                [
                    layout.layout_params.entire_available_rect.left_top(),
                    pos2(
                        (drop_x - item_outline_width / 2.0)
                            .max(layout.layout_params.entire_available_rect.left()),
                        layout.layout_params.entire_available_rect.top(),
                    ),
                ],
                Stroke::new(style.outline.width, style.outline.color),
            );

            // right side of the break line
            painter.line_segment(
                [
                    pos2(
                        (drop_x + item_outline_width / 2.0)
                            .min(layout.layout_params.entire_available_rect.right()),
                        layout.layout_params.entire_available_rect.top(),
                    ),
                    layout.layout_params.entire_available_rect.right_top(),
                ],
                style.outline.clone(),
            );
        }
    }
    newly_selected
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
    top_radious: f32,
    item_width: f32,
    item_height: f32,
    /// That (x,y) margins to be applied to the outline
    ///
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

    let top_circle_center_x = item_width / 2.0 + margin_x + top_radious;

    let bottom_segment_width = (item_width - 2. * bottom_radius).max(0.0);
    let bottom_circle_center_y = item_height - bottom_radius + margin_y;
    let bottom_circle_center_x = bottom_segment_width / 2.0 + margin_x;

    // top left rounding
    add_circle_quadrant(
        &mut path,
        pos2(-top_circle_center_x, top_radious),
        top_radious,
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
        pos2(top_circle_center_x, top_radious),
        top_radious,
        2.0,
    );

    path
}

// ========== EXPERIMENTAL SIMPLE PICKER ==========
// Simplified picker using egui's horizontal layout instead of custom rendering
// No selection outline or animations yet

/// Simplified picker widget using egui's horizontal layout and IconButtons
pub struct SimplePicker<'a, Item: PartialEq> {
    pub current: Item,
    pub items: &'a [PickerItem<Item>],
    pub style: PickerVisualStyle,
    pub layout_params: PickerLayoutParams,
}

impl<'a, Item: PartialEq> SimplePicker<'a, Item> {
    pub fn show(
        self,
        ui: &mut Ui,
        theme: &crate::theme::AppTheme,
    ) -> InnerResponse<Option<&'a Item>> {
        let mut result = None;
        let available_rect = ui.available_rect_before_wrap();
        let response = ui.add(SimplePickerResultWrapper(
            &mut result,
            self,
            theme,
            available_rect,
        ));
        InnerResponse::new(result, response)
    }
}

struct SimplePickerResultWrapper<'a, 'b, 'theme, Item: PartialEq>(
    &'b mut Option<&'a Item>,
    SimplePicker<'a, Item>,
    &'theme crate::theme::AppTheme,
    Rect, // available_rect
);

impl<'a, 'b, 'theme, Item: PartialEq> Widget for SimplePickerResultWrapper<'a, 'b, 'theme, Item> {
    fn ui(self, ui: &mut Ui) -> Response {
        let SimplePickerResultWrapper(
            result,
            SimplePicker {
                items,
                current,
                style,
                layout_params,
            },
            theme,
            available_rect,
        ) = self;

        let mut newly_selected = None;
        let animation_duration = 0.2;
        let picker_id = ui.id().with("simple_picker");

        // Track selected item rect for outline drawing
        let mut selected_item_rect: Option<Rect> = None;
        let mut selected_item_index: Option<usize> = None;

        let response = ui.horizontal_centered(|ui| {
            // draw_debug_rect(ui);
            ui.spacing_mut().item_spacing.x = layout_params.gap;
            // // to make sure that the very left rounding does not exceed the widget bounds
            ui.add_space(layout_params.top_rounding + layout_params.outline_margin.0);

            for (i, item) in items.iter().enumerate() {
                let is_selected = &item.data == &current;

                // Get icon based on item kind
                let icon_str = match &item.kind {
                    PickerItemKind::FontIcon(icon_str, _font_id) => icon_str.as_str(),
                    PickerItemKind::ItemName(_name, _font_id) => {
                        // Ignore external files (ItemName) for now
                        continue;
                    }
                };

                // Map phosphor icon strings back to AppIcon enum
                // We need to match the exact strings from to_icon_str()
                use egui_phosphor::light as P;
                let app_icon = match icon_str {
                    P::NUMBER_ONE => crate::theme::AppIcon::One,
                    P::NUMBER_TWO => crate::theme::AppIcon::Two,
                    P::NUMBER_THREE => crate::theme::AppIcon::Three,
                    P::NUMBER_FOUR => crate::theme::AppIcon::Four,
                    P::GEAR_FINE => crate::theme::AppIcon::Settings,
                    P::DOTS_THREE_OUTLINE => crate::theme::AppIcon::More,
                    P::FOLDER_SIMPLE => crate::theme::AppIcon::Folder,
                    _ => crate::theme::AppIcon::More, // fallback
                };

                let selection_y_jump = layout_params.gap;
                let item_id = picker_id.with(i);
                let selection_progress =
                    ui.ctx()
                        .animate_bool_with_time(item_id, is_selected, animation_duration);

                // Calculate vertical offset (negative to move up, matching line 230)
                let y_offset = -selection_progress * selection_y_jump;
                let transform = TSTransform::from_translation((0.0, y_offset).into());

                let button = crate::ui_components::IconButton::new(app_icon, theme)
                    .size(crate::ui_components::IconButtonSize::Large)
                    .toggled(is_selected)
                    .tooltip(&item.tooltip, None);

                let button_response = ui.with_visual_transform(transform, |ui| ui.add(button));

                if button_response.inner.clicked() && !is_selected {
                    newly_selected = Some(&item.data);
                }

                // Store selected item's rect for outline drawing
                if is_selected {
                    selected_item_rect = Some(button_response.response.rect);
                    selected_item_index = Some(i);
                }
            }
        });

        if let (Some(item_rect), Some(_idx)) = (selected_item_rect, selected_item_index) {
            // Animate item width (line 256-260)
            let animated_item_width = ui.ctx().animate_value_with_time(
                picker_id.with("width"),
                item_rect.width(),
                animation_duration,
            );

            let selection_y_jump = layout_params.gap;
            let animated_height = ui.ctx().animate_value_with_time(
                picker_id.with("height"),
                item_rect.center().y + item_rect.height() / 2.0
                    - available_rect.top()
                    - selection_y_jump,
                animation_duration,
            );

            let mut drop_shape = Shape::Path(PathShape {
                points: selection_outline(SelectionOutlineDesc {
                    bottom_radius: layout_params.bottom_rounding,
                    top_radious: layout_params.top_rounding,
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

            drop_shape.translate([drop_x, available_rect.top()].into());

            let painter = ui.painter();
            painter.add(drop_shape);

            let (margin_x, _) = layout_params.outline_margin;
            let item_outline_width =
                animated_item_width + 2. * (margin_x + layout_params.top_rounding);

            // Draw left side of the break line
            painter.line_segment(
                [
                    available_rect.left_top(),
                    pos2(
                        (drop_x - item_outline_width / 2.0).max(available_rect.left()),
                        available_rect.top(),
                    ),
                ],
                Stroke::new(style.outline.width, style.outline.color),
            );

            // Draw right side of the break line
            painter.line_segment(
                [
                    pos2(
                        (drop_x + item_outline_width / 2.0).min(available_rect.right()),
                        available_rect.top(),
                    ),
                    available_rect.right_top(),
                ],
                style.outline.clone(),
            );
        }

        *result = newly_selected;

        response.response
    }
}
