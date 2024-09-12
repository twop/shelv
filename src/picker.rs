use eframe::{
    egui::{
        self, Align2, Context, FontFamily, FontId, Id, InnerResponse, Response, RichText, Sense,
        Ui, Widget, WidgetInfo, WidgetType, WidgetWithState,
    },
    epaint::{
        self, pos2, tessellator::path::add_circle_quadrant, vec2, Color32, PathShape, PathStroke,
        Pos2, Rect, Shape, Stroke, Vec2,
    },
};

pub enum PickerItemKind {
    Dot,
    FontIcon(&'static str, FontFamily),
}

pub struct PickerItem<Item: PartialEq> {
    pub tooltip: String,
    pub kind: PickerItemKind,
    pub data: Item,
}

pub struct Picker<'a, Item: PartialEq> {
    pub current: usize,
    pub items: &'a [PickerItem<Item>],
    pub gap: f32,
    pub item_size: f32,
    // colors
    pub inactive_color: Color32,
    pub hover_color: Color32,
    pub pressed_color: Color32,
    pub selected_stroke_color: Color32,
    pub selected_fill_color: Color32,
    pub tooltip_text_color: Color32,

    // drop
    pub outline: PathStroke,
}

impl<'a, Item: PartialEq> Picker<'a, Item> {
    pub fn show(self, ui: &mut Ui) -> InnerResponse<Option<&'a Item>> {
        let mut result = None;
        let response = ui.add(PickerResultWrapper(&mut result, self));
        InnerResponse::new(result, response)
    }
}

struct PickerResultWrapper<'a, 'b, Item: PartialEq>(&'b mut Option<&'a Item>, Picker<'a, Item>);

impl<'a, 'b, Item: PartialEq> Widget for PickerResultWrapper<'a, 'b, Item> {
    fn ui(self, ui: &mut Ui) -> Response {
        let PickerResultWrapper(
            result,
            Picker {
                items,
                gap,
                item_size: box_size,
                current: original_current,
                inactive_color: inactive,
                hover_color: hover,
                pressed_color: pressed,
                selected_stroke_color: selected_stroke,
                outline,
                selected_fill_color: selected_fill,
                tooltip_text_color: tooltip_text,
            },
        ) = self;

        let mut current = original_current;
        let radius = box_size / 2.0;
        let count = items.len() as u32;
        let desired_size = vec2(
            box_size * (count as f32) + gap * ((count - 1) as f32),
            box_size,
        );

        let available_rect = ui.available_rect_before_wrap();
        ui.add_space(radius * 2.);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::hover());
        // println!("allocated={:?}, available = {:?}", rect, avail);

        response.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, true, ""));

        if ui.is_rect_visible(rect) {
            // let visuals = ui.style().interact_selectable(&response, checked); // too colorful
            // let visuals = ui.style().interact(&response);

            let painter = ui.painter();

            let mut offset = rect.min.x;

            let ctx = ui.ctx();
            for i in items.iter().enumerate().map(Some).intersperse_with(|| None) {
                match i {
                    Some((i, item)) => {
                        let center = pos2(offset + radius, rect.center().y);
                        let rect = Rect::from_center_size(center, vec2(box_size, box_size));

                        let point_id = response.id.with(i);
                        let mut point_response = ui.interact(rect, point_id, Sense::click());

                        if point_response.clicked() {
                            current = i;
                        }

                        let is_selected = i == current;

                        if !is_selected {
                            let tooltip_ui = |ui: &mut egui::Ui| {
                                ui.label(RichText::new(&item.tooltip).color(tooltip_text));
                                //.font(self.font_id.clone())
                            };

                            point_response = point_response.on_hover_ui(tooltip_ui);
                        }

                        let selection_progress =
                            ctx.animate_bool_with_time(point_id, is_selected, 0.2);

                        let fill = interpolate_color(
                            Color32::TRANSPARENT,
                            selected_fill,
                            selection_progress,
                        );

                        let stroke = match (
                            is_selected,
                            point_response.hovered(),
                            point_response.is_pointer_button_down_on(),
                        ) {
                            (true, _, _) => Stroke::new(1.5, selected_stroke),
                            (_, true, false) => Stroke::new(1.5, hover),
                            (_, _, true) => Stroke::new(2.0, pressed),
                            _ => Stroke::new(1.0, inactive),
                        };

                        let center = pos2(center.x, center.y - selection_progress * radius / 2.0);

                        match &item.kind {
                            PickerItemKind::Dot => {
                                painter.add(epaint::CircleShape {
                                    center,
                                    radius,
                                    fill,
                                    stroke,
                                });
                            }
                            PickerItemKind::FontIcon(icon_symbol, font_family) => {
                                painter.text(
                                    center,
                                    Align2::CENTER_CENTER,
                                    icon_symbol,
                                    FontId::new(box_size, font_family.clone()),
                                    stroke.color,
                                );
                            }
                        }

                        // painter.add(shape)

                        if is_selected {
                            let top_rounding = gap;

                            // the outline will be right in between dots
                            let drop_radius = radius + gap / 2.;

                            let drop_x = ctx.animate_value_with_time(
                                response.id.with("drop"),
                                center.x,
                                0.2,
                            );

                            let drop_pos = pos2(drop_x, center.y);

                            let space_above = drop_pos.y - available_rect.top();

                            // TODO, cache producing path in the widget state
                            let mut drop_shape = Shape::Path(PathShape {
                                points: drop_path(DropPathDesc {
                                    radius: drop_radius,
                                    total_space_above: space_above,
                                    top_rounding,
                                }),
                                closed: false,
                                fill: Color32::TRANSPARENT,
                                stroke: outline.clone(),
                            });

                            painter.line_segment(
                                [
                                    available_rect.left_top(),
                                    pos2(
                                        (drop_pos.x - top_rounding - drop_radius)
                                            .max(available_rect.left()),
                                        available_rect.top(),
                                    ),
                                ],
                                outline.clone(),
                            );

                            painter.line_segment(
                                [
                                    pos2(
                                        (drop_pos.x + top_rounding + drop_radius)
                                            .min(available_rect.right()),
                                        available_rect.top(),
                                    ),
                                    available_rect.right_top(),
                                ],
                                outline.clone(),
                            );

                            drop_shape.translate(drop_pos.to_vec2());
                            painter.add(drop_shape);
                        }

                        offset += box_size
                    }
                    None => offset += gap,
                }
            }
        }

        if current != original_current {
            *result = items.get(current).map(|item| &item.data);
        }

        response
    }
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

struct DropPathDesc {
    radius: f32,
    total_space_above: f32,
    top_rounding: f32,
}

fn drop_path(
    DropPathDesc {
        radius,
        total_space_above,
        top_rounding,
    }: DropPathDesc,
) -> Vec<Pos2> {
    let mut path: Vec<Pos2> = vec![];

    // top left rounding
    add_circle_quadrant(
        &mut path,
        pos2(-radius - top_rounding, -total_space_above + top_rounding),
        top_rounding,
        3.0,
    );

    // line down to baseline (center of the bigger radius)
    // will be done automatically

    // big smile consisting of two quadrants
    let mut smile: Vec<Pos2> = vec![];
    add_circle_quadrant(&mut smile, pos2(0., 0.), radius, 0.0);
    add_circle_quadrant(&mut smile, pos2(0., 0.), radius, 1.0);
    // it has to be in reverse order, because it goes counterclockwise
    smile.reverse();

    path.extend(smile.iter());

    // top right rounding
    add_circle_quadrant(
        &mut path,
        pos2(radius + top_rounding, -total_space_above + top_rounding),
        top_rounding,
        2.0,
    );

    path
}
