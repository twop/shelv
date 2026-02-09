use eframe::{
    egui::{
        self, Context, CursorIcon, FontFamily, FontSelection, Frame, Id, Key, KeyboardShortcut,
        Label, Layout, Margin, Modal, Modifiers, Painter, Response, RichText, ScrollArea, Sense,
        Spacing, TextEdit, TextFormat, TextStyle, TextWrapMode, TopBottomPanel, Ui, UiBuilder,
        UiKind, UiStackInfo, Vec2, WidgetText,
        scroll_area::ScrollBarVisibility,
        text::{CCursor, CCursorRange},
        text_edit::TextEditOutput,
        text_selection::text_cursor_state::cursor_rect,
    },
    emath::{self, Align, Align2},
    epaint::{Color32, FontId, Rect, Stroke, pos2, vec2},
};
use egui_taffy::{
    TuiBuilderLogic,
    taffy::{AlignContent, AlignItems, FlexDirection, FlexboxItemStyle, JustifyContent},
    tui,
};
use itertools::Itertools;
use pulldown_cmark::CowStr;
// use itertools::Itertools;
use shared::Version;
use smallvec::SmallVec;
use smol_str::ToSmolStr;
use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};

use crate::{
    actions::word_jump::{JumpCharSequence, JumpLabel, JumpLabelMatchResult},
    app_actions::{
        AppAction, AppNotification, AppNotificationAction, FocusTarget, SlashPaletteAction,
        SwitchToNoteTarget, WordJumpAction,
    },
    app_state::{
        CodeBlockAnnotation, ComputedLayout, FeedbackState, InlineLLMPromptState,
        InlinePromptStatus, LayoutParams, NoteSignature, RenderAction, SlashPalette, VersionState,
        WordJumpState,
    },
    byte_span::UnOrderedByteSpan,
    command::{
        CommandContext, CommandInstruction, CommandList, EditorCommandOutput, FrameHotkey,
        FrameHotkeys, PROMOTED_COMMANDS, SlashPaletteCmd,
    },
    commands::inline_llm_prompt::compute_inline_prompt_text_input_id,
    effects::text_change_effect::TextChange,
    feedback::{Feedback, FeedbackResult},
    persistent_state::{ExternalFile, NoteId},
    settings_parsing::format_mac_shortcut_with_symbols,
    taffy_styles::{StyleBuilder, flex_column, flex_row},
    text_structure::{InteractiveTextPart, SpanIndex, TextStructure},
    theme::{AppIcon, AppTheme},
    ui::{
        NotificationId, Notifications, footer_ui::render_footer_panel,
        notifications::NotificationItem,
    },
    ui_components::{IconButton, IconButtonSize, apply_icon_btn_styling, rich_text_tooltip},
};

