use std::ops::Range;

use eframe::{
    egui::{
        self,
        text::CCursor,
        text_edit::{CCursorRange, TextEditOutput, TextEditState},
        Context, Id, KeyboardShortcut, Layout, Modifiers, OpenUrl, Painter, RichText, Sense,
        TopBottomPanel, Ui, Window,
    },
    emath::{Align, Align2},
    epaint::{pos2, vec2, Color32, FontId, Rect, Stroke, Vec2},
};
use smallvec::SmallVec;

use crate::{
    app_actions::{proccess_app_action, AppAction},
    app_state::{AppState, ComputedLayout, MsgToApp, Note},
    md_shortcut::{execute_instruction, MdAnnotationShortcut, ShortcutContext, Source},
    picker::{Picker, PickerItem},
    text_structure::{InteractiveTextPart, SpanKind, TextStructure},
    theme::{AppIcon, AppTheme},
};

#[derive(Debug)]
struct ShortcutExecContext<'a> {
    structure: &'a TextStructure,
    text: &'a str,
    selection_byte_range: Range<usize>,
    replace_range: Range<usize>,
}

impl<'a> ShortcutContext<'a> for ShortcutExecContext<'a> {
    fn get_source(&self, source: Source) -> Option<&'a str> {
        match source {
            Source::Selection => {
                if self.selection_byte_range.is_empty() {
                    None
                } else {
                    self.text.get(self.selection_byte_range.clone())
                }
            }
            Source::BeforeSelection => self.text.get(0..self.selection_byte_range.start),
            Source::AfterSelection => self.text.get(self.selection_byte_range.end..),
            Source::SurroundingSpanContent(kind) => self
                .structure
                .find_span_at(kind, self.selection_byte_range.clone())
                .map(|(_, index)| self.structure.get_span_inner_content(index))
                .and_then(|content| self.text.get(content.clone())),
        }
    }

    fn is_inside_span(&self, kind: SpanKind) -> bool {
        self.structure
            .find_span_at(kind, self.selection_byte_range.clone())
            .is_some()
    }

    fn set_replace_area(&mut self, kind: SpanKind) {
        if let Some((range, _)) = self
            .structure
            .find_span_at(kind, self.selection_byte_range.clone())
        {
            self.replace_range = range;
        }
    }

    fn is_inside_unmarked(&self) -> bool {
        self.structure
            .find_any_span_at(self.selection_byte_range.clone())
            .is_none()
    }
}

