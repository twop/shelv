use core::f32;

use eframe::{
    egui::{
        self,
        text::{CCursor, CCursorRange, LayoutJob},
        text_edit::TextEditOutput,
        Context, EventFilter, FontFamily, Id, Key, KeyboardShortcut, LayerId, Layout,
        ModifierNames, Modifiers, Painter, Response, RichText, Sense, TextEdit, TextFormat,
        TopBottomPanel, Ui, UiBuilder, UiStackInfo, Vec2, WidgetText, Window,
    },
    emath::{Align, Align2},
    epaint::{pos2, vec2, Color32, FontId, PathStroke, Rect, Stroke},
    Frame,
};
use itertools::Itertools;
use pulldown_cmark::CowStr;
// use itertools::Itertools;
use smallvec::SmallVec;
use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};

use crate::{
    app_actions::{AppAction, SlashPaletteAction},
    app_state::{
        ComputedLayout, InlineLLMPromptState, InlinePromptStatus, LayoutParams, MsgToApp,
        SlashPalette, SlashPaletteCmd,
    },
    byte_span::UnOrderedByteSpan,
    command::{BuiltInCommand, CommandList, PROMOTED_COMMANDS},
    commands::{
        inline_llm_prompt::{self, compute_inline_prompt_text_input_id},
        run_llm::LLM_LANG,
    },
    effects::text_change_effect::TextChange,
    persistent_state::NoteFile,
    picker::{Picker, PickerItem, PickerItemKind},
    settings::format_mac_shortcut,
    text_structure::{InteractiveTextPart, TextStructure},
    theme::{AppIcon, AppTheme},
};

pub struct AppRenderData<'a> {
    pub selected_note: NoteFile,
    pub note_count: usize,
    pub text_edit_id: Id,
    pub byte_cursor: Option<UnOrderedByteSpan>,
    pub command_list: &'a CommandList,
    pub syntax_set: &'a SyntaxSet,
    pub theme_set: &'a ThemeSet,
    pub computed_layout: Option<ComputedLayout>,
    pub inline_llm_prompt: Option<&'a mut InlineLLMPromptState>,
    pub slash_palette: Option<&'a SlashPalette>,
    pub is_window_pinned: bool,
}

pub struct RenderAppResult {
    pub requested_actions: SmallVec<[AppAction; 4]>,
    pub updated_text_structure: TextStructure,
    pub latest_cursor: Option<UnOrderedByteSpan>,
    pub latest_layout: Option<ComputedLayout>,
    pub text_changed: bool,
}

