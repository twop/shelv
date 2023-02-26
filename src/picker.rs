use eframe::{
    egui::{Context, Id, Response, Sense, Ui, Widget, WidgetInfo, WidgetType, WidgetWithState},
    epaint::{
        self, pos2, tessellator::path::add_circle_quadrant, vec2, Color32, PathShape, Pos2, Rect,
        Shape, Stroke, Vec2,
    },
};

pub struct Picker<'a> {
    pub current: &'a mut u32,
    pub count: u32,
    pub gap: f32,
    pub radius: f32,
    // colors
    pub inactive: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub selected: Color32,

    // drop
    pub outline: Stroke,
}

#[derive(Clone, Default)]
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
            count,
            gap,
            radius,
            current,
            inactive,
            hover,
            pressed,
            selected,
            outline,
        } = self;

        let box_size = radius * 2.0;
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
                        let point_response = ui.interact(rect, point_id, Sense::click());

                        if point_response.clicked() {
                            *current = i;
                        }

                        let is_selected = i == *current;

                        let selection_progress =
                            ctx.animate_bool_with_time(point_id, is_selected, 0.2);
                        let fill =
                            interpolate_color(Color32::TRANSPARENT, selected, selection_progress);

                        let stroke = match (
                            is_selected,
                            point_response.hovered(),
                            point_response.is_pointer_button_down_on(),
                        ) {
                            (true, _, _) => Stroke::NONE,
                            (_, true, false) => Stroke::new(1.5, hover),
                            (_, _, true) => Stroke::new(2.0, pressed),
                            _ => Stroke::new(1.0, inactive),
                        };

                        let center = pos2(center.x, center.y - selection_progress * radius / 2.0);

                        painter.add(epaint::CircleShape {
                            center,
                            radius,
                            fill,
                            stroke,
                        });

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
                                stroke: outline,
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
                                outline,
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
                                outline,
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