pub fn render_app(state: &mut AppState, ctx: &egui::Context, frame: &mut eframe::Frame) {
    // let text_edit_id = Id::new(("text_edit", state.selected_note));
    let text_edit_id = Id::new("text_edit");

    // if (state.hacky_render_count == 1) {
    //     state.hacky_render_count = 2;
    //     frame.focus();
    //     ctx.memory_mut(|mem| mem.request_focus(text_edit_id));
    //     println!("focus again");
    // }

    while let Ok(msg) = state.msg_queue.try_recv() {
        println!("got in render: {msg:?}");
        match msg {
            MsgToApp::ToggleVisibility => {
                state.hidden = !state.hidden;

                if state.hidden {
                    hide_app();
                } else {
                    frame.set_visible(!state.hidden);
                }

                if !state.hidden {
                    frame.focus();
                    // // frame.request_user_attention(egui::UserAttentionType::Reset);
                    ctx.memory_mut(|mem| mem.request_focus(text_edit_id));
                    println!(
                        "after: focus, has_focus = {:?}",
                        frame.info().window_info.focused
                    );
                }
            }
        }
    }

    let cur_focus = frame.info().window_info.focused;
    if state.prev_focused != cur_focus {
        if cur_focus {
            println!("gained focus");
            ctx.memory_mut(|mem| mem.request_focus(text_edit_id))
        } else {
            println!("lost focus");
            state.hidden = true;
            // frame.set_visible(!state.hidden);
            hide_app();
        }
        state.prev_focused = cur_focus;
    }

    if !state.hidden && !cur_focus {
        // println!("restore focus");
        frame.request_user_attention(egui::UserAttentionType::Informational);
        frame.focus()
    }

    // TEST code for cosuming input
    // ctx.input_mut(|input| {
    //     if input.consume_shortcut(&KeyboardShortcut::new(Modifiers::NONE, egui::Key::Enter)) {
    //         println!("Consumed Enter")
    //     }
    // });

    let mut actions = render_footer_panel(
        state.selected_note,
        state
            .notes
            .iter()
            .map(|n| format!("Shelf {}", ctx.format_shortcut(&n.shortcut)))
            .collect(),
        ctx,
        &state.theme,
    );

    ctx.input_mut(|input| {
        for (index, shortcut) in state.notes.iter().map(|n| n.shortcut).enumerate() {
            if input.consume_shortcut(&shortcut) {
                actions.push(AppAction::SwitchToNote {
                    index: index as u32,
                    via_shortcut: true,
                })
            }
        }
    });

    render_header_panel(ctx, &state.theme);

    for action in actions {
        proccess_app_action(action, ctx, state, text_edit_id);
    }

    let current_note = &mut state.notes[state.selected_note as usize];
    restore_cursor_from_note_state(&current_note, ctx, text_edit_id);

    egui::CentralPanel::default().show(ctx, |ui| {
        let avail_space = ui.available_rect_before_wrap();

        render_hints(
            &format!("{}", state.selected_note + 1),
            current_note
                .text
                .is_empty()
                .then(|| state.md_annotation_shortcuts.as_slice()),
            avail_space,
            ui.painter(),
            ctx,
            &state.theme,
        );

        // ---- TAB in LISTS ----
        // Note that it happens before rendering the panel
        if ui.input_mut(|input| input.modifiers.is_none() && input.key_pressed(egui::Key::Tab)) {
            if let (Some(mut text_edit_state), Some(computed_layout)) = (
                TextEditState::load(ctx, text_edit_id),
                &state.computed_layout,
            ) {
                use egui::TextBuffer;

                if let Some(ccursor_range) = text_edit_state.ccursor_range() {
                    let [ccursor_start, ccursor_end] = ccursor_range.sorted();
                    let byte_start = current_note
                        .text
                        .byte_index_from_char_index(ccursor_start.index);
                    let byte_end = current_note
                        .text
                        .byte_index_from_char_index(ccursor_start.index);

                    if let Some(inside_list_item) = computed_layout
                        .text_structure
                        .find_surrounding_list_item(byte_start..byte_end)
                    {
                        let chars_before = current_note.text
                            [0..inside_list_item.item_byte_pos.start]
                            .chars()
                            .count();

                        let cursor_before_list_item = computed_layout
                            .galley
                            .from_ccursor(CCursor::new(chars_before));

                        // only apply logic if the item is located on the same line as the cursor
                        match text_edit_state.cursor_range(&computed_layout.galley) {
                            Some(range)
                                if range.sorted().primary.rcursor.row
                                    == cursor_before_list_item.rcursor.row =>
                            {
                                let insertion = "\t";
                                let list_item_pos = inside_list_item.item_byte_pos.clone();

                                // TODO: normalize working with numbered lists

                                let inserted_chars_count = insertion.chars().count();
                                current_note.text.insert_str(list_item_pos.start, insertion);

                                text_edit_state.set_ccursor_range(Some(CCursorRange::two(
                                    ccursor_start + inserted_chars_count,
                                    ccursor_end + inserted_chars_count,
                                )));

                                text_edit_state.store(ctx, text_edit_id);

                                // Prevent TAB from modifying the text state
                                ui.input_mut(|input| {
                                    input.consume_key(Modifiers::NONE, egui::Key::Tab)
                                });
                            }
                            _ => (),
                        }
                    }
                }
            }
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

            let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                let computed_layout = match state.computed_layout.take() {
                    Some(layout) if !layout.should_recompute(text, wrap_width) => layout,

                    // TODO reuse the prev computed layout
                    _ => ComputedLayout::compute(
                        text,
                        wrap_width,
                        ui,
                        &state.theme,
                        &state.syntax_set,
                        &state.theme_set,
                    ),
                };

                let res = computed_layout.galley.clone();
                state.computed_layout = Some(computed_layout);

                res
            };

            // let mut edited_text = state.markdown.clone();

            let TextEditOutput {
                response: text_edit_response,
                galley,
                text_draw_pos,
                text_clip_rect: _,
                state: mut text_edit_state,
                mut cursor_range,
            } = egui::TextEdit::multiline(&mut current_note.text)
                .font(egui::TextStyle::Monospace) // for cursor height
                .code_editor()
                .id(text_edit_id)
                .lock_focus(true)
                .desired_width(f32::INFINITY)
                .frame(false)
                .layouter(&mut layouter)
                .show(ui);

            if text_edit_response.changed() {
                state.save_to_storage = true;
            }

            let space_below = ui.available_rect_before_wrap();

            // ---- CLICKING ON EMPTY AREA FOCUSES ON TEXT EDIT ----
            if space_below.height() > 0.
                && ui
                    .interact(space_below, Id::new("space_below"), Sense::click())
                    .clicked()
            {
                text_edit_state.set_ccursor_range(Some(egui::text::CCursorRange::one(
                    egui::text::CCursor::new(current_note.text.chars().count()),
                )));

                ctx.memory_mut(|mem| mem.request_focus(text_edit_id));
            }

            // ---- SHORTCUTS FOR MAKING BOLD/ITALIC/STRIKETHROUGH ----
            if let (Some(text_cursor_range), Some(computed_layout)) =
                (cursor_range, &state.computed_layout)
            {
                for md_shortcut in state.md_annotation_shortcuts.iter() {
                    if ui.input_mut(|input| input.consume_shortcut(&md_shortcut.shortcut)) {
                        use egui::TextBuffer as _;

                        let selected_char_range = text_cursor_range.as_sorted_char_range();

                        let byte_start = current_note
                            .text
                            .byte_index_from_char_index(selected_char_range.start);

                        let byte_end = current_note
                            .text
                            .byte_index_from_char_index(selected_char_range.end);

                        let span = computed_layout
                            .text_structure
                            .find_span_at(md_shortcut.target_span, byte_start..byte_end)
                            .map(|(span_range, idx)| {
                                (
                                    span_range,
                                    computed_layout.text_structure.get_span_inner_content(idx),
                                )
                            });

                        let [cursor_start, cursor_end] = match span {
                            Some((span_byte_range, content_byte_range)) => {
                                // we need to remove the annotations because it is already annotated
                                // for example: if it is already "bold" then remove "**" on each side

                                match (
                                    current_note
                                        .text
                                        .get(content_byte_range)
                                        .map(|s| s.to_string()),
                                    current_note
                                        .text
                                        .get(0..span_byte_range.start)
                                        .map(|s| s.chars().count()),
                                ) {
                                    (Some(inner_content), Some(span_char_offset)) => {
                                        current_note
                                            .text
                                            .replace_range(span_byte_range, &inner_content);

                                        let cursor_start = CCursor::new(span_char_offset);

                                        [cursor_start, cursor_start + inner_content.chars().count()]
                                    }
                                    _ => text_cursor_range.as_ccursor_range().sorted(),
                                }
                            }
                            None => {
                                // means that we need to execute instruction for the shortcut, presumably to add annotations
                                let mut cx = ShortcutExecContext {
                                    structure: &computed_layout.text_structure,
                                    text: &current_note.text,
                                    selection_byte_range: byte_start..byte_end,
                                    replace_range: byte_start..byte_end,
                                };

                                // println!("!! md shortcut:\n{:#?}", cx);
                                match execute_instruction(&mut cx, &md_shortcut.instruction) {
                                    Some(result) => {
                                        let cursor_start = CCursor::new(
                                            current_note.text[..cx.replace_range.start]
                                                .chars()
                                                .count(),
                                        );

                                        current_note.text.replace_range(
                                            cx.replace_range.clone(),
                                            &result.content,
                                        );

                                        [
                                            cursor_start + result.relative_char_cursor.start,
                                            cursor_start + result.relative_char_cursor.end,
                                        ]
                                    }
                                    None => text_cursor_range.as_ccursor_range().sorted(),
                                }
                            }
                        };

                        text_edit_state
                            .set_ccursor_range(Some(CCursorRange::two(cursor_start, cursor_end)));

                        cursor_range = text_edit_state.cursor_range(&galley);
                    }
                }
            }

            // ---- INTERACTIVE TEXT PARTS (TODO + LINKS) ----
            if let (Some(pointer_pos), Some(computed_layout)) =
                (ui.ctx().pointer_interact_pos(), &state.computed_layout)
            {
                let cursor = galley.cursor_from_pos(pointer_pos - text_draw_pos);
                use egui::TextBuffer;

                let byte_cursor = galley
                    .text()
                    .byte_index_from_char_index(cursor.ccursor.index);

                if let Some(interactive) = computed_layout
                    .text_structure
                    .find_interactive_text_part(byte_cursor)
                {
                    // if ui.input(|i| i.modifiers.command)
                    {
                        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                        if ui.input(|i| i.pointer.primary_clicked()) {
                            match interactive {
                                InteractiveTextPart::TaskMarker {
                                    byte_range,
                                    checked,
                                } => {
                                    current_note.text.replace_range(
                                        byte_range,
                                        if checked { "[ ]" } else { "[x]" },
                                    );
                                }
                                InteractiveTextPart::Link(url) => {
                                    println!("open url {url:}");
                                    ctx.output_mut(|output| output.open_url(url));
                                }
                            }
                        }
                    }
                }
            }

            // ---- AUTO INDENT LISTS ----
            if ui.input_mut(|input| input.key_pressed(egui::Key::Enter)) {
                if let (Some(text_cursor_range), Some(computed_layout)) =
                    (cursor_range, &state.computed_layout)
                {
                    use egui::TextBuffer;

                    let char_range = text_cursor_range.as_sorted_char_range();
                    let byte_start = current_note
                        .text
                        .byte_index_from_char_index(char_range.start);
                    let byte_end = current_note.text.byte_index_from_char_index(char_range.end);

                    let inside_item = computed_layout.text_structure.find_surrounding_list_item(
                        // note that "\n" was already inserted,
                        //thus we need to just look for "cursor_start -1" to detect a list item
                        if current_note.text[..byte_start].ends_with("\n") {
                            (byte_start - 1)..(byte_start - 1)
                        } else {
                            byte_start..byte_end
                        },
                    );

                    println!(
                        "\nnewline\nbefore_cursor='{}'\ncursor='{}'\nafter='{}'",
                        &current_note.text[0..byte_start],
                        &current_note.text[byte_start..byte_end],
                        &current_note.text[byte_end..]
                    );

                    if let Some(inside_list_item) = inside_item {
                        use egui::TextBuffer as _;

                        let text_to_insert = match inside_list_item.starting_index {
                            Some(starting_index) => format!(
                                "{}{}. ",
                                "\t".repeat(inside_list_item.depth as usize),
                                starting_index + inside_list_item.item_index as u64 + 1
                            ),
                            None => {
                                format!("{}- ", "\t".repeat(inside_list_item.depth as usize))
                            }
                        };

                        current_note
                            .text
                            .insert_text(text_to_insert.as_str(), char_range.start);

                        let [min, max] = text_cursor_range.as_ccursor_range().sorted();

                        // that byte size and char size of insertion are te same in this case
                        text_edit_state.set_ccursor_range(Some(CCursorRange::two(
                            min + text_to_insert.len(),
                            max + text_to_insert.len(),
                        )));
                    }
                }
            }

            current_note.cursor = text_edit_state.ccursor_range();
            text_edit_state.store(ui.ctx(), text_edit_id);
        });
    });

    if state.is_settings_opened {
        render_settings_dialog(ctx, &state.theme);
    }
}

