use eframe::{
    egui::{
        self,
        text::CCursor,
        text_edit::{CCursorRange, TextEditOutput},
        Context, Id, KeyboardShortcut, Layout, Painter, RichText, Sense, TopBottomPanel, Ui,
        Window,
    },
    emath::{Align, Align2},
    epaint::{pos2, vec2, Color32, FontId, Rect, Stroke},
};
use smallvec::SmallVec;
use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};

use crate::{
    app_actions::{apply_text_changes, AppAction},
    app_state::{AppShortcuts, ComputedLayout, LayoutParams},
    byte_span::UnOrderedByteSpan,
    picker::{Picker, PickerItem},
    scripting::execute_live_scripts,
    text_structure::{InteractiveTextPart, TextStructure},
    theme::{AppIcon, AppTheme},
};

pub struct AppRenderData<'a> {
    pub selected_note: u32,
    pub text_edit_id: Id,
    pub font_scale: i32,
    pub byte_cursor: Option<UnOrderedByteSpan>,
    pub md_shortcuts: &'a [(String, KeyboardShortcut)],
    pub syntax_set: &'a SyntaxSet,
    pub theme_set: &'a ThemeSet,
    pub computed_layout: Option<ComputedLayout>,
    pub is_window_pinned: bool,
}

pub struct RenderAppResult(
    pub SmallVec<[AppAction; 4]>,
    pub TextStructure,
    pub Option<UnOrderedByteSpan>,
    pub Option<ComputedLayout>,
);

pub fn render_app(
    mut text_structure: TextStructure,
    editor_text: &mut String,
    visual_state: AppRenderData,
    shortcuts: &AppShortcuts,
    theme: &AppTheme,
    ctx: &egui::Context,
) -> RenderAppResult {
    let AppRenderData {
        selected_note,
        text_edit_id,
        font_scale,
        mut byte_cursor,
        md_shortcuts,
        computed_layout,
        syntax_set,
        theme_set,
        is_window_pinned,
    } = visual_state;

    let mut output_actions: SmallVec<[AppAction; 4]> = Default::default();

    let footer_actions = render_footer_panel(
        selected_note,
        font_scale,
        shortcuts
            .switch_to_note
            .iter()
            .map(|shortcut| format!("Shelf {}", ctx.format_shortcut(&shortcut)))
            .collect(),
        is_window_pinned,
        ctx,
        &theme,
    );

    output_actions.extend(footer_actions);

    // TODO migrate these to be commands
    // note that it is possible that commands might need to be more generalized
    // for example you should be able to switch to a note even withtout having a focus
    ctx.input_mut(|input| {
        for (index, shortcut) in shortcuts.switch_to_note.iter().enumerate() {
            if input.consume_shortcut(&shortcut) {
                output_actions.push(AppAction::SwitchToNote {
                    index: index as u32,
                    via_shortcut: true,
                })
            }
        }

        if input.consume_shortcut(&shortcuts.increase_font) {
            output_actions.push(AppAction::IncreaseFontSize);
        }
        if input.consume_shortcut(&shortcuts.decrease_font) {
            output_actions.push(AppAction::DecreaseFontSize);
        }
    });

    render_header_panel(ctx, theme);

    restore_cursor_from_note_state(&editor_text, byte_cursor, ctx, text_edit_id);

    let (text_structure, computed_layout, updated_cursor) = egui::CentralPanel::default()
        .show(ctx, |ui| {
            let avail_space = ui.available_rect_before_wrap();

            render_hints(
                &format!("{}", selected_note + 1),
                editor_text.is_empty().then(|| md_shortcuts),
                avail_space,
                ui.painter(),
                ctx,
                &theme,
            );

            egui::ScrollArea::vertical()
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

                    let (layout, mut text_structure, mut updated_cursor, text_draw_pos) =
                        render_editor(
                            ui,
                            editor_text,
                            text_structure,
                            font_scale,
                            computed_layout,
                            theme,
                            syntax_set,
                            theme_set,
                            text_edit_id,
                        );

                    let space_below = ui.available_rect_before_wrap();

                    // ---- CLICKING ON EMPTY AREA FOCUSES ON TEXT EDIT ----
                    // TODO migrate to use app actions
                    if space_below.height() > 0.
                        && ui
                            .interact(space_below, Id::new("space_below"), Sense::click())
                            .clicked()
                    {
                        updated_cursor =
                            Some(UnOrderedByteSpan::new(editor_text.len(), editor_text.len()));
                        ctx.memory_mut(|mem| mem.request_focus(text_edit_id));
                    }

                    // ---- INTERACTIVE TEXT PARTS (TODO + LINKS) ----
                    if let (Some(layout), Some(pointer_pos)) =
                        (&layout, ui.ctx().pointer_interact_pos())
                    {
                        let cursor = layout.galley.cursor_from_pos(pointer_pos - text_draw_pos);

                        use eframe::egui::TextBuffer as _;
                        let byte_cursor = layout
                            .galley
                            .text()
                            .byte_index_from_char_index(cursor.ccursor.index);

                        if let Some(interactive) =
                            text_structure.find_interactive_text_part(byte_cursor)
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
                                            editor_text.replace_range(
                                                byte_range.range(),
                                                if checked { "[ ]" } else { "[x]" },
                                            );

                                            text_structure = text_structure.recycle(&editor_text);
                                        }
                                        InteractiveTextPart::Link(url) => {
                                            println!("open url {url:}");
                                            output_actions
                                                .push(AppAction::OpenLink(url.to_string()));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    (text_structure, layout, updated_cursor)
                })
                .inner
        })
        .inner;

    RenderAppResult(
        output_actions,
        text_structure,
        updated_cursor,
        computed_layout,
    )
}

