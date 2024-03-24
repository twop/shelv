use eframe::{
    egui::{
        self, Context, Id, Response, RichText, Sense, Ui, Widget, WidgetInfo, WidgetType,
        WidgetWithState,
    },
    epaint::{
        self, pos2, tessellator::path::add_circle_quadrant, vec2, Color32, PathShape, Pos2, Rect,
        Shape, Stroke, Vec2,
    },
};

pub struct PickerItem {
    pub tooltip: String,
}

// TODO This is a pretty shitty enum. The reason is that the animation code is built around the currently selected note.
// When settings is selected, there is no current note. Either A) we move settings in here B) we make the animation logic
// central, instead of per note.
pub enum PickerSelection {
    Selected(u32),
    Deselected(u32),
}

pub struct Picker<'a> {
    pub current: &'a mut PickerSelection,
    pub items: &'a [PickerItem],
    pub gap: f32,
    pub radius: f32,
    // colors
    pub inactive: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub selected_stroke: Color32,
    pub selected_fill: Color32,
    pub tooltip_text: Color32,

    // drop
    pub outline: Stroke,
}

#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
// #[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
// #[cfg_attr(feature = "serde", serde(default))]
pub struct PickerState {}

impl PickerState {
    pub fn load(ctx: &Context, id: Id) -> Option<Self> {
        ctx.data_mut(|d| d.get_persisted(id))
    }

    pub fn store(self, ctx: &Context, id: Id) {
        ctx.data_mut(|d| d.insert_persisted(id, self));
    }
}

impl<'t> WidgetWithState for Picker<'t> {
    type State = PickerState;
}