fn hide_app() {
    // https://developer.apple.com/documentation/appkit/nsapplication/1428733-hide
    use objc2::rc::{Id, Shared};
    use objc2::runtime::Object;
    use objc2::{class, msg_send, msg_send_id};
    unsafe {
        let app: Id<Object, Shared> = msg_send_id![class!(NSApplication), sharedApplication];
        let arg = app.as_ref();
        let _: () = msg_send![&app, hide:arg];
    }
}

fn render_settings_dialog(ctx: &Context, theme: &AppTheme) {
    let mut window = Window::new("")
        .id("settings".into())
        // .open(&mut state.is_settings_opened)
        .title_bar(false)
        .anchor(Align2::CENTER_CENTER, [0., 0.])
        // .resizable(false)
        //.default_size((250., 200.))
        .fixed_size((250., 200.));

    window.show(&ctx, |ui| {
        ui.vertical(|ui| {
            let sizes = &theme.sizes;

            let avail_rect = ui.available_rect_before_wrap();
            ui.set_min_size(vec2(avail_rect.width(), avail_rect.height()));

            ui.painter().line_segment(
                [avail_rect.left(), avail_rect.right()]
                    .map(|x| pos2(x, avail_rect.top() + sizes.header_footer)),
                Stroke::new(1.0, theme.colors.outline_fg),
            );

            ui.allocate_ui(vec2(avail_rect.width(), theme.sizes.header_footer), |ui| {
                ui.with_layout(
                    Layout::centered_and_justified(egui::Direction::TopDown),
                    |ui| {
                        ui.label(
                            RichText::new("Settings")
                                .color(theme.colors.subtle_text_color)
                                .font(FontId {
                                    size: theme.fonts.size.normal,
                                    family: theme.fonts.family.bold.clone(),
                                }),
                        );
                    },
                );
                // ui.horizontal(|ui| {
                //     ui.label(
                //         RichText::new("Settings")
                //             .color(theme.colors.subtle_text_color)
                //             .font(FontId {
                //                 size: theme.fonts.size.normal,
                //                 family: theme.fonts.family.bold.clone(),
                //             }),
                //     );
                // })
            });

            ui.label("yo");
            if ui.button("close").clicked() {
                // state.is_settings_opened = false;
            }
        });
    });
}
fn restore_cursor_from_note_state(note: &Note, ctx: &Context, text_state_id: Id) {
    if let Some(mut text_edit_state) = egui::TextEdit::load_state(ctx, text_state_id) {
        // println!(
        //     "\nswitched\nnote_cursor='{:?}'\nstate_cursor='{:?}'",
        //     &note.cursor,
        //     &text_edit_state.ccursor_range(),
        // );
        let ccursor = note.cursor.unwrap_or_else(|| {
            egui::text::CCursorRange::one(egui::text::CCursor::new(note.text.chars().count()))
        });

        text_edit_state.set_ccursor_range(Some(ccursor));
        text_edit_state.store(ctx, text_state_id);
    }
}