fn render_editor(
    ui: &mut Ui,
    editor_text: &mut String,
    text_structure: TextStructure,
    font_scale: i32,
    mut computed_layout: Option<ComputedLayout>,
    theme: &AppTheme,
    syntax_set: &SyntaxSet,
    theme_set: &ThemeSet,
    text_edit_id: Id,
) -> (
    Option<ComputedLayout>,
    TextStructure,
    Option<UnOrderedByteSpan>,
    egui::Pos2,
) {
    let mut structure_wrapper = Some(text_structure);

    let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
        let layout_cache_params = LayoutParams::new(text, wrap_width, font_scale);

        let layout = match computed_layout.take() {
            Some(layout) if !layout.should_recompute(&layout_cache_params) => layout,

            _ => {
                let scaled_theme = theme.scaled(f32::powi(1.2, font_scale));

                let structure = structure_wrapper.take().unwrap().recycle(text);

                // println!("### updated structure {structure:#?}");
                // println!("### rerender with {text}");
                let layout = ComputedLayout::compute(
                    &structure,
                    &layout_cache_params,
                    ui,
                    &scaled_theme,
                    syntax_set,
                    theme_set,
                );

                structure_wrapper = Some(structure);

                layout
            }
        };

        let res = layout.galley.clone();
        computed_layout = Some(layout);
        res
    };

    // let mut edited_text = state.markdown.clone();

    let TextEditOutput {
        response: text_edit_response,
        galley,
        text_draw_pos,
        text_clip_rect: _,
        state: _,
        cursor_range,
    } = egui::TextEdit::multiline(editor_text)
        .font(egui::TextStyle::Monospace) // for cursor height
        .code_editor()
        .id(text_edit_id)
        .lock_focus(true)
        .desired_width(f32::INFINITY)
        .frame(false)
        .layouter(&mut layouter)
        .show(ui);

    use egui::TextBuffer;

    let byte_cursor = cursor_range.map(|range| {
        let [start, end] = [range.secondary, range.primary]
            .map(|c| editor_text.byte_index_from_char_index(c.ccursor.index));

        UnOrderedByteSpan::new(start, end)
    });

    let text_structure = structure_wrapper.unwrap();
    (computed_layout, text_structure, byte_cursor, text_draw_pos)
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
fn restore_cursor_from_note_state(
    text: &str,
    byte_cursor: Option<UnOrderedByteSpan>,
    ctx: &Context,
    text_state_id: Id,
) {
    if let Some(mut text_edit_state) = egui::TextEdit::load_state(ctx, text_state_id) {
        let ccursor_range = byte_cursor.map(|unordered_span| {
            CCursorRange::two(
                CCursor::new(char_index_from_byte_index(&text, unordered_span.start)),
                CCursor::new(char_index_from_byte_index(&text, unordered_span.end)),
            )
        });

        if ccursor_range != text_edit_state.ccursor_range() {
            text_edit_state.set_ccursor_range(ccursor_range);
            text_edit_state.store(ctx, text_state_id);
        }
    }
}

