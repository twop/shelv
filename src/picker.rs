use eframe::{
    egui::{
        self, Align2, FontFamily, FontId, InnerResponse, Response, RichText, Sense, Ui, Vec2,
        Widget, WidgetInfo, WidgetType,
    },
    epaint::{
        Color32, PathShape, PathStroke, Pos2, Rect, Shape, Stroke, pos2,
        tessellator::path::add_circle_quadrant, vec2,
    },
};
use smallvec::SmallVec;
use smol_str::SmolStr;

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
    gap: f32,
    bottom_rounding: f32,
    top_rounding: f32,
    available_rect: Rect,
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

pub struct Picker<'a, Item: PartialEq> {
    pub current: Item,
    pub items: &'a [PickerItem<Item>],
    pub gap: f32,
    pub bottom_rounding: f32,
    pub style: PickerVisualStyle,
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
    gap: f32,
    bottom_rounding: f32,
    painter: &egui::Painter,
    available_rect: Rect,
) -> PickerLayout<'a, Item> {
    let mut layout_items = SmallVec::new();
    // just have some safe space
    // the idea is that the top arc should just end at relative x:0
    let mut offset = bottom_rounding + gap / 2.0;

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

        offset += item_size.x + gap;
    }

    let total_width = offset - gap; // Remove the last gap

    PickerLayout {
        items: layout_items,
        total_width,
        gap,
        bottom_rounding,
        top_rounding: bottom_rounding,
        available_rect,
    }
}

impl<'a, 'b, Item: PartialEq> Widget for PickerResultWrapper<'a, 'b, Item> {
    fn ui(self, ui: &mut Ui) -> Response {
        let PickerResultWrapper(
            result,
            Picker {
                items,
                gap,
                bottom_rounding,
                current: original_current,
                style,
            },
        ) = self;

        let mut current = original_current;
        let radius = bottom_rounding;
        let available_rect = ui.available_rect_before_wrap();

        let layout =
            calculate_picker_layout(items, gap, bottom_rounding, &ui.painter(), available_rect);

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
            layout.available_rect.left() + item_layout.offset_x,
            layout.available_rect.center().y,
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

        let selection_y_jump = layout.gap / 2.0;

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

            let margin = layout.gap / 2.0;

            let animated_height = ctx.animate_value_with_time(
                picker_id.with("height"),
                center.y + item_layout.size.y / 2.0
                    - layout.available_rect.top()
                    - selection_y_jump,
                animation_duration,
            );

            let mut drop_shape = Shape::Path(PathShape {
                points: selection_outline(SelectionOutlineDesc {
                    bottom_radius: layout.bottom_rounding,
                    top_radious: layout.top_rounding,
                    item_width: animated_item_width,
                    item_height: animated_height,
                    margin,
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

            drop_shape.translate([drop_x, layout.available_rect.top()].into());
            painter.add(drop_shape);

            let item_outline_width = animated_item_width + 2. * (margin + layout.top_rounding);
            painter.line_segment(
                [
                    layout.available_rect.left_top(),
                    pos2(
                        (drop_x - item_outline_width / 2.0).max(layout.available_rect.left()),
                        layout.available_rect.top(),
                    ),
                ],
                Stroke::new(style.outline.width, style.outline.color),
            );

            painter.line_segment(
                [
                    pos2(
                        (drop_x + item_outline_width / 2.0).min(layout.available_rect.right()),
                        layout.available_rect.top(),
                    ),
                    layout.available_rect.right_top(),
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
    /// That applies to the sides and bottom
    margin: f32,
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

    let top_circle_center_x = item_width / 2.0 + margin + top_radious;

    let bottom_segment_width = (item_width - 2. * bottom_radius).max(0.0);
    let bottom_circle_center_y = item_height - bottom_radius + margin;
    let bottom_circle_center_x = bottom_segment_width / 2.0 + margin;

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