fn render_footer_panel(
    selected: u32,
    tooltips: Vec<String>,
    ctx: &Context,
    theme: &AppTheme,
) -> SmallVec<[AppAction; 1]> {
    let mut current_selection = selected;
    let mut actions = SmallVec::new();
    TopBottomPanel::bottom("footer")
        // .exact_height(32.)
        .show_separator_line(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let sizes = &theme.sizes;
                let avail_width = ui.available_width();
                ui.set_min_size(vec2(avail_width, sizes.header_footer));

                set_menu_bar_style(ui);

                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.add(Picker {
                        current: &mut current_selection,
                        items: tooltips
                            .into_iter()
                            .map(|tooltip| PickerItem {
                                tooltip,
                                // tooltip: format!("Shelf {}", ctx.format_shortcut(&n.shortcut)),
                            })
                            .collect::<Vec<_>>()
                            .as_slice(),
                        gap: sizes.s,
                        radius: sizes.s,
                        inactive: theme.colors.outline_fg,
                        hover: theme.colors.button_hover_bg_stroke,
                        pressed: theme.colors.button_pressed_fg,
                        selected_stroke: theme.colors.button_fg,
                        selected_fill: theme.colors.button_bg,
                        outline: Stroke::new(1.0, theme.colors.outline_fg),
                        tooltip_text: theme.colors.subtle_text_color,
                    });
                });

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    for item in [
                        (
                            &AppIcon::Twitter,
                            "tweet us @shelvdotapp",
                            "https://twitter.com/shelvdotapp",
                        ),
                        (&AppIcon::Discord, "join our discrod", "https://shelv.app"),
                        (
                            &AppIcon::HomeSite,
                            "visit https://shelv.app",
                            "https://shelv.app",
                        ),
                        // (
                        //     &icons.at,
                        //     "e-mail us at hi@shelv.app",
                        //     "mailto:hi@shelv.app",
                        // ),
                    ]
                    .into_iter()
                    .map(Some)
                    .intersperse(None)
                    {
                        match item {
                            Some((icon, tooltip, url)) => {
                                let resp =
                                    ui.button(icon.render(
                                        sizes.toolbar_icon,
                                        theme.colors.subtle_text_color,
                                    ))
                                    .on_hover_ui(|ui| {
                                        ui.label(
                                            RichText::new(tooltip)
                                                .color(theme.colors.subtle_text_color),
                                        );
                                    });

                                if resp.clicked() {
                                    actions.push(AppAction::OpenLink(url.to_owned()));
                                    // ctx.open_url(OpenUrl::new_tab(url));
                                }
                            }
                            None => {
                                ui.add_space(theme.sizes.s);
                            }
                        }
                    }
                });
            });
        });

    if current_selection != selected {
        actions.push(AppAction::SwitchToNote {
            index: current_selection,
            via_shortcut: false,
        });
    }

    actions
}