pub fn render_app(
    text_structure: TextStructure,
    editor_text: &mut String,
    visual_state: AppRenderData,
    theme: &AppTheme,
    ctx: &egui::Context,
) -> RenderAppResult {
    let AppRenderData {
        selected_note,
        text_edit_id,
        note_count,
        byte_cursor,
        command_list,
        computed_layout,
        syntax_set,
        theme_set,
        is_window_pinned,
        inline_llm_prompt,
        slash_palette,
    } = visual_state;

    let mut output_actions: SmallVec<[AppAction; 4]> = Default::default();

    let footer_actions = render_footer_panel(selected_note, note_count, command_list, ctx, &theme);
    output_actions.extend(footer_actions);

    let header_actions =
        render_header_panel(ctx, theme, command_list, selected_note, is_window_pinned);
    output_actions.extend(header_actions);

    restore_cursor_from_note_state(&editor_text, byte_cursor, ctx, text_edit_id);

    let (text_has_changed, text_structure, computed_layout, updated_cursor, editor_actions) =
        egui::CentralPanel::default()
            .show(ctx, |ui| {
                {
                    let avail_space = ui.available_rect_before_wrap();

                    let hints: Option<SmallVec<[(CowStr<'static>, KeyboardShortcut); 8]>> =
                        editor_text.is_empty().then(|| {
                            PROMOTED_COMMANDS
                                .into_iter()
                                .filter_map(|builtin| command_list.find(builtin))
                                .filter_map(|cmd| {
                                    cmd.kind.map(|k| k.human_description()).zip(cmd.shortcut)
                                })
                                .collect()
                        });

                    render_hints(
                        hints.as_ref().map(|hints| hints.as_slice()),
                        avail_space,
                        ui.painter(),
                        ctx,
                        &theme,
                    );
                }

                egui::ScrollArea::vertical()
                    .id_source(text_edit_id)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

                        //                        ctx.memory_ui(ui);

                        let (
                            changed,
                            layout,
                            mut text_structure,
                            mut updated_cursor,
                            text_draw_pos,
                            editor_actions,
                        ) = render_editor(
                            ui,
                            editor_text,
                            text_structure,
                            computed_layout,
                            inline_llm_prompt,
                            slash_palette,
                            theme,
                            syntax_set,
                            theme_set,
                            text_edit_id,
                            selected_note,
                            command_list,
                            ctx,
                        );

                        if changed {
                            output_actions.push(AppAction::EvalNote(selected_note))
                        }

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
                                                output_actions.push(AppAction::apply_text_changes(
                                                    selected_note,
                                                    [TextChange::Insert(
                                                        byte_range,
                                                        (if checked { "[ ]" } else { "[x]" })
                                                            .to_string(),
                                                    )]
                                                    .into(),
                                                ));

                                                text_structure =
                                                    text_structure.recycle(&editor_text);
                                            }
                                            InteractiveTextPart::Link(url) => {
                                                println!("open url {url:}");

                                                let parts: Vec<&str> =
                                                    url.split("://").collect_vec();
                                                let action = match parts.as_slice() {
                                                    ["shelv", "note1", ..] => {
                                                        AppAction::SwitchToNote {
                                                            note_file: NoteFile::Note(0),
                                                            via_shortcut: true,
                                                        }
                                                    }
                                                    ["shelv", "note2", ..] => {
                                                        AppAction::SwitchToNote {
                                                            note_file: NoteFile::Note(1),
                                                            via_shortcut: true,
                                                        }
                                                    }
                                                    ["shelv", "note3", ..] => {
                                                        AppAction::SwitchToNote {
                                                            note_file: NoteFile::Note(2),
                                                            via_shortcut: true,
                                                        }
                                                    }
                                                    ["shelv", "note4", ..] => {
                                                        AppAction::SwitchToNote {
                                                            note_file: NoteFile::Note(3),
                                                            via_shortcut: true,
                                                        }
                                                    }
                                                    ["shelv", "settings", ..] => {
                                                        AppAction::SwitchToNote {
                                                            note_file: NoteFile::Settings,
                                                            via_shortcut: true,
                                                        }
                                                    }
                                                    _ => AppAction::OpenLink(url.to_string()),
                                                };
                                                output_actions.push(action)
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        (
                            changed,
                            text_structure,
                            layout,
                            updated_cursor,
                            editor_actions,
                        )
                    })
                    .inner
            })
            .inner;

    output_actions.extend(editor_actions);
    RenderAppResult {
        requested_actions: output_actions,
        updated_text_structure: text_structure,
        latest_cursor: updated_cursor,
        latest_layout: computed_layout,
        text_changed: text_has_changed,
    }
}