pub struct AppRenderData<'a> {
    //---- for footer panel ----
    pub selected_note: NoteId,
    pub opened_files: SmallVec<[NoteId; 8]>,
    pub external_files: &'a [crate::persistent_state::ExternalFile],
    //---- end of footer panel ----
    pub code_block_annotations: &'a [(SpanIndex, CodeBlockAnnotation)],
    pub text_edit_id: Id,
    pub byte_cursor: Option<UnOrderedByteSpan>,
    pub command_list: &'a CommandList,
    pub syntax_set: &'a SyntaxSet,
    pub theme_set: &'a ThemeSet,
    pub computed_layout: Option<ComputedLayout>,
    pub inline_llm_prompt: Option<&'a mut InlineLLMPromptState>,
    pub slash_palette: Option<&'a SlashPalette>,
    pub word_jump_state: Option<&'a WordJumpState>,
    pub is_window_pinned: bool,
    pub render_actions: SmallVec<[RenderAction; 2]>,
    pub feedback: Option<&'a mut FeedbackState>,
    pub frame_hotkeys: &'a mut FrameHotkeys,
    pub version_state: &'a VersionState,
    pub dev_tools_show: bool,
    pub notifications: &'a mut Notifications<AppNotification>,
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
        external_files,
        byte_cursor,
        command_list,
        computed_layout,
        syntax_set,
        theme_set,
        is_window_pinned,
        inline_llm_prompt,
        slash_palette,
        word_jump_state,
        mut render_actions,
        feedback,
        frame_hotkeys,
        code_block_annotations,
        version_state,
        dev_tools_show,
        notifications,
        opened_files,
    } = visual_state;

    let mut output_actions: SmallVec<[AppAction; 4]> = Default::default();

    let footer_actions = TopBottomPanel::bottom("footer")
        .show_separator_line(false)
        .frame(
            Frame::new()
                .inner_margin(Vec2::new(theme.sizes.s, 0.))
                .fill(theme.colors.main_bg),
        )
        .show(ctx, |ui| {
            ui.set_height(theme.sizes.header_footer_height);

            render_footer_panel(
                ui,
                selected_note,
                opened_files,
                external_files,
                command_list,
                ctx,
                &theme,
            )
        })
        .inner;
    output_actions.extend(footer_actions);

    let header_actions = render_header_panel(
        ctx,
        theme,
        command_list,
        selected_note,
        is_window_pinned,
        feedback.as_ref().map(|f| f.is_sent).unwrap_or(false),
        version_state,
        dev_tools_show,
    );
    output_actions.extend(header_actions);

    restore_cursor_from_note_state(&editor_text, byte_cursor, ctx, text_edit_id);

    if let Some(feedback) = feedback {
        let feedback_widget = Feedback::new(theme, &mut feedback.feedback_data);

        if feedback.is_feedback_open {
            let modal = Modal::new(Id::new("Feedback Modal")).show(ctx, |ui| {
                ui.set_width(300.);
                let feedback_result = feedback_widget.show(ui, frame_hotkeys);

                match feedback_result {
                    Some(FeedbackResult::Cancel) => {
                        output_actions.push(AppAction::CloseFeedbackWindow)
                    }
                    Some(FeedbackResult::SubmitFeedback) => {
                        output_actions.push(AppAction::SubmitFeedback)
                    }
                    None => {}
                }
            });

            if modal.should_close() {
                output_actions.push(AppAction::CloseFeedbackWindow);
            }
        }

        // egui::Window::new(RichText::new("Feedback").text_style(egui::TextStyle::Body))
        //     .open(&mut feedback.is_feedback_open)
        //     .fade_in(true)
        //     .fade_out(true)
        //     .title_bar(false)
        //     .constrain(false)
        //     .collapsible(false)
        //     .resizable(false)
        //     .movable(false)
        //     .anchor(Align2::RIGHT_BOTTOM, [-8.0, -32.0])
        //     .max_size([400., 400.])
        //     .default_size([400., 400.])
        //     .frame(egui::Frame::window(&ctx.style()).fill(window_bg))
        //     .show(ctx, |ui| {
        //         let feedback_result = feedback_widget.show(ui);

        //         match feedback_result {
        //             Some(FeedbackResult::Cancel) => {
        //                 output_actions.push(AppAction::CloseFeedbackWindow)
        //             }
        //             Some(FeedbackResult::SubmitFeedback) => {
        //                 output_actions.push(AppAction::SubmitFeedback)
        //             }
        //             None => {}
        //         }
        //     });
    }

    let (text_has_changed, text_structure, computed_layout, updated_cursor, editor_actions) =
        egui::CentralPanel::default()
            .frame(Frame::central_panel(&ctx.style()).inner_margin(Margin::ZERO))
            .show(ctx, |ui| {
                {
                    let avail_space = ui.available_rect_before_wrap();

                    let hints: Option<SmallVec<[(CowStr<'static>, KeyboardShortcut); 8]>> =
                        editor_text.is_empty().then(|| {
                            PROMOTED_COMMANDS
                                .into_iter()
                                .filter_map(|builtin| command_list.find(builtin))
                                .filter_map(|cmd| {
                                    Some(cmd.instruction.human_description()).zip(cmd.shortcut)
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
                    .id_salt(text_edit_id)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
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
                            word_jump_state,
                            &mut render_actions,
                            theme,
                            syntax_set,
                            theme_set,
                            text_edit_id,
                            selected_note,
                            command_list,
                            frame_hotkeys,
                            code_block_annotations,
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
                                .byte_index_from_char_index(cursor.index);

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
                                                        AppAction::SwitchToNote(
                                                            SwitchToNoteTarget::TargetNote {
                                                                note_file: NoteId::Note(0),
                                                                via_shortcut: true,
                                                            },
                                                        )
                                                    }
                                                    ["shelv", "note2", ..] => {
                                                        AppAction::SwitchToNote(
                                                            SwitchToNoteTarget::TargetNote {
                                                                note_file: NoteId::Note(1),
                                                                via_shortcut: true,
                                                            },
                                                        )
                                                    }
                                                    ["shelv", "note3", ..] => {
                                                        AppAction::SwitchToNote(
                                                            SwitchToNoteTarget::TargetNote {
                                                                note_file: NoteId::Note(2),
                                                                via_shortcut: true,
                                                            },
                                                        )
                                                    }
                                                    ["shelv", "note4", ..] => {
                                                        AppAction::SwitchToNote(
                                                            SwitchToNoteTarget::TargetNote {
                                                                note_file: NoteId::Note(3),
                                                                via_shortcut: true,
                                                            },
                                                        )
                                                    }
                                                    ["shelv", "settings", ..] => {
                                                        AppAction::SwitchToNote(
                                                            SwitchToNoteTarget::TargetNote {
                                                                note_file: NoteId::Settings,
                                                                via_shortcut: true,
                                                            },
                                                        )
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

    let notification_actions = notifications.show(ctx, theme);
    output_actions.extend(notification_actions.into_iter().flatten());

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
    word_jump_state: Option<&WordJumpState>,
    render_actions: &mut SmallVec<[RenderAction; 2]>,
    theme: &AppTheme,
    syntax_set: &SyntaxSet,
    theme_set: &ThemeSet,
    text_edit_id: Id,
    note_file: NoteId,
    command_list: &CommandList,
    frame_hotkeys: &mut FrameHotkeys,
    code_block_annotations: &[(SpanIndex, CodeBlockAnnotation)],
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

    let text_edit_margin = Margin {
        left: (theme.sizes.l) as i8,
        right: (theme.sizes.l) as i8,
        top: (theme.sizes.l) as i8,
        bottom: (theme.sizes.l) as i8,
    };

    let estimated_text_pos = ui.next_widget_position() + text_edit_margin.left_top();

    let code_bg = ui.visuals().code_bg_color;
    let code_bg_rounding = ui.visuals().widgets.inactive.corner_radius;
    if let Some(computed_layout) = &computed_layout {
        for area in computed_layout.code_areas.iter() {
            let background_rect = area
                .rect
                // .shrink(0.5)
                .expand(1.)
                .translate(estimated_text_pos.to_vec2());

            ui.painter()
                .rect_filled(background_rect, code_bg_rounding, code_bg);
        }
    }

    let mut layouter = |ui: &egui::Ui, text: &dyn TextBuffer, wrap_width: f32| {
        let layout_cache_params =
            LayoutParams::new(text.as_str(), wrap_width, ctx.pixels_per_point());

        let layout = match computed_layout.take() {
            Some(layout) if !layout.should_recompute(&layout_cache_params) => layout,

            _ => {
                let structure = structure_wrapper.take().unwrap().recycle(text.as_str());

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

    let TextEditOutput {
        response: text_edit_response,
        galley_pos,
        cursor_range,
        galley,
        ..
    } = egui::TextEdit::multiline(editor_text)
        .font(TextStyle::Monospace) // for cursor height
        .code_editor()
        .id(text_edit_id)
        .lock_focus(true)
        .desired_width(f32::INFINITY)
        .frame(false)
        .margin(text_edit_margin)
        .layouter(&mut layouter)
        .show(ui);

    let prev_actions_count = render_actions.len();
    render_actions.retain(|action| !matches!(action, RenderAction::ScrollToEditorCursorPos));

    // that verifies that we indeed removed some actions, that is, there was at least one scroll action
    if let (Some(cursor_range), true) = (cursor_range, prev_actions_count != render_actions.len()) {
        let font_id = FontSelection::Style(TextStyle::Monospace).resolve(ui.style());
        let row_height = ui.fonts(|f| f.row_height(&font_id));
        let primary_cursor_pos = cursor_rect(&galley, &cursor_range.primary, row_height);

        println!(
            "## ScrollTo cursor, galley_pos={galley_pos:?}, cursor_rect={primary_cursor_pos:?}"
        );

        ui.scroll_to_rect(primary_cursor_pos.translate(galley_pos.to_vec2()), None);
    }

    let text_structure = structure_wrapper.unwrap();

    // ------- WORD JUMP LABELS -------
    if let Some(word_jump_state) = word_jump_state {
        let painter = ui.painter();
        let current_sequence = word_jump_state.current_key_strokes();

        for jump in word_jump_state.jumps() {
            let match_result = jump.label.check_match(current_sequence);
            if matches!(match_result, JumpLabelMatchResult::NoMatch) {
                continue; // Skip labels that cannot match
            }

            // Find word start position using the galley
            let word_start_char_pos = char_index_from_byte_index(editor_text, jump.span.start);

            let cursor_rect = galley.pos_from_cursor(egui::text::CCursor::new(word_start_char_pos));
            let pos = cursor_rect.left_center() + galley_pos.to_vec2();

            // Render the label text
            render_word_jump_label(&painter, match_result, jump.label, pos, theme);
        }

        frame_hotkeys.add_key(FrameHotkey::raw_input(
            Key::Escape,
            AppAction::WordJump(WordJumpAction::CancelJumpingMode),
        ));
    }

    // Test notification keybinding (Cmd+Shift+T)
    // Use current time as ID
    // let test_notification_id = NotificationId::new(ctx.input(|i| i.time as u64));
    // frame_hotkeys.add_key(FrameHotkey::raw_input(
    //     (Modifiers::COMMAND | Modifiers::SHIFT, Key::T),
    //     AppAction::ShowNotification(AppNotification {
    //         id: test_notification_id,
    //         title: Some((Color32::RED, AppIcon::Bug, "Test title".to_string())),
    //         message: "Test notification triggered with Cmd+Shift+T!".to_string(),
    //         action: Some(AppNotificationAction {
    //             button_text: "Test Action".to_string(),
    //             icon: Some(AppIcon::Check),
    //             handler: Box::new(SmallVec::from_iter([
    //                 AppAction::SwitchToNote {
    //                     note_file: NoteFile::Settings,
    //                     via_shortcut: false,
    //                 },
    //                 AppAction::CloseNotification(test_notification_id),
    //             ])),
    //         }),
    //     }),
    // ));

    // ------- FLOATING BUTTONS -------
    if let Some(computed_layout) = &computed_layout {
        for area in computed_layout.code_areas.iter() {
            let code_area = area.rect.translate(estimated_text_pos.to_vec2());

            // that mambo jambo check if the cursor is inside the code area of interest, if yes:
            //   shortcuts become avalable, such as copy block or run the bloc etc
            let is_cursor_inside_area = cursor_range.map_or(false, |range| {
                use egui::TextBuffer;
                let cursor_byte_pos = editor_text.byte_index_from_char_index(range.primary.index);
                match text_structure.get_span_with_meta(area.code_block_span_index) {
                    Some((code_span, _)) => code_span.byte_pos.contains_pos(cursor_byte_pos),
                    None => false,
                }
            });

            let code_block_actions = render_code_actions(
                ui,
                theme,
                code_area,
                code_block_annotations
                    .iter()
                    .find(|(idx, _)| *idx == area.code_block_span_index)
                    .map(|(_, a)| a),
                area.code_block_span_index,
                note_file,
                frame_hotkeys,
                is_cursor_inside_area,
            );
            resulting_actions.extend(code_block_actions);
        }
    }

    let overlay_layer_width = galley.job.wrap.max_width - 2. * estimated_text_pos.x;

    let note_version = NoteSignature {
        note_file,
        text_version: text_structure.opaque_version(),
    };

    // ------- LLM PROMPT -------
    match inline_llm_prompt {
        Some(inline_llm_prompt) if inline_llm_prompt.address.note_version == note_version => {
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
                frame_hotkeys,
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

    // ------- SLASH PALETTE -------
    if let Some(palette) = slash_palette {
        let slash_char_pos = char_index_from_byte_index(editor_text, palette.slash_byte_pos);
        let relative_slash_pos = galley.pos_from_cursor(CCursor::new(slash_char_pos));

        let frame_width = theme.sizes.menu_width;
        let frame_height = theme.sizes.menu_height;

        let frame_right = estimated_text_pos.x
            + (overlay_layer_width).min(relative_slash_pos.left() + frame_width);

        let frame_left = frame_right - frame_width;

        let frame_rect = Rect::from_min_size(
            pos2(
                frame_left,
                estimated_text_pos.y + relative_slash_pos.bottom(),
            ),
            vec2(frame_width, frame_height),
        );

        let (palette_rect, palette_actions) =
            render_slash_palette(palette, frame_rect, theme, frame_hotkeys, ui);

        // TODO hack, only scroll to it on post render
        // Possibly makt it an app action maybe?
        if palette.update_count == 1 {
            // frame_resp.scroll_to_me(None);
            println!("scrolling to palette");
            ui.scroll_to_rect(palette_rect, None);
        }

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
            .map(|c| editor_text.byte_index_from_char_index(c.index));

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
    frame_hotkeys: &mut FrameHotkeys,
) -> (Rect, SmallVec<[AppAction; 1]>) {
    let mut resulting_actions = SmallVec::new();

    let [relative_selection_start, relative_selection_end] = [
        inline_llm_prompt.address.span.start,
        inline_llm_prompt.address.span.end,
    ]
    .map(|pos| {
        let char_pos = char_index_from_byte_index(editor_text, pos);
        galley.pos_from_cursor(CCursor::new(char_pos))
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

    let frame_resp = egui::Frame::new()
        .fill(theme.colors.code_bg_color)
        .inner_margin(theme.sizes.s)
        .stroke(prompt_ui.visuals().window_stroke)
        .shadow(prompt_ui.visuals().window_shadow)
        .corner_radius(prompt_ui.visuals().window_corner_radius)
        .show(&mut prompt_ui, |ui| {
            ui.set_min_width(top_of_frame.width());
            let inline_prompt_address = inline_llm_prompt.address;
            // ui.memory(|m| m.set_focus_lock_filter(id, event_filter);)

            let prompt_text_id = compute_inline_prompt_text_input_id(inline_prompt_address);

            let is_focused = ctx.memory(|m| m.focused()) == Some(prompt_text_id);

            // CommandCondition::loose_match([UiStateAttribute::Focus(
            //                         AppFocus::InlinePropmptEditor,
            //                     )])
            // TODO move that into a command instead
            if is_focused {
                frame_hotkeys.add_key(
                    FrameHotkey::raw_input(Key::Enter, |ctx: CommandContext<'_>| {
                        let action =
                            ctx.app_state
                                .inline_llm_prompt
                                .as_ref()
                                .map(|inline_prompt| {
                                    match &inline_prompt.status {
                                        InlinePromptStatus::NotStarted => {
                                            if inline_prompt.prompt.is_empty() {
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
                                            if prompt == &inline_prompt.prompt {
                                                AppAction::AcceptPromptSuggestion { accept: true }
                                            } else {
                                                AppAction::ExecutePrompt
                                            }
                                        }
                                    }
                                });

                        action
                            .map(|action| SmallVec::from([action]))
                            .unwrap_or_default()
                    }), // enter needs to be overriden if focus is on the text field
                );

                // Esc otherwise will just close the input prompt
                frame_hotkeys.add_key(
                    // Note that this should override the default focus behavior,
                    // hence it is processed before egui has a chance to hanlde esc
                    FrameHotkey::raw_input(
                        Key::Escape,
                        AppAction::AcceptPromptSuggestion { accept: false },
                    ),
                );
            } else {
                // if the inline prompt is open, esc will refocus it back to the input field
                frame_hotkeys.add_key(FrameHotkey::raw_input(
                    Key::Escape,
                    AppAction::FocusRequest(FocusTarget::SpecificId(prompt_text_id)),
                ));
                // CommandCondition::loose_match([UiStateAttribute::Focus(
                //     AppFocus::InlinePropmptEditor,
                // )]),
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
                println!("prompt input gained focus");
                prompt_input_resp.response.scroll_to_me(Some(Align::Center));
            }

            ui.add_space(theme.sizes.s);
            ui.separator();
            ui.add_space(theme.sizes.s);
            let render_btn = |ui: &mut Ui, icon: AppIcon, text| {
                ui.button(icon.render_with_text(
                    (
                        theme.colors.normal_text_color,
                        theme.colors.normal_text_color,
                    ),
                    text,
                    theme.fonts.size.normal,
                ))
            };

            ui.horizontal(|ui| match &inline_llm_prompt.status {
                InlinePromptStatus::NotStarted => {
                    if render_btn(ui, AppIcon::Play, "Run")
                        .on_hover_ui(|ui| {
                            ui.label(rich_text_tooltip(
                                "Prompt AI",
                                Some(KeyboardShortcut::new(Modifiers::NONE, Key::Enter)),
                                theme,
                            ));
                        })
                        .clicked()
                    {
                        resulting_actions.push(AppAction::ExecutePrompt);
                    }
                    ui.add_space(theme.sizes.s);
                    if render_btn(ui, AppIcon::Close, "Cancel")
                        .on_hover_ui(|ui| {
                            ui.label(rich_text_tooltip(
                                "Cancel prompt",
                                Some(KeyboardShortcut::new(Modifiers::NONE, Key::Escape)),
                                theme,
                            ));
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
                        && render_btn(ui, AppIcon::Check, "Accept")
                            .on_hover_ui(|ui| {
                                ui.label(rich_text_tooltip(
                                    "Accept suggestions",
                                    Some(KeyboardShortcut::new(Modifiers::NONE, Key::Enter)),
                                    theme,
                                ));
                            })
                            .clicked()
                    {
                        resulting_actions.push(AppAction::AcceptPromptSuggestion { accept: true });
                    }

                    if prompt != &inline_llm_prompt.prompt
                        && render_btn(ui, AppIcon::Play, "Re-run")
                            .on_hover_ui(|ui| {
                                ui.label(rich_text_tooltip(
                                    "Re-run using the new prompt",
                                    Some(KeyboardShortcut::new(Modifiers::NONE, Key::Enter)),
                                    theme,
                                ));
                            })
                            .clicked()
                    {
                        resulting_actions.push(AppAction::ExecutePrompt);
                    }

                    ui.add_space(theme.sizes.s);
                    if render_btn(ui, AppIcon::Close, "Cancel")
                        .on_hover_ui(|ui| {
                            ui.label(rich_text_tooltip(
                                "Reject changes",
                                Some(KeyboardShortcut::new(Modifiers::NONE, Key::Escape)),
                                theme,
                            ));
                        })
                        .clicked()
                    {
                        resulting_actions.push(AppAction::AcceptPromptSuggestion { accept: false });
                    }
                }
            });

            ui.add_space(theme.sizes.s);
            ui.separator();
            ui.add_space(theme.sizes.s);

            if let Some(reasoning) = inline_llm_prompt.parsed_response.reasoning.as_ref() {
                ui.collapsing(
                    RichText::new("reasoning").color(theme.colors.subtle_text_color),
                    |ui| {
                        ui.label(reasoning);
                    },
                );
                ui.add_space(theme.sizes.s);
            }

            if !inline_llm_prompt.response_text.is_empty() {
                ui.label(WidgetText::LayoutJob(inline_llm_prompt.layout_job.clone()));
            }

            if let Some(explanation) = inline_llm_prompt.parsed_response.explanation.as_ref() {
                ui.add_space(theme.sizes.s);
                ui.collapsing(
                    RichText::new("explanation").color(theme.colors.subtle_text_color),
                    |ui| {
                        ui.label(explanation);
                    },
                );
                // ui.separator();_==_->->=_=_=_=_=_=_
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
    allocated_frame: Rect,
    theme: &AppTheme,
    frame_hotkeys: &mut FrameHotkeys,
    ui: &mut Ui,
) -> (Rect, SmallVec<[AppAction; 1]>) {
    let mut resulting_actions = SmallVec::new();

    let starting_point = pos2(
        allocated_frame.left(),
        allocated_frame.top() + theme.sizes.xs,
    );

    let prompt_ui_rect = Rect::from_min_size(starting_point, allocated_frame.size());

    let mut prompt_ui = ui.new_child(
        UiBuilder::new()
            // .max_rect(Rect::from_pos(point))
            .max_rect(prompt_ui_rect)
            .id_salt("slash_palette_ui")
            .layout(Layout::top_down(Align::LEFT))
            .ui_stack_info(UiStackInfo::new(egui::UiKind::GenericArea)),
    );

    let frame_resp = egui::Frame::none()
        .fill(theme.colors.code_bg_color)
        .inner_margin(theme.sizes.s)
        .stroke(prompt_ui.visuals().window_stroke)
        .shadow(prompt_ui.visuals().window_shadow)
        .corner_radius(prompt_ui.visuals().window_corner_radius)
        // .id_salt("slash_palette_frame")
        .show(&mut prompt_ui, |ui| {
            set_menu_bar_style(ui);

            // ui.set_min_width(top_of_frame.width());

            ScrollArea::vertical()
                .max_height(allocated_frame.height())
                .min_scrolled_height(allocated_frame.height())
                .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                .stick_to_bottom(false)
                .stick_to_right(false)
                .id_salt("slash_palette_scroll")
                .show(ui, |ui| {
                    let mut responses: SmallVec<[Response; 10]> = SmallVec::new();
                    if slash_palette.options.is_empty() {
                        ui.label({
                            RichText::new("No command matches found")
                                .color(theme.colors.subtle_text_color)
                        });
                        // CommandCondition::loose_match([
                        //     UiStateAttribute::Focus(AppFocus::NoteEditor),
                        //     UiStateAttribute::SlashMenu,
                        // ]),
                        frame_hotkeys.add_key(FrameHotkey::raw_input(
                            Key::Escape,
                            AppAction::SlashPalette(SlashPaletteAction::Hide),
                        ));
                    } else {
                        frame_hotkeys.add_key(FrameHotkey::raw_input(
                            Key::ArrowDown,
                            AppAction::SlashPalette(SlashPaletteAction::NextCommand),
                        ));
                        frame_hotkeys.add_key(FrameHotkey::raw_input(
                            Key::ArrowUp,
                            AppAction::SlashPalette(SlashPaletteAction::PrevCommand),
                        ));
                        frame_hotkeys.add_key(FrameHotkey::raw_input(
                            Key::Escape,
                            AppAction::SlashPalette(SlashPaletteAction::Hide),
                        ));

                        frame_hotkeys.add_key(FrameHotkey::raw_input(
                            Key::Enter,
                            AppAction::SlashPalette(SlashPaletteAction::ExecuteCommand(
                                slash_palette.selected,
                            )),
                        ));

                        // ui.set_min_width(prompt_ui_rect.width());
                        // ui.label("here is a long long long long label");
                        for item in Itertools::intersperse(
                            slash_palette.options.iter().enumerate().map(Some),
                            None,
                        ) {
                            match item {
                                Some((i, cmd)) => {
                                    let selected = i == slash_palette.selected;
                                    let resp = render_slash_cmd(ui, theme, cmd, selected)
                                        .interact(Sense::CLICK)
                                        .on_hover_cursor(CursorIcon::PointingHand);

                                    if selected {
                                        let is_visible = ui.is_rect_visible(resp.rect);
                                        if !is_visible {
                                            resp.scroll_to_me(Some(Align::Center));
                                        }
                                    }

                                    if resp.gained_focus() {
                                        // TODO remove, trying to debug what is getting focus
                                        println!("has focus!!")
                                    }

                                    if resp.clicked() {
                                        println!("clicked on {cmd:#?}");

                                        resulting_actions.push(AppAction::FocusRequest(
                                            FocusTarget::CurrentNote,
                                        ));
                                        // TODO fix
                                        // This is quite silly, but it seems to take a few renders before the cursor is properly restored.
                                        resulting_actions.push(AppAction::defer(AppAction::defer(
                                            AppAction::SlashPalette(
                                                SlashPaletteAction::ExecuteCommand(i),
                                            ),
                                        )));
                                    }

                                    ui.input(|input| {
                                        if input.pointer.is_moving()
                                            || input.smooth_scroll_delta != Vec2::ZERO
                                            || input.raw_scroll_delta != Vec2::ZERO
                                        {
                                            if resp.contains_pointer() {
                                                resulting_actions.push(AppAction::SlashPalette(
                                                    SlashPaletteAction::SelectCommand(i),
                                                ));
                                            }
                                        }
                                    });

                                    responses.push(resp);
                                }

                                None => {
                                    ui.separator();
                                }
                            }
                        }
                    }
                });
        })
        .response;

    // if inline_llm_prompt.fresh_response {
    //     inline_llm_prompt.fresh_response = false;
    //     frame_resp.scroll_to_me(None);
    // }

    (frame_resp.rect, resulting_actions)
}

fn render_code_actions(
    ui: &mut Ui,
    theme: &AppTheme,
    code_area: Rect,
    annotation: Option<&CodeBlockAnnotation>,
    span_index: SpanIndex,
    note_file: NoteId,
    frame_hotkeys: &mut FrameHotkeys,
    is_cursor_inside: bool,
) -> EditorCommandOutput {
    let id = ui.id().with("code_annotations").with(span_index);
    let is_hovered = ui.rect_contains_pointer(code_area);
    let alpha = ui.ctx().animate_bool_with_time_and_easing(
        id,
        is_hovered,
        0.2,
        emath::easing::cubic_in_out,
    );

    let monospace = &theme.fonts.family.code;
    let mut resulting_actions: SmallVec<[AppAction; 1]> = SmallVec::new();

    let buttons_anim_id = ui.id().with("buttons_overlay_anim").with(span_index);
    let buttons_visible = ui
        .ctx()
        .animate_bool_with_time(buttons_anim_id, is_hovered, 0.20);

    // TODO: add a setting setup of these
    let run_hotkey = KeyboardShortcut::new(Modifiers::MAC_CMD, Key::Enter);
    // TODO figure out how to bind cmd + c derivitave in egui, it doesn't currently work :(
    // let copy_hotkey = KeyboardShortcut::new(Modifiers::MAC_CMD.plus(Modifiers::SHIFT), Key::Copy);

    // Collect dimensions for logging
    let mut dimensions = Vec::new();
    dimensions.push(format!("Code area: {:?}", code_area));
    dimensions.push(format!(
        "Initial available space: {:?}",
        ui.available_rect_before_wrap()
    ));

    // Create a child UI for buttons on the right side
    let mut buttons_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(code_area)
            .layout(Layout::top_down(Align::TOP))
            .ui_stack_info(UiStackInfo::new(egui::UiKind::GenericArea)),
    );

    // Render buttons on the right side (copy button)
    tui(&mut buttons_ui, id.with("right_buttons"))
        .reserve_available_width()
        .style(
            flex_row()
                .flex_direction(FlexDirection::RowReverse)
                .align_items(AlignItems::Start)
                .align_content(AlignContent::Stretch)
                .width(code_area.width())
                .auto_height()
                .padding(theme.sizes.xs)
                .gap(theme.sizes.xs),
        )
        .show(|tui| {
            // Only show the copy button if the mouse is over the code area
            if buttons_visible > 0.0 {
                if tui
                    .ui_add(
                        IconButton::new(AppIcon::Copy, theme)
                            .size(IconButtonSize::Medium)
                            .tooltip("Copy code", None)
                            .fade(alpha),
                    )
                    .clicked()
                {
                    resulting_actions.push(AppAction::CopyCodeBlock(note_file, span_index));
                }
            }
        });

    // if is_cursor_inside {
    //     frame_hotkeys.add_key_with_modifier(
    //         copy_hotkey.modifiers,
    //         copy_hotkey.logical_key,
    //         move |_| {
    //             println!("COPIED");
    //             SmallVec::from_buf([AppAction::CopyCodeBlock(note_file, span_index)])
    //         },
    //     );
    // }

    if let Some(annotation) = annotation {
        let left_annotation_position = pos2(code_area.left(), code_area.bottom());

        let left_annotation_rect = Rect::from_min_max(
            pos2(ui.max_rect().left(), code_area.top()),
            left_annotation_position,
        );

        let mut left_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(left_annotation_rect)
                .layout(Layout::top_down(Align::RIGHT))
                .ui_stack_info(UiStackInfo::new(egui::UiKind::GenericArea)),
        );

        // DBG: visualize the area allocated for extra stuff
        // left_ui.painter().rect_stroke(
        //     left_ui.available_rect_before_wrap(),
        //     0.,
        //     Stroke::new(1., Color32::LIGHT_YELLOW),
        //     StrokeKind::Inside,
        // );
        dimensions.push(format!(
            "Left area rect: {:?}, avail space {:?}",
            left_annotation_rect,
            left_ui.available_rect_before_wrap()
        ));

        // Render annotations on the left side (applied/error icons)
        tui(&mut left_ui, id.with("left_annotations"))
            .reserve_available_width()
            .style(
                flex_column()
                    .flex_direction(FlexDirection::Column)
                    .align_items(AlignItems::Center)
                    .align_content(AlignContent::Center)
                    .width(left_annotation_rect.width())
                    .auto_height()
                    // .padding(theme.sizes.xs)
                    .gap(theme.sizes.xs),
            )
            .show(|tui| match annotation {
                CodeBlockAnnotation::RunButton => {
                    if is_cursor_inside {
                        // TODO: add a setting setup of that
                        frame_hotkeys.add_key(FrameHotkey::raw_input(
                            (run_hotkey.modifiers, run_hotkey.logical_key),
                            AppAction::RunCodeBlock(note_file, span_index),
                        ));
                    }

                    if tui
                        .ui_add(
                            IconButton::new(AppIcon::Play, theme)
                                .size(IconButtonSize::Medium)
                                .tooltip("Execute", Some(run_hotkey)),
                        )
                        .clicked()
                    {
                        resulting_actions.push(AppAction::RunCodeBlock(note_file, span_index));
                    }
                }

                CodeBlockAnnotation::Applied { message } => {
                    tui.label(
                        AppIcon::Check
                            .render(theme.fonts.size.normal, theme.colors.success_fg_color),
                    )
                    .on_hover_ui(|ui| {
                        ui.style_mut().interaction.selectable_labels = true;
                        ScrollArea::both().show(ui, |ui| {
                            ui.add(
                                Label::new(
                                    RichText::new(message)
                                        .color(theme.colors.subtle_text_color)
                                        .family(monospace.clone()),
                                )
                                .wrap_mode(TextWrapMode::Extend),
                            )
                        });
                    });
                }

                CodeBlockAnnotation::Error { title, message } => {
                    tui.label(
                        AppIcon::Error.render(theme.fonts.size.normal, theme.colors.error_fg_color),
                    )
                    .on_hover_ui(|ui| {
                        ui.style_mut().interaction.selectable_labels = true;
                        ScrollArea::both().show(ui, |ui| {
                            ui.label(
                                RichText::new(title)
                                    .strong()
                                    .color(theme.colors.error_fg_color),
                            );
                            ui.add(
                                Label::new(
                                    RichText::new(message)
                                        .color(theme.colors.subtle_text_color)
                                        .family(monospace.clone()),
                                )
                                .wrap_mode(TextWrapMode::Extend),
                            )
                        });
                    });
                }
            });
    }

    // Print all dimensions at once
    // println!("UI DIMENSIONS:\n{}", dimensions.join("\n"));

    resulting_actions
}

fn render_slash_cmd(
    ui: &mut Ui,
    theme: &AppTheme,

    cmd: &SlashPaletteCmd,
    selected: bool,
) -> egui::Response {
    let phosphor_icon_font = TextFormat::simple(
        FontId::new(theme.fonts.size.h4, FontFamily::Name("phosphor".into())),
        theme.colors.normal_text_color,
    );

    let header_font = TextFormat::simple(
        FontId::new(theme.fonts.size.normal, theme.fonts.family.normal.clone()),
        theme.colors.md_code,
    );

    let shortcut_font = TextFormat::simple(
        FontId::new(theme.fonts.size.small, theme.fonts.family.italic.clone()),
        theme.colors.subtle_text_color,
    );

    let description_font = TextFormat::simple(
        FontId::new(theme.fonts.size.small, theme.fonts.family.normal.clone()),
        theme.colors.subtle_text_color,
    );

    let available_width = ui.available_width();

    tui(ui, ui.id().with(cmd))
        .reserve_available_width()
        .show(|tui| {
            tui.style(
                flex_column()
                    .align_content(AlignContent::Stretch)
                    .width(available_width)
                    .auto_height()
                    .padding(theme.sizes.xs)
                    .gap(theme.sizes.xs),
            )
            .selectable(selected, |tui| {
                tui.style(flex_row().justify_content(JustifyContent::SpaceBetween))
                    .add(|tui| {
                        tui.style(flex_row().gap(theme.sizes.s)).add(|tui| {
                            tui.ui_add(
                                Label::new(
                                    RichText::new(if let Some(icon) = &cmd.phosphor_icon {
                                        icon.symbol()
                                    } else {
                                        egui_phosphor::light::GEAR
                                    })
                                    .font(phosphor_icon_font.font_id)
                                    .color(phosphor_icon_font.color),
                                )
                                .wrap_mode(TextWrapMode::Extend),
                            );
                            tui.ui_add(
                                Label::new(
                                    RichText::new(&cmd.prefix)
                                        .font(header_font.font_id)
                                        .color(header_font.color),
                                )
                                .wrap_mode(TextWrapMode::Extend),
                            );
                        });

                        tui.ui_add(
                            Label::new(
                                RichText::new(format!(
                                    "{}",
                                    cmd.instance
                                        .shortcut
                                        .map(format_mac_shortcut_with_symbols)
                                        .unwrap_or_default()
                                ))
                                .font(shortcut_font.font_id)
                                .color(shortcut_font.color),
                            )
                            .wrap_mode(egui::TextWrapMode::Extend),
                        );
                    });

                tui.label(
                    RichText::new(&cmd.description)
                        .font(description_font.font_id)
                        .color(description_font.color),
                );
            })
        })
        .response
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

        if ccursor_range != text_edit_state.cursor.char_range() {
            text_edit_state.cursor.set_char_range(ccursor_range);
            text_edit_state.store(ctx, text_state_id);
        }
    }
}

pub fn draw_debug_rect(ui: &Ui, color: Color32) {
    ui.painter().debug_rect(
        Rect::from_center_size(
            ui.available_rect_before_wrap().center(),
            ui.available_size(),
        ),
        color,
        format!("r={:?}", ui.available_rect_before_wrap()),
    );
}

fn set_menu_bar_style(ui: &mut egui::Ui) {
    let style = ui.style_mut();
    // TODO 2 seems better (more square, but we need to take the value from theme or soemthing)
    // style.spacing.button_padding = vec2(2., 0.0);
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
    selected_note: NoteId,
    is_window_pinned: bool,
    feedback_sent: bool,
    version_state: &VersionState,
    dev_tools_show: bool,
) -> SmallVec<[AppAction; 1]> {
    TopBottomPanel::top("top_panel")
        .show_separator_line(false)
        // .exact_height(theme.sizes.header_footer)
        .frame(Frame::new().fill(theme.colors.main_bg))
        .show(ctx, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            let mut resulting_actions: SmallVec<[AppAction; 1]> = Default::default();
            let sizes = &theme.sizes;
            ui.set_height(sizes.header_footer_height);
            let avail_width = ui.available_width();
            let avail_rect = ui.available_rect_before_wrap();
            ui.painter().line_segment(
                [avail_rect.left() + theme.sizes.s, avail_rect.right() - theme.sizes.s]
                    .map(|x| pos2(x, avail_rect.top() + sizes.header_footer_height)),
                Stroke::new(1.0, theme.colors.outline_fg),
            );
            // ui.set_min_size(vec2(avail_width, sizes.header_footer));

            let header_ui_id = ui.id().with("header");

            let tooltip_animation_id = header_ui_id.with("feedback_sent_tooltip");
            let tooltip_value =
                ctx.animate_bool_with_time(tooltip_animation_id, feedback_sent, 2.0);

            let tui_result = tui(ui, header_ui_id)
                .style(
                    flex_row()
                        .width(avail_width)
                        .height(sizes.header_footer_height)
                        .align_items(AlignItems::Center)
                        .justify_content(JustifyContent::SpaceBetween)
                        .padding_horizontal(sizes.s),
                )
                .show(|t| {
                    // Left section: Close button and title
                    t.style(flex_row().align_items(AlignItems::Center).gap(sizes.m))
                        .add(|t| {
                            // Close button
                            if t.ui_add(
                                IconButton::new(AppIcon::Close, theme)
                                    .size(IconButtonSize::Large)
                                    .tooltip("Hide Shelv", None),
                            )
                            .clicked()
                            {
                                resulting_actions.push(AppAction::HideApp);
                            }

                            // Title
                            t.ui_add(
                                Label::new(
                                    RichText::new(format!(
                                        "Shelv - {}",
                                        match selected_note {
                                            NoteId::Note(index)=>format!("note {}",index+1),
                                            NoteId::Settings=>"settings".to_string(),
                                            NoteId::ExternalFileId(external_file_id) => format!("TODO: external {:?}",external_file_id),
                                        }
                                    ))
                                    .color(theme.colors.subtle_text_color)
                                    .font(FontId {
                                        size: theme.fonts.size.normal,
                                        family: theme.fonts.family.bold.clone(),
                                    }),
                                )
                                .extend(),
                            );
                        });

                    // Right section: Feedback button, pin button, separator, and menu

                    t.style(flex_row().align_items(AlignItems::Center).gap(sizes.s))
                        .add(|t| {
                            // Update button
                            let update_btn = match version_state {
                                VersionState::UpToDate => None,
                                VersionState::UpdateAvailable(Version(version)) => Some(
                                    IconButton::new(AppIcon::Download, theme)
                                        .color(theme.colors.hyperlink_color)
                                        .size(IconButtonSize::Large)
                                        .text("Update Available")
                                        .tooltip(format!("v{} available, click to open App Store to update.", version), None),
                                ),
                                VersionState::RequiredUpdateAvailable(Version(version)) => Some(
                                    IconButton::new(AppIcon::Download, theme)
                                        .color(theme.colors.warn_fg_color)
                                        .size(IconButtonSize::Large)
                                        .text("Required Update")
                                        .tooltip(format!("Version ≥{} required, click to open App Store to update.", version), None),
                                ),
                            };

                            if let Some(update_btn) = update_btn {
                                if t.ui_add(update_btn).clicked() {
                                    resulting_actions.push(AppAction::AppUpdateClicked);
                                };
                            }

                            // TEST: Open external file button (temporary)
                            #[cfg(debug_assertions)]
                            if t.ui_add(
                                IconButton::new(AppIcon::Folder, theme)
                                    .size(IconButtonSize::Large)
                                    .tooltip("TEST: Open test.md", None),
                            )
                            .clicked()
                            {
                                use std::path::PathBuf;
                                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                                let readme_path = PathBuf::from(home).join("work/shelv/test.md");
                                resulting_actions.push(AppAction::OpenExternalFile(readme_path));
                            }

                            // Feedback button
                            if t.ui_add(
                                IconButton::new(AppIcon::Feedback, theme)
                                    .size(IconButtonSize::Large)
                                    .tooltip(
                                        "Send this note to report a bug or share feedback.",
                                        None,
                                    ),
                            )
                            .clicked()
                            {
                                resulting_actions.push(AppAction::OpenFeedbackWindow);
                            }

                            // Dev tools button (debug builds only)
                            #[cfg(debug_assertions)]
                            if t.ui_add(
                                IconButton::new(AppIcon::Bug, theme)
                                    .size(IconButtonSize::Large)
                                    .toggled(dev_tools_show)
                                    .tooltip(
                                        if dev_tools_show {
                                            "Hide dev tools"
                                        } else {
                                            "Show dev tools"
                                        },
                                        None,
                                    ),
                            )
                            .clicked()
                            {
                                resulting_actions.push(AppAction::ToggleDevTools);
                            }

                            // Pin button with tooltip and keyboard shortcut
                            if t.ui_add(
                                IconButton::new(AppIcon::Pin, theme)
                                    .size(IconButtonSize::Large)
                                    .toggled(is_window_pinned)
                                    .tooltip(
                                        if is_window_pinned {
                                            "Unpin window"
                                        } else {
                                            "Pin window"
                                        },
                                        command_list
                                            .find(CommandInstruction::PinWindow)
                                            .and_then(|cmd| cmd.shortcut),
                                    ),
                            )
                            .clicked()
                            {
                                resulting_actions
                                    .push(AppAction::SetWindowPinned(!is_window_pinned));
                            }

                            // Separator
                            t.label(
                                AppIcon::VerticalSeparator
                                    .render(sizes.toolbar_icon, theme.colors.outline_fg),
                            );

                            // Menu button - use ui_add_manual to embed the original menu_button
                            t.ui_finite(
                                |ui| {
                                    apply_icon_btn_styling(ui.style_mut());
                                    ui.menu_button(
                                        AppIcon::Menu.render(
                                            sizes.toolbar_icon,
                                            theme.colors.subtle_text_color,
                                        ),
                                        |ui| {
                                            ui.set_max_width(200.0);

                                            if ui
                                                .button(AppIcon::Tutorial.render_with_text(
                                                    (theme.colors.normal_text_color, theme.colors.normal_text_color),
                                                    "Start tutorial",
                                                    theme.fonts.size.normal,
                                                ))
                                                .clicked()
                                            {
                                                ui.close_kind(UiKind::Menu);
                                                resulting_actions.push(AppAction::StartTutorial);
                                            }

                                            ui.separator();

                                            for (icon, text, link) in [
                                                (
                                                    &AppIcon::HomeSite,
                                                    "Visit https://shelv.app",
                                                    "https://shelv.app",
                                                ),
                                                (
                                                    &AppIcon::Discord,
                                                    "Join our Discord",
                                                    include_str!("../distribution/discord_invite.txt")
                                                        .trim(),
                                                ),
                                                (
                                                    &AppIcon::Github,
                                                    "Give as a Star or file an issue",
                                                    "https://github.com/twop/shelv",
                                                ),
                                            ] {
                                                if ui
                                                    .button(icon.render_with_text(
                                                        (theme.colors.normal_text_color, theme.colors.normal_text_color),
                                                        text,
                                                        theme.fonts.size.normal,
                                                    ))
                                                    .clicked()
                                                {
                                                    ui.close_menu();
                                                    resulting_actions.push(AppAction::OpenLink(
                                                        link.to_string(),
                                                    ));
                                                }
                                            }

                                            ui.separator();

                                            if ui
                                                .button(AppIcon::Folder.render_with_text(
                                                    (theme.colors.normal_text_color, theme.colors.normal_text_color),
                                                    "Open notes folder",
                                                    theme.fonts.size.normal,
                                                ))
                                                .clicked()
                                            {
                                                ui.close_menu();
                                                resulting_actions
                                                    .push(AppAction::OpenNotesInFinder);
                                            }
                                        },
                                    )
                                    .response
                                },
                                // |mut val, _ui| {
                                //     // Menu button can grow minimally
                                //     val.max_size = val.min_size;
                                //     val.infinite = egui::Vec2b::FALSE;
                                //     val
                                // },
                            );
                        });
                });

            // Handle feedback sent animation - show tooltip if feedback was recently sent
            if feedback_sent && tooltip_value < 1. {
                // We need to show the tooltip on the feedback button, but since we're outside the taffy context,
                // we'll just let the animation run for now. The tooltip will be handled by the button's hover state
                // in future iterations if needed.
            }

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

/// Renders a word jump label with appropriate styling based on match result
fn render_word_jump_label(
    painter: &Painter,
    match_result: JumpLabelMatchResult,
    label: JumpLabel,
    left_center: egui::Pos2,
    theme: &AppTheme,
) {
    use JumpLabelMatchResult::*;

    let label_text = label.to_string();

    let label_font = FontId::new(theme.fonts.size.small, FontFamily::Monospace);

    let label_galley = painter.layout_no_wrap(
        label_text.clone(),
        label_font.clone(),
        theme.colors.normal_text_color,
    );

    let label_rect = Rect::from_min_size(
        left_center + vec2(-1.0, -label_galley.size().y / 2.0),
        label_galley.size() + vec2(2.0, 0.0),
    );

    painter.rect_filled(label_rect, 4.0, theme.colors.md_code);

    // painter.rect_stroke(
    //     label_rect,
    //     4.0,
    //     Stroke::new(0.5, theme.colors.subtle_text_color),
    //     StrokeKind::Outside,
    // );

    let (completed_chars, pending_chars) = match match_result {
        NoMatch => {
            // this should not happen, but for completeness
            return;
        }
        Possible(matches) => (
            JumpCharSequence::from_iter(label_text.chars().take(matches)),
            JumpCharSequence::from_iter(label_text.chars().skip(matches)),
        ),
        FullMatch => (
            JumpCharSequence::new(),
            JumpCharSequence::from_iter(label_text.chars()),
        ),
    };

    let mut current_x = left_center.x;

    for (i, (ch, char_color)) in completed_chars
        .into_iter()
        .map(|ch| (ch, theme.colors.subtle_text_color))
        .chain(
            pending_chars
                .into_iter()
                .map(|ch| (ch, theme.colors.code_bg_color)),
        )
        .enumerate()
    {
        let label_rect = painter.text(
            egui::pos2(current_x, left_center.y),
            Align2::LEFT_CENTER,
            ch.to_string(),
            label_font.clone(),
            char_color,
        );

        current_x += label_rect.size().x;
    }
}

impl NotificationItem for AppNotification {
    type Output = EditorCommandOutput;

    fn render(&self, ui: &mut egui::Ui, theme: &AppTheme) -> Option<Self::Output> {
        let tui_id = ui.id().with("notification_content");
        let mut result_action = None;

        tui(ui, tui_id)
            .style(
                flex_column()
                    .auto_height()
                    // .max_width(theme.sizes.menu_width)
                    .min_width(theme.sizes.menu_width)
                    // .width(100.)
                    // .grow(1.0)
                    .gap(theme.sizes.s),
            )
            .show(|t| {
                t.style(
                    flex_row()
                        .justify_content(JustifyContent::SpaceBetween)
                        .align_items(AlignItems::Center)
                        .auto_width()
                        .min_width(theme.sizes.menu_width)
                        .auto_height(),
                )
                .add(|t| {
                    let title_text = match &self.title {
                        Some((color, icon, title)) => icon.render_with_text(
                            (*color, theme.colors.normal_text_color),
                            title,
                            theme.fonts.size.normal,
                        ),
                        None => RichText::new(&self.message)
                            .color(theme.colors.normal_text_color)
                            .size(theme.fonts.size.normal)
                            .into(),
                    };
                    t.ui_add(egui::Label::new(title_text).wrap_mode(TextWrapMode::Extend));

                    if t.ui_add(IconButton::new(AppIcon::Close, theme).tooltip("Dismiss", None))
                        .clicked()
                    {
                        result_action =
                            Some(EditorCommandOutput::from([AppAction::CloseNotification(
                                self.id,
                            )]));
                    }
                });

                if self.title.is_some() {
                    t.style(
                        flex_column()
                            .auto_width()
                            .min_width(theme.sizes.menu_width)
                            .auto_height(),
                    )
                    .add(|t| {
                        t.ui_add(egui::Label::new(&self.message));
                    });
                }

                if let Some(action) = &self.action {
                    t.style(
                        flex_row()
                            .auto_width()
                            .auto_height()
                            .min_width(theme.sizes.menu_width)
                            .justify_content(JustifyContent::End),
                    )
                    .add(|t| {
                        let button_icon = match &action.icon {
                            Some(icon) => icon.clone(),
                            None => AppIcon::Check,
                        };

                        let button = IconButton::new(button_icon, theme)
                            .text(&action.button_text)
                            .color(theme.colors.hyperlink_color);

                        if t.ui_add(button).clicked() {
                            if !action.handler.is_empty() {
                                result_action = Some(*action.handler.clone());
                            }
                        }
                    });
                }
            });

        result_action
    }
}