fn set_menu_bar_style(ui: &mut egui::Ui) {
    let style = ui.style_mut();
    style.spacing.button_padding = vec2(0.0, 0.0);
    style.spacing.item_spacing = vec2(0.0, 0.0);
    style.visuals.widgets.active.bg_stroke = Stroke::NONE;
    style.visuals.widgets.hovered.bg_stroke = Stroke::NONE;
    style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
}

fn render_header_panel(ctx: &egui::Context, theme: &AppTheme) {
    TopBottomPanel::top("top_panel")
        .show_separator_line(false)
        .show(ctx, |ui| {
            // println!("-----");
            // println!("before menu {:?}", ui.available_size());
            ui.horizontal(|ui| {
                let sizes = &theme.sizes;

                let avail_width = ui.available_width();
                let avail_rect = ui.available_rect_before_wrap();
                ui.painter().line_segment(
                    [avail_rect.left(), avail_rect.right()]
                        .map(|x| pos2(x, avail_rect.top() + sizes.header_footer)),
                    Stroke::new(1.0, theme.colors.outline_fg),
                );
                ui.set_min_size(vec2(avail_width, sizes.header_footer));
                let icon_block_width = sizes.xl * 2.;

                set_menu_bar_style(ui);

                // println!("before x {:?}", ui.available_size());

                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.set_width(icon_block_width);

                    if ui
                        .button(AppIcon::Close.render(sizes.toolbar_icon, theme.colors.button_fg))
                        .clicked()
                    {}
                });

                // println!("before title {:?}", ui.available_size());

                ui.scope(|ui| {
                    ui.set_width(avail_width - 2. * icon_block_width);
                    ui.with_layout(
                        Layout::centered_and_justified(egui::Direction::LeftToRight),
                        |ui| {
                            ui.label(
                                RichText::new("Shelv")
                                    .color(theme.colors.subtle_text_color)
                                    .font(FontId {
                                        size: theme.fonts.size.normal,
                                        family: theme.fonts.family.bold.clone(),
                                    }),
                            );
                        },
                    );
                });

                // println!("before help {:?}", ui.available_size());

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.set_width(icon_block_width);

                    // Vec2::new(sizes.toolbar_icon, sizes.toolbar_icon),

                    let settings = ui
                        .button(
                            AppIcon::Settings.render(sizes.toolbar_icon, theme.colors.button_fg),
                        )
                        .on_hover_ui(|ui| {
                            ui.label(
                                RichText::new("open app settings")
                                    .color(theme.colors.subtle_text_color),
                            );
                        });

                    if settings.clicked() {
                        println!("clicked on settings");
                    }
                });
            });
        });
}