fn render_editor(
    ui: &mut Ui,
    editor_text: &mut String,
    text_structure: TextStructure,
    mut computed_layout: Option<ComputedLayout>,
    inline_llm_prompt: Option<&mut InlineLLMPromptState>,
    slash_palette: Option<&SlashPalette>,
    theme: &AppTheme,
    syntax_set: &SyntaxSet,
    theme_set: &ThemeSet,
    text_edit_id: Id,
    note_file: NoteFile,
    command_list: &CommandList,
    ctx: &egui::Context,
) -> (
    bool, // if the text was changed, TODO rework this mess
    Option<ComputedLayout>,
    TextStructure,
    Option<UnOrderedByteSpan>,
    egui::Pos2,
    SmallVec<[AppAction; 1]>,
) {
    let mut resulting_actions: SmallVec<[AppAction; 1]> = SmallVec::new();
    let mut structure_wrapper = Some(text_structure);

    let estimated_text_pos = ui.next_widget_position();
    // let available_width = ui.available_width();

    let code_bg = ui.visuals().code_bg_color;
    let code_bg_rounding = ui.visuals().widgets.inactive.rounding;
    if let Some(computed_layout) = &computed_layout {
        for area in computed_layout.code_areas.iter() {
            let background_rect = area
                .rect
                .shrink(0.5)
                .translate(estimated_text_pos.to_vec2());

            ui.painter()
                .rect_filled(background_rect, code_bg_rounding, code_bg);
        }
    }

    let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
        let layout_cache_params = LayoutParams::new(text, wrap_width, ctx.pixels_per_point());

        let layout = match computed_layout.take() {
            Some(layout) if !layout.should_recompute(&layout_cache_params) => layout,

            _ => {
                let structure = structure_wrapper.take().unwrap().recycle(text);

                // println!("### updated structure w={avail_w} {layout_cache_params:#?}");
                //println!("### rerender with {text}");
                let layout = ComputedLayout::compute(
                    &structure,
                    &layout_cache_params,
                    ui,
                    theme,
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
        galley_pos,
        cursor_range,
        galley,
        ..
    } = egui::TextEdit::multiline(editor_text)
        .font(egui::TextStyle::Monospace) // for cursor height
        .code_editor()
        .id(text_edit_id)
        .lock_focus(true)
        .desired_width(f32::INFINITY)
        .frame(false)
        .layouter(&mut layouter)
        .show(ui);

    // floating buttons over code blocks
    ui.scope(|ui| {
        set_menu_bar_style(ui);

        if let Some(computed_layout) = &computed_layout {
            for (i, area) in computed_layout
                .code_areas
                .iter()
                .filter(|area| &area.lang == LLM_LANG)
                .enumerate()
            {
                let code_area = area.rect.translate(estimated_text_pos.to_vec2());

                {
                    let is_hovered = ui.rect_contains_pointer(code_area);

                    let mut ui = ui.new_child(
                        UiBuilder::new()
                            .max_rect(code_area.translate(Vec2::new(-theme.sizes.xs, 0.0)))
                            .layout(Layout::right_to_left(Align::TOP))
                            .ui_stack_info(UiStackInfo::new(egui::UiKind::GenericArea)),
                    );

                    let alpha = ui
                        .ctx()
                        .animate_bool(ui.id().with("hover").with(i), is_hovered);

                    let button_color = theme
                        .colors
                        .subtle_text_color
                        .gamma_multiply(0.2)
                        .lerp_to_gamma(theme.colors.button_fg, alpha);

                    let run_btn = ui
                        .button(AppIcon::Play.render(theme.sizes.toolbar_icon, button_color))
                        .on_hover_ui(|ui| {
                            let tooltip_text = "Execute code block";
                            let tooltip_text = command_list
                                .find(BuiltInCommand::RunLLMBlock)
                                .and_then(|cmd| cmd.shortcut)
                                .map(|shortcut| {
                                    format!("{} {}", tooltip_text, ctx.format_shortcut(&shortcut))
                                })
                                .unwrap_or_else(|| tooltip_text.to_string());

                            ui.label(
                                RichText::new(tooltip_text).color(theme.colors.subtle_text_color),
                            );
                        });

                    if run_btn.clicked() {
                        resulting_actions.push(AppAction::RunLLMBLock(
                            note_file,
                            area.code_block_span_index,
                        ));
                    }
                }
            }
        }
    });

    let text_structure = structure_wrapper.unwrap();

    let overlay_layer_width = galley.job.wrap.max_width - 2. * estimated_text_pos.x;

    match inline_llm_prompt {
        Some(inline_llm_prompt)
            if inline_llm_prompt.address.note_file == note_file
                && inline_llm_prompt.address.text_version == text_structure.opaque_version() =>
        {
            // TODO wtf is that?
            let mut top_of_frame = Rect::from_pos(estimated_text_pos);
            top_of_frame.set_width(overlay_layer_width);

            let (prompt_rect, prompt_actions) = render_inline_prompt(
                inline_llm_prompt,
                editor_text,
                &galley,
                top_of_frame,
                theme,
                ui,
                ctx,
                command_list,
            );

            let delta = prompt_rect.bottom() - text_edit_response.rect.bottom();
            if delta > 0. {
                // that means that overlay is outside text editor bounds
                // hence add that delta + some margin to expand scroll view
                ui.add_space(delta + theme.sizes.s);
            }

            resulting_actions.extend(prompt_actions);
        }

        Some(_) => {
            // reset inline prompt in case the text was modified
            resulting_actions.push(AppAction::AcceptPromptSuggestion { accept: false });
        }

        None => (),
    }

    if let Some(palette) = slash_palette {
        let slash_char_pos = char_index_from_byte_index(editor_text, palette.slash_byte_pos);
        let relative_slash_pos = galley.pos_from_ccursor(CCursor::new(slash_char_pos));

        let mut top_of_frame =
            Rect::from_pos(estimated_text_pos + vec2(0., relative_slash_pos.bottom()));
        top_of_frame.set_width(overlay_layer_width.min(theme.sizes.menu_width));

        let (palette_rect, palette_actions) =
            render_slash_palette(palette, top_of_frame, theme, ui);

        let delta = palette_rect.bottom() - text_edit_response.rect.bottom();
        if delta > 0. {
            // that means that overlay is outside text editor bounds
            // hence add that delta + some margin to expand scroll view
            ui.add_space(delta + theme.sizes.s);
        }

        resulting_actions.extend(palette_actions);
    }

    use egui::TextBuffer;

    let byte_cursor = cursor_range.map(|range| {
        let [start, end] = [range.secondary, range.primary]
            .map(|c| editor_text.byte_index_from_char_index(c.ccursor.index));

        UnOrderedByteSpan::new(start, end)
    });

    (
        text_edit_response.changed(),
        computed_layout,
        text_structure,
        byte_cursor,
        galley_pos,
        resulting_actions,
    )
}

fn render_inline_prompt(
    inline_llm_prompt: &mut InlineLLMPromptState,
    editor_text: &str,
    galley: &egui::Galley,
    top_of_frame: Rect,
    theme: &AppTheme,
    ui: &mut Ui,
    ctx: &Context,
    command_list: &CommandList,
) -> (Rect, SmallVec<[AppAction; 1]>) {
    let mut resulting_actions = SmallVec::new();

    let [relative_selection_start, relative_selection_end] = [
        inline_llm_prompt.address.span.start,
        inline_llm_prompt.address.span.end,
    ]
    .map(|pos| {
        let char_pos = char_index_from_byte_index(editor_text, pos);
        galley.pos_from_ccursor(CCursor::new(char_pos))
    });

    let prompt_ui_rect = Rect::from_pos(pos2(
        top_of_frame.left(),
        relative_selection_end.bottom() + top_of_frame.top() + theme.sizes.xs,
    ));

    let mut prompt_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(prompt_ui_rect)
            .layout(Layout::top_down(Align::LEFT))
            .ui_stack_info(UiStackInfo::new(egui::UiKind::GenericArea)),
    );

    prompt_ui.painter().line_segment(
        [
            pos2(
                top_of_frame.left(),
                relative_selection_start.top() + top_of_frame.top(),
            ),
            pos2(
                top_of_frame.left(),
                relative_selection_end.bottom() + top_of_frame.top(),
            ),
        ],
        Stroke::new(1., theme.colors.subtle_text_color),
    );

    let frame_resp = egui::Frame::none()
        .fill(theme.colors.code_bg_color)
        .inner_margin(theme.sizes.s)
        .stroke(prompt_ui.visuals().window_stroke)
        .shadow(prompt_ui.visuals().window_shadow)
        .rounding(prompt_ui.visuals().window_rounding)
        .show(&mut prompt_ui, |ui| {
            ui.set_min_width(top_of_frame.width());
            let inline_prompt_address = inline_llm_prompt.address;
            // ui.memory(|m| m.set_focus_lock_filter(id, event_filter);)

            let prompt_text_id = compute_inline_prompt_text_input_id(inline_prompt_address);

            let is_focused = ctx.memory(|m| m.focused()) == Some(prompt_text_id);

            // TODO move that into a command instead
            if is_focused {
                if ui.input_mut(|input| {
                    input
                        .consume_shortcut(&KeyboardShortcut::new(Modifiers::NONE, egui::Key::Enter))
                }) {
                    resulting_actions.push(match &inline_llm_prompt.status {
                        InlinePromptStatus::NotStarted => {
                            if inline_llm_prompt.prompt.is_empty() {
                                // that will hide the UI
                                AppAction::AcceptPromptSuggestion { accept: false }
                            } else {
                                AppAction::ExecutePrompt
                            }
                        }
                        InlinePromptStatus::Streaming { .. } => {
                            AppAction::AcceptPromptSuggestion { accept: false }
                        }
                        InlinePromptStatus::Done { prompt } => {
                            if prompt == &inline_llm_prompt.prompt {
                                AppAction::AcceptPromptSuggestion { accept: true }
                            } else {
                                AppAction::ExecutePrompt
                            }
                        }
                    });
                }
            }

            let prompt_input_resp = TextEdit::multiline(&mut inline_llm_prompt.prompt)
                .id(prompt_text_id)
                .desired_width(f32::INFINITY)
                .frame(false)
                .desired_rows(1)
                .desired_width(f32::INFINITY)
                .hint_text("Prompt AI ...")
                .show(ui);

            if prompt_input_resp.response.gained_focus() {
                // println!("prompt input gained focus");
                prompt_input_resp.response.scroll_to_me(Some(Align::Center));
            }

            ui.add_space(theme.sizes.s);
            ui.separator();
            ui.add_space(theme.sizes.s);
            let render_btn = |ui: &mut Ui, icon: AppIcon, text| {
                ui.button(icon.render_with_text(
                    theme.fonts.size.normal,
                    theme.colors.normal_text_color,
                    text,
                ))
            };

            let render_tooltip =
                |ui: &mut Ui, tooltip_text: &str, shortcut: Option<KeyboardShortcut>| {
                    ui.label({
                        RichText::new(tooltip_text)
                            // RichText::new(match shortcut {
                            //     Some(shortcut) => format!(
                            //         "{} {}",
                            //         tooltip_text,
                            //         ctx.format_shortcut(&shortcut)
                            //     ),
                            //     None => tooltip_text.to_string(),
                            // })
                            .color(theme.colors.subtle_text_color)
                    })
                };
            let just_enter_shortcut = KeyboardShortcut::new(Modifiers::NONE, Key::Enter);

            ui.horizontal(|ui| match &inline_llm_prompt.status {
                InlinePromptStatus::NotStarted => {
                    if render_btn(ui, AppIcon::Play, "Run")
                        .on_hover_ui(|ui| {
                            render_tooltip(ui, "Prompt AI", Some(just_enter_shortcut));
                        })
                        .clicked()
                    {
                        resulting_actions.push(AppAction::ExecutePrompt);
                    }
                    ui.add_space(theme.sizes.s);
                    if render_btn(ui, AppIcon::Close, "Cancel")
                        .on_hover_ui(|ui| {
                            render_tooltip(
                                ui,
                                "Cancel prompt",
                                command_list
                                    .find(BuiltInCommand::HidePrompt)
                                    .and_then(|cmd| cmd.shortcut),
                            );
                        })
                        .clicked()
                    {
                        resulting_actions.push(AppAction::AcceptPromptSuggestion { accept: false });
                    }
                }

                InlinePromptStatus::Streaming { .. } => {
                    ui.spinner();
                }

                InlinePromptStatus::Done { prompt } => {
                    if prompt == &inline_llm_prompt.prompt
                        && render_btn(ui, AppIcon::Accept, "Accept")
                            .on_hover_ui(|ui| {
                                render_tooltip(ui, "Accept suggestions", Some(just_enter_shortcut));
                            })
                            .clicked()
                    {
                        resulting_actions.push(AppAction::AcceptPromptSuggestion { accept: true });
                    }

                    if prompt != &inline_llm_prompt.prompt
                        && render_btn(ui, AppIcon::Play, "Re-run")
                            .on_hover_ui(|ui| {
                                render_tooltip(
                                    ui,
                                    "Re-run using the new prompt",
                                    Some(just_enter_shortcut),
                                );
                            })
                            .clicked()
                    {
                        resulting_actions.push(AppAction::ExecutePrompt);
                    }

                    ui.add_space(theme.sizes.s);
                    if render_btn(ui, AppIcon::Close, "Cancel")
                        .on_hover_ui(|ui| {
                            render_tooltip(
                                ui,
                                "Reject changes",
                                command_list
                                    .find(BuiltInCommand::HidePrompt)
                                    .and_then(|cmd| cmd.shortcut),
                            );
                        })
                        .clicked()
                    {
                        resulting_actions.push(AppAction::AcceptPromptSuggestion { accept: false });
                    }
                }
            });

            if !inline_llm_prompt.response_text.is_empty() {
                ui.add_space(theme.sizes.s);
                ui.separator();
                ui.add_space(theme.sizes.s);
                ui.label(WidgetText::LayoutJob(inline_llm_prompt.layout_job.clone()));
            }
        })
        .response;

    if inline_llm_prompt.fresh_response {
        inline_llm_prompt.fresh_response = false;
        frame_resp.scroll_to_me(None);
    }

    (frame_resp.rect, resulting_actions)
}

fn render_slash_palette(
    slash_palette: &SlashPalette,
    top_of_frame: Rect,
    theme: &AppTheme,
    ui: &mut Ui,
    // command_list: &CommandList,
) -> (Rect, SmallVec<[AppAction; 1]>) {
    let mut resulting_actions = SmallVec::new();

    let prompt_ui_rect = Rect::from_pos(pos2(
        top_of_frame.left(),
        top_of_frame.top() + theme.sizes.xs,
    ));

    let mut prompt_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(prompt_ui_rect)
            .layout(Layout::top_down(Align::LEFT))
            .ui_stack_info(UiStackInfo::new(egui::UiKind::GenericArea)),
    );

    // prompt_ui.painter().line_segment(
    //     [
    //         pos2(
    //             top_of_frame.left(),
    //             relative_selection_start.top() + top_of_frame.top(),
    //         ),
    //         pos2(
    //             top_of_frame.left(),
    //             relative_selection_end.bottom() + top_of_frame.top(),
    //         ),
    //     ],
    //     Stroke::new(1., theme.colors.subtle_text_color),
    // );

    let frame_resp = egui::Frame::none()
        .fill(theme.colors.code_bg_color)
        .inner_margin(theme.sizes.s)
        .stroke(prompt_ui.visuals().window_stroke)
        .shadow(prompt_ui.visuals().window_shadow)
        .rounding(prompt_ui.visuals().window_rounding)
        .show(&mut prompt_ui, |ui| {
            set_menu_bar_style(ui);

            ui.layout();

            ui.set_min_width(top_of_frame.width());

            let mut responses: SmallVec<[Response; 10]> = SmallVec::new();

            for item in
                Itertools::intersperse(slash_palette.options.iter().enumerate().map(Some), None)
            {
                match item {
                    Some((i, cmd)) => {
                        let resp = render_slash_cmd(ui, theme, cmd);

                        if resp.clicked() {
                            println!("clicked on {cmd:#?}");
                            resulting_actions.push(AppAction::SlashPalette(
                                SlashPaletteAction::ExecuteCommand(i),
                            ));
                        }

                        responses.push(resp);
                    }

                    None => {
                        ui.add_space(theme.sizes.s);
                        ui.separator();
                        ui.add_space(theme.sizes.s)
                    }
                }
            }

            let is_any_hovered = responses.iter().any(|r| r.hovered());
            if !is_any_hovered {
                for (i, resp) in responses.into_iter().enumerate() {
                    if i == slash_palette.selected {
                        resp.highlight();
                    }
                }
            }
        })
        .response;

    // if inline_llm_prompt.fresh_response {
    //     inline_llm_prompt.fresh_response = false;
    //     frame_resp.scroll_to_me(None);
    // }

    (frame_resp.rect, resulting_actions)
}