impl<'a> Widget for Picker<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let Self {
            items,
            gap,
            radius,
            current,
            inactive,
            hover,
            pressed,
            selected_stroke,
            outline,
            selected_fill,
            tooltip_text,
        } = self;

        let box_size = radius * 2.0;
        let count = items.len() as u32;
        let desired_size = vec2(
            box_size * (count as f32) + gap * ((count - 1) as f32),
            box_size,
        );

        let available_rect = ui.available_rect_before_wrap();
        ui.add_space(radius * 2.);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::hover());
        // println!("allocated={:?}, available = {:?}", rect, avail);

        response.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, ""));

        if ui.is_rect_visible(rect) {
            // let visuals = ui.style().interact_selectable(&response, checked); // too colorful
            // let visuals = ui.style().interact(&response);

            let painter = ui.painter();

            let mut offset = rect.min.x;

            let ctx = ui.ctx();
            for i in (0..count).map(Some).intersperse_with(|| None) {
                match i {
                    Some(i) => {
                        let center = pos2(offset + radius, rect.center().y);
                        let rect = Rect::from_center_size(center, vec2(box_size, box_size));

                        let point_id = response.id.with(i);
                        let mut point_response = ui.interact(rect, point_id, Sense::click());

                        if point_response.clicked() {
                            *current = PickerSelection::Selected(i);
                        }

                        let is_current_index = match current {
                            PickerSelection::Selected(selected_index)
                            | PickerSelection::Deselected(selected_index) => *selected_index == i,
                        };

                        let is_selected = match current {
                            PickerSelection::Selected(selected_index) => *selected_index == i,
                            PickerSelection::Deselected(_) => false,
                        };

                        if !is_current_index {
                            let tooltip_ui = |ui: &mut egui::Ui| {
                                ui.label(
                                    RichText::new(&items[i as usize].tooltip).color(tooltip_text),
                                );
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

                        let center_y_target = if is_selected {
                            center.y - 1. * radius / 2.0
                        } else {
                            center.y
                        };
                        let center = pos2(center.x, center.y - selection_progress * radius / 2.0);

                        painter.add(epaint::CircleShape {
                            center,
                            radius,
                            fill,
                            stroke,
                        });

                        // Important Note: All of the animation code below needs a complete re-write.
                        // I was hacking to just see if I could make it work, but ran into a lot of trouble.
                        // TODO Fix it.
                        //  - Realized that we don't actually need to have each interpolation be its own animation.
                        //  - There's some complexity where certain values depend on other (now animated) values.
                        //  - There's also multiple different animations going on: Translation (selecting different note), Selection (settings <-> note).
                        if is_current_index {
                            let top_rounding = gap;

                            // the outline will be right in between dots
                            let drop_radius = radius + gap / 2.;

                            let drop_x_target = center.x;

                            let drop_x_animated = ctx.animate_value_with_time(
                                response.id.with("drop"),
                                drop_x_target,
                                0.2,
                            );

                            let drop_y_target = if is_selected {
                                center_y_target
                            } else {
                                available_rect.top()
                            };

                            let drop_y_animated = ctx.animate_value_with_time(
                                response.id.with("drop-y"),
                                drop_y_target,
                                0.2,
                            );

                            let drop_radius_target = if is_selected { drop_radius } else { 0.0 };
                            let drop_radius_animated = ctx.animate_value_with_time(
                                response.id.with("drop-radius"),
                                drop_radius_target,
                                0.2,
                            );

                            let top_rounding_target = if is_selected { top_rounding } else { 0.0 };
                            let top_rounding_animated = ctx.animate_value_with_time(
                                response.id.with("drop-top-rounding"),
                                top_rounding_target,
                                0.2,
                            );

                            let drop_pos_animated = pos2(drop_x_animated, drop_y_animated);

                            let half_drop_x_target = if is_selected {
                                available_rect.right()
                            } else {
                                available_rect.right() - radius - gap / 2.0
                            };

                            let half_drop_x_animated = ctx.animate_value_with_time(
                                response.id.with("half-drop-x"),
                                half_drop_x_target,
                                0.2,
                            );

                            let half_drop_y_target = if is_selected {
                                available_rect.top()
                            } else {
                                center_y_target
                            };

                            let half_drop_y_animated = ctx.animate_value_with_time(
                                response.id.with("half-drop-y"),
                                half_drop_y_target,
                                0.2,
                            );

                            let half_drop_radius_target =
                                if is_selected { 0.0 } else { radius + gap / 2. };
                            let half_drop_radius_animated = ctx.animate_value_with_time(
                                response.id.with("half-drop-radius"),
                                half_drop_radius_target,
                                0.2,
                            );

                            let half_top_rounding_target = if is_selected { 0.0 } else { gap };
                            let half_top_rounding_animated = ctx.animate_value_with_time(
                                response.id.with("half-drop-top_rounding"),
                                half_top_rounding_target,
                                0.2,
                            );

                            let half_drop_pos_animated =
                                pos2(half_drop_x_animated, half_drop_y_animated);

                            // let space_above = drop_pos.y - available_rect.top();
                            let space_above_target = if is_selected {
                                drop_y_target - available_rect.top()
                            } else {
                                0.0
                            };
                            let space_above_animated = ctx.animate_value_with_time(
                                response.id.with("drop-space-above"),
                                space_above_target,
                                0.2,
                            );

                            let half_drop_space_above_target = if is_selected {
                                0.0
                            } else {
                                half_drop_y_target - available_rect.top()
                            };
                            let half_drop_space_above_animated = ctx.animate_value_with_time(
                                response.id.with("half-drop-space-above"),
                                half_drop_space_above_target,
                                0.2,
                            );

                            // TODO, cache producing path in the widget state
                            let mut drop_shape = Shape::Path(PathShape {
                                points: drop_path(DropPathDesc {
                                    radius: drop_radius_animated,
                                    total_space_above: space_above_animated,
                                    top_rounding: top_rounding_animated,
                                }),
                                closed: false,
                                fill: Color32::TRANSPARENT,
                                stroke: outline,
                            });

                            let mut half_drop_shape = Shape::Path(PathShape {
                                points: half_drop_path(DropPathDesc {
                                    radius: half_drop_radius_animated,
                                    total_space_above: half_drop_space_above_animated,
                                    top_rounding: half_top_rounding_animated,
                                }),
                                closed: false,
                                fill: Color32::TRANSPARENT,
                                stroke: outline,
                            });

                            painter.line_segment(
                                [
                                    available_rect.left_top(),
                                    pos2(
                                        (drop_pos_animated.x
                                            - top_rounding_animated
                                            - drop_radius_animated)
                                            .max(available_rect.left()),
                                        available_rect.top(),
                                    ),
                                ],
                                outline,
                            );

                            painter.line_segment(
                                [
                                    pos2(
                                        (drop_pos_animated.x
                                            + top_rounding_animated
                                            + drop_radius_animated)
                                            .min(available_rect.right()),
                                        available_rect.top(),
                                    ),
                                    pos2(
                                        (half_drop_pos_animated.x
                                            - half_top_rounding_animated
                                            - half_drop_radius_animated)
                                            .min(available_rect.right()),
                                        available_rect.top(),
                                    ),
                                ],
                                outline,
                            );

                            drop_shape.translate(drop_pos_animated.to_vec2());
                            painter.add(drop_shape);

                            half_drop_shape.translate(half_drop_pos_animated.to_vec2());
                            painter.add(half_drop_shape);
                        }

                        offset += box_size
                    }
                    None => offset += gap,
                }
            }
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

// TODO need better name.
fn half_drop_path(
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
    add_circle_quadrant(&mut smile, pos2(0., 0.), radius, 1.0);
    // it has to be in reverse order, because it goes counterclockwise
    smile.reverse();

    path.extend(smile.iter());

    path
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