fn render_hints(
    title: &str,
    shortcuts: Option<&[MdAnnotationShortcut]>,
    available_space: Rect,
    painter: &Painter,
    cx: &egui::Context,
    theme: &AppTheme,
) {
    let AppTheme {
        fonts,
        colors,
        sizes,
    } = theme;

    let hint_color = theme.colors.outline_fg;

    // painter.text(
    //     available_space.center_top(), //+ vec2(0., sizes.header_footer)
    //     Align2::CENTER_TOP,
    //     title,
    //     FontId {
    //         size: fonts.size.h1,
    //         family: fonts.family.normal.clone(),
    //     },
    //     hint_color,
    // );

    match shortcuts {
        Some(shortcuts) if shortcuts.len() > 0 => {
            let hints_font_id = FontId {
                size: fonts.size.normal,
                family: fonts.family.normal.clone(),
            };

            let starting_point = available_space.center()
                - vec2(
                    0.,
                    ((2 * shortcuts.len() - 1) as f32) / 2.0 * fonts.size.normal,
                );

            for (i, md_shortcut) in shortcuts.iter().enumerate() {
                painter.text(
                    starting_point + vec2(0., (i as f32) * 2.0 * fonts.size.normal),
                    Align2::RIGHT_CENTER,
                    format!("{}  ", md_shortcut.name,),
                    hints_font_id.clone(),
                    hint_color,
                );
                painter.text(
                    starting_point + vec2(0., (i as f32) * 2.0 * fonts.size.normal),
                    Align2::CENTER_CENTER,
                    ":",
                    hints_font_id.clone(),
                    hint_color,
                );
                painter.text(
                    starting_point + vec2(0., (i as f32) * 2.0 * fonts.size.normal),
                    Align2::LEFT_CENTER,
                    format!("  {}", cx.format_shortcut(&md_shortcut.shortcut)),
                    hints_font_id.clone(),
                    hint_color,
                );
            }
        }
        _ => (),
    };
}
