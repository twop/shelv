use eframe::{
    egui::{Response, Sense, Ui, Widget, WidgetInfo, WidgetType},
    epaint::{self, pos2, vec2, Color32, Rect, Stroke, Vec2},
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
        } = self;

        let box_size = radius * 2.0;
        let desired_size = vec2(
            box_size * (count as f32) + gap * ((count - 1) as f32),
            box_size,
        );

        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

        response.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, ""));

        if ui.is_rect_visible(rect) {
            // let visuals = ui.style().interact_selectable(&response, checked); // too colorful
            // let visuals = ui.style().interact(&response);

            let painter = ui.painter();

            let mut offset = rect.min.x;

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

                        let (fill, stroke) = match (
                            is_selected,
                            point_response.hovered(),
                            point_response.is_pointer_button_down_on(),
                        ) {
                            (true, _, _) => (selected, Stroke::NONE),
                            (_, true, false) => (Color32::TRANSPARENT, Stroke::new(2.0, hover)),
                            (_, _, true) => (Color32::TRANSPARENT, Stroke::new(2.0, pressed)),
                            _ => (Color32::TRANSPARENT, Stroke::new(1.0, inactive)),
                        };

                        painter.add(epaint::CircleShape {
                            center,
                            radius,
                            fill,
                            stroke,
                        });

                        offset += box_size
                    }
                    None => offset += gap,
                }
            }
        }

        response
    }
}