fn render_slash_cmd(ui: &mut Ui, theme: &AppTheme, cmd: &SlashPaletteCmd) -> egui::Response {
    let mut layout_job = LayoutJob::default();

    let phosphor_icon_font = TextFormat::simple(
        FontId::new(theme.fonts.size.h4, FontFamily::Name("phosphor".into())),
        theme.colors.normal_text_color,
    );

    let header_font = TextFormat::simple(
        FontId::new(theme.fonts.size.normal, theme.fonts.family.normal.clone()),
        theme.colors.normal_text_color,
    );

    let shortcut_font = TextFormat::simple(
        FontId::new(theme.fonts.size.small, theme.fonts.family.italic.clone()),
        theme.colors.subtle_text_color,
    );

    let description_font = TextFormat::simple(
        FontId::new(theme.fonts.size.small, theme.fonts.family.normal.clone()),
        theme.colors.subtle_text_color,
    );

    if let Some(icon) = &cmd.phosphor_icon {
        layout_job.append(&icon, 0., phosphor_icon_font.clone());
        layout_job.append(" ", 0., phosphor_icon_font.clone());
    }

    layout_job.append(&cmd.prefix, 0., header_font.clone());

    if let Some(shortcut) = &cmd.shortcut {
        layout_job.append("\t", 0., header_font.clone());
        layout_job.append(
            &format!("{}", format_mac_shortcut(*shortcut)),
            0.,
            shortcut_font.clone(),
        );
    }

    layout_job.append("\n", 0., header_font);
    layout_job.append(&cmd.description, 0., description_font);

    ui.button(WidgetText::LayoutJob(layout_job))
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
    selected: NoteFile,
    note_count: usize,
    command_list: &CommandList,
    ctx: &Context,
    theme: &AppTheme,
) -> SmallVec<[AppAction; 1]> {
    let tooltips: SmallVec<[String; 6]> = (0..note_count)
        .map(|note_index| BuiltInCommand::SwitchToNote(note_index as u8))
        .map(|cmd| command_list.find(cmd))
        .enumerate()
        .map(|(note_index, cmd)| match cmd.and_then(|cmd| cmd.shortcut) {
            Some(shortcut) => format!("Shelf {}", ctx.format_shortcut(&shortcut)),
            None => format!("Shelf {}", note_index + 1),
        })
        .collect();

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
                    let items = tooltips
                        .into_iter()
                        .enumerate()
                        .map(|(index, tooltip)| PickerItem {
                            tooltip,
                            kind: PickerItemKind::FontIcon(
                                match index {
                                    0 => AppIcon::One,
                                    1 => AppIcon::Two,
                                    2 => AppIcon::Three,
                                    3 => AppIcon::Four,
                                    // should not be reachable
                                    _ => AppIcon::More,
                                }
                                .to_icon_str(),
                                FontFamily::Proportional,
                            ),
                            data: NoteFile::Note(index as u32),
                        })
                        .chain([PickerItem {
                            tooltip: {
                                let tooltip_text = "Settings";
                                command_list
                                    .find(BuiltInCommand::SwitchToSettings)
                                    .and_then(|cmd| cmd.shortcut)
                                    .map(|shortcut| {
                                        format!(
                                            "{} {}",
                                            tooltip_text,
                                            ctx.format_shortcut(&shortcut)
                                        )
                                    })
                                    .unwrap_or_else(|| tooltip_text.to_string())
                            },
                            kind: PickerItemKind::FontIcon(
                                AppIcon::Settings.to_icon_str(),
                                FontFamily::Proportional,
                            ),
                            data: NoteFile::Settings,
                        }])
                        .collect::<Vec<_>>();

                    let picker = Picker {
                        current: match selected {
                            NoteFile::Note(i) => i as usize,
                            NoteFile::Settings => note_count,
                        },
                        items: &items,
                        gap: sizes.s,
                        // TODO why the button icons are rendered with h3 font size?
                        item_size: theme.sizes.toolbar_icon,
                        inactive_color: theme.colors.subtle_text_color,
                        hover_color: theme.colors.button_hover_fg,
                        pressed_color: theme.colors.button_pressed_fg,
                        selected_stroke_color: theme.colors.button_pressed_fg,
                        selected_fill_color: theme.colors.button_pressed_fg,
                        outline: PathStroke::new(1.0, theme.colors.outline_fg),
                        tooltip_text_color: theme.colors.subtle_text_color,
                    };

                    if let Some(&note_file) = picker.show(ui).inner {
                        actions.push(AppAction::SwitchToNote {
                            note_file,
                            via_shortcut: false,
                        });
                    }
                });

                // TODO Maybe this should be a global notification/toast UI instead of just font size.
                // ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                //     let font_animation_id = ui.id().with("font_size");
                //     let color_animation_id = ui.id().with("message_color");

                //     let font_size_value =
                //         ctx.animate_value_with_time(font_animation_id, font_size as f32, 2.0);
                //     let show_font_message = font_size_value != font_size as f32;

                //     let show_hide_value = ctx.animate_value_with_time(
                //         color_animation_id,
                //         if show_font_message { 1.0 } else { 0.0 },
                //         0.2,
                //     );
                //     let interpolated_font_color = interpolate_color(
                //         Color32::TRANSPARENT,
                //         theme.colors.subtle_text_color,
                //         show_hide_value,
                //     );

                //     ui.add_space(theme.sizes.xl);
                //     ui.label(
                //         RichText::new(format!("Font scaling set to {}", font_size))
                //             .color(interpolated_font_color)
                //             .font(FontId {
                //                 size: theme.fonts.size.normal,
                //                 family: theme.fonts.family.bold.clone(),
                //             }),
                //     );
                // });

                let icon_block_width = sizes.xl * 2.;

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.set_width(icon_block_width);

                    let share_btn = ui
                        .button(
                            AppIcon::Feedback.render(sizes.toolbar_icon, theme.colors.button_fg),
                        )
                        .on_hover_ui(|ui| {
                            ui.label(
                                RichText::new("Send this note to report a bug or share feedback.")
                                    .color(theme.colors.subtle_text_color),
                            );
                        });

                    if share_btn.clicked() {
                        println!("clicked on shared");
                        actions.push(AppAction::SendFeedback(selected));
                    }
                });
            });
        });

    actions
}