fn render_footer_panel(
    selected: u32,
    font_size: i32,
    tooltips: Vec<String>,
    is_window_pinned: bool,
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

                // TODO Maybe this should be a global notification/toast UI instead of just font size.
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    let font_animation_id = ui.id().with("font_size");
                    let color_animation_id = ui.id().with("message_color");

                    let font_size_value =
                        ctx.animate_value_with_time(font_animation_id, font_size as f32, 2.0);
                    let show_font_message = font_size_value != font_size as f32;

                    let show_hide_value = ctx.animate_value_with_time(
                        color_animation_id,
                        if show_font_message { 1.0 } else { 0.0 },
                        0.2,
                    );
                    let interpolated_font_color = interpolate_color(
                        Color32::TRANSPARENT,
                        theme.colors.subtle_text_color,
                        show_hide_value,
                    );

                    ui.add_space(theme.sizes.xl);
                    ui.label(
                        RichText::new(format!("Font scaling set to {}", font_size))
                            .color(interpolated_font_color)
                            .font(FontId {
                                size: theme.fonts.size.normal,
                                family: theme.fonts.family.bold.clone(),
                            }),
                    );
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

                    // pin button
                    ui.add_space(theme.sizes.s);

                    // TODO either wait or fork egui-phosphor to update to phosphor 2.1
                    // which has "|" as a separator
                    ui.label(
                        AppIcon::VerticalSeparator
                            .render(sizes.toolbar_icon, theme.colors.subtle_text_color),
                    );
                    ui.add_space(theme.sizes.s);

                    let resp = ui
                        .button(AppIcon::Pin.render(
                            sizes.toolbar_icon,
                            if is_window_pinned {
                                theme.colors.button_fg
                            } else {
                                theme.colors.subtle_text_color
                            },
                        ))
                        .on_hover_ui(|ui| {
                            ui.label(
                                RichText::new(if is_window_pinned {
                                    "unpin window"
                                } else {
                                    "pin window"
                                })
                                .color(theme.colors.subtle_text_color),
                            );
                        });

                    // TODO handle that with shortcuts
                    if resp.clicked() {
                        actions.push(AppAction::SetWindowPinned(!is_window_pinned));
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
    shortcuts: Option<&[(String, KeyboardShortcut)]>,
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

            for (i, (name, shortcut)) in shortcuts.iter().enumerate() {
                painter.text(
                    starting_point + vec2(0., (i as f32) * 2.0 * fonts.size.normal),
                    Align2::RIGHT_CENTER,
                    format!("{}  ", name,),
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
                    format!("  {}", cx.format_shortcut(shortcut)),
                    hints_font_id.clone(),
                    hint_color,
                );
            }
        }
        _ => (),
    };
}

/// Count presses of a key. If non-zero, the presses are consumed, so that this will only return non-zero once.
///
/// Includes key-repeat events.
pub fn is_shortcut_match(input: &egui::InputState, shortcut: &KeyboardShortcut) -> bool {
    let KeyboardShortcut { modifiers, key } = shortcut.clone();

    input.events.iter().any(|event| {
        matches!(
            event,
            egui::Event::Key {
                key: ev_key,
                modifiers: ev_mods,
                pressed: true,
                ..
            } if *ev_key == key && ev_mods.matches(modifiers)
        )
    })
}

pub fn char_index_from_byte_index(s: &str, byte_index: usize) -> usize {
    for (ci, (bi, _)) in s.char_indices().enumerate() {
        if bi == byte_index {
            return ci;
        }
    }

    s.chars().count()
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