fn set_menu_bar_style(ui: &mut egui::Ui) {
    let style = ui.style_mut();
    // TODO 2 seems better (more square, but we need to take the value from theme or soemthing)
    style.spacing.button_padding = vec2(2., 0.0);
    style.spacing.item_spacing = vec2(0.0, 0.0);
    style.visuals.widgets.active.bg_stroke = Stroke::NONE;
    style.visuals.widgets.hovered.bg_stroke = Stroke::NONE;
    style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
}

fn render_header_panel(
    ctx: &egui::Context,
    theme: &AppTheme,
    command_list: &CommandList,
    selected_note: NoteFile,
    is_window_pinned: bool,
) -> SmallVec<[AppAction; 1]> {
    TopBottomPanel::top("top_panel")
        .show_separator_line(false)
        .show(ctx, |ui| {
            let mut resulting_actions: SmallVec<[AppAction; 1]> = Default::default();
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
                        .button(
                            AppIcon::Close
                                .render(sizes.toolbar_icon, theme.colors.subtle_text_color),
                        )
                        .on_hover_ui(|ui| {
                            ui.label({
                                RichText::new("Hide Shelv").color(theme.colors.subtle_text_color)
                            });
                        })
                        .clicked()
                    {
                        resulting_actions
                            .push(AppAction::HandleMsgToApp(MsgToApp::ToggleVisibility))
                    }

                    ui.add_space(theme.sizes.m);

                    ui.label(
                        RichText::new(format!(
                            "Shelv - {}",
                            match selected_note {
                                NoteFile::Note(index) => format!("note {}", index + 1),
                                NoteFile::Settings => "settings".to_string(),
                            }
                        ))
                        .color(theme.colors.subtle_text_color)
                        .font(FontId {
                            size: theme.fonts.size.normal,
                            family: theme.fonts.family.bold.clone(),
                        }),
                    );
                });

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.menu_button(
                        AppIcon::Menu.render(sizes.toolbar_icon, theme.colors.subtle_text_color),
                        |ui| {
                            ui.set_max_width(200.0);

                            if ui
                                .button(AppIcon::Tutorial.render_with_text(
                                    theme.fonts.size.normal,
                                    theme.colors.normal_text_color,
                                    "Start tutorial",
                                ))
                                .clicked()
                            {
                                ui.close_menu();
                                resulting_actions.push(AppAction::StartTutorial);
                            }

                            ui.separator();

                            for (icon, text, link) in [
                                (
                                    &AppIcon::Discord,
                                    "Join our Discord",
                                    "https://discord.gg/sSGHwNKy",
                                ),
                                (
                                    &AppIcon::Twitter,
                                    "Tweet us @shelvdotapp",
                                    "https://twitter.com/shelvdotapp",
                                ),
                                (
                                    &AppIcon::HomeSite,
                                    "Visit https://shelv.app",
                                    "https://shelv.app",
                                ),
                            ] {
                                if ui
                                    .button(icon.render_with_text(
                                        theme.fonts.size.normal,
                                        theme.colors.normal_text_color,
                                        text,
                                    ))
                                    .clicked()
                                {
                                    ui.close_menu();
                                    resulting_actions.push(AppAction::OpenLink(link.to_string()));
                                }
                            }

                            ui.separator();

                            if ui
                                .button(AppIcon::Folder.render_with_text(
                                    theme.fonts.size.normal,
                                    theme.colors.normal_text_color,
                                    "Open notes folder",
                                ))
                                .clicked()
                            {
                                ui.close_menu();
                                resulting_actions.push(AppAction::OpenNotesInFinder);
                            }
                        },
                    );

                    // ui.add_space(theme.sizes.s);
                    ui.label(
                        AppIcon::VerticalSeparator
                            .render(sizes.toolbar_icon, theme.colors.outline_fg),
                    );
                    // ui.add_space(theme.sizes.s);

                    let resp = ui
                        .button(AppIcon::Pin.render(
                            sizes.toolbar_icon,
                            if is_window_pinned {
                                theme.colors.button_pressed_fg
                            } else {
                                theme.colors.subtle_text_color
                            },
                        ))
                        .on_hover_ui(|ui| {
                            let tooltip_text = if is_window_pinned {
                                "Unpin window"
                            } else {
                                "Pin window"
                            };

                            let tooltip_text = command_list
                                .find(BuiltInCommand::PinWindow)
                                .and_then(|cmd| cmd.shortcut)
                                .map(|shortcut| {
                                    format!("{} {}", tooltip_text, ctx.format_shortcut(&shortcut))
                                })
                                .unwrap_or_else(|| tooltip_text.to_string());

                            ui.label(
                                RichText::new(tooltip_text).color(theme.colors.subtle_text_color),
                            );
                        });

                    // TODO handle that with shortcuts
                    if resp.clicked() {
                        resulting_actions.push(AppAction::SetWindowPinned(!is_window_pinned));
                    }
                });

                // println!("before help {:?}", ui.available_size());
            });

            resulting_actions
        })
        .inner
}

fn render_hints(
    shortcuts: Option<&[(CowStr<'static>, KeyboardShortcut)]>,
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
    let KeyboardShortcut {
        modifiers,
        logical_key,
    } = shortcut.clone();

    input.events.iter().any(|event| {
        matches!(
            event,
            egui::Event::Key {
                key: ev_key,
                modifiers: ev_mods,
                pressed: true,
                ..
            } if *ev_key == logical_key && ev_mods.matches_exact(modifiers)
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
