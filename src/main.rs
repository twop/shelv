use app_actions::{
    AppAction, AppIO, HideMode, SlashPaletteAction, compute_app_focus, process_app_action,
};
use app_io::RealAppIO;
use app_state::{AppInitData, AppState, MsgToApp, compute_editor_text_id};
use app_ui::{AppRenderData, RenderAppResult, render_app};
use command::{AppFocusState, CommandContext, EditorCommandOutput};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use hotwatch::{
    Event, EventKind, Hotwatch,
    notify::event::{DataChange, ModifyKind},
};
use image::ImageFormat;
use persistent_state::{UpdateStatus, load_and_migrate, try_save, v1};
use scripting::settings_eval::Scripts;
use smallvec::SmallVec;
use theme::{configure_styles, get_font_definitions};
use tokio::runtime::Runtime;
use update_messages::get_update_notification;

use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};
// use tray_item::TrayItem;G1

use std::{path::PathBuf, sync::mpsc::sync_channel};

use eframe::{
    CreationContext,
    egui::{self, Key, KeyboardShortcut},
    epaint::vec2,
    get_value,
};

use crate::{
    actions::word_jump::process_word_jump_input,
    app_actions::WordJumpAction,
    app_state::{NoteSignature, UnsavedChange},
    command::AppFocus,
    persistent_state::extract_note_file,
};
use shared::Version;

mod actions;
mod app_actions;
mod app_io;
mod app_state;
mod app_ui;
mod byte_span;
mod command;
mod commands;
mod dev_tools;
mod effects;
mod egui_hotkey;
mod feedback;
// mod knus_test;
mod nord;
mod persistent_state;
mod scripting;
mod settings_parsing;
mod taffy_styles;
mod text_structure;
mod theme;
mod ui;
mod ui_components;
mod update_messages;

pub struct MyApp<IO: AppIO> {
    state: AppState,
    tray: TrayIcon,
    persistence_folder: PathBuf,
    app_io: IO,

    // begining of the frame
    app_focus_state: AppFocusState,
}

impl MyApp<RealAppIO> {
    pub fn new(cc: &CreationContext) -> Self {
        let theme = Default::default();
        configure_styles(&cc.egui_ctx, &theme);

        let fonts = get_font_definitions();

        cc.egui_ctx.set_fonts(fonts);

        let persistence_folder = directories_next::ProjectDirs::from("app", "", "Shelv")
            .map(|proj_dirs| proj_dirs.data_dir().to_path_buf())
            .unwrap();

        let (msg_queue_tx, msg_queue_rx) = sync_channel::<MsgToApp>(10);

        let (shelv_api_server, shelv_magic_token, debug_chat_prompts): (
            &'static str,
            &'static str,
            bool,
        ) = const_dotenvy::dotenvy!(
            SHELV_API_SERVER: &'static str,
            SHELV_MAGIC_TOKEN: &'static str,
            SHELV_DEBUG_CHAT_PROMPTS: bool = false
        );

        let current_version = env!("CARGO_PKG_VERSION");

        let hotkeys_manager =
            GlobalHotKeyManager::new().expect("Failed to initialize global hotkey manager");

        {
            let sender = msg_queue_tx.clone();
            let ctx = cc.egui_ctx.clone();

            GlobalHotKeyEvent::set_event_handler(Some(move |ev: GlobalHotKeyEvent| {
                if ev.state() == HotKeyState::Pressed {
                    sender.send(MsgToApp::GlobalHotkey(ev.id())).unwrap();
                    ctx.request_repaint();
                }
            }));
        }

        // Create and setup hotwatch for persistence folder
        let sender = msg_queue_tx.clone();
        let ctx = cc.egui_ctx.clone();
        let mut hotwatch = Hotwatch::new().expect("hotwatch failed to initialize!");
        hotwatch
            .watch(&persistence_folder, move |event: Event| {
                // println!("\nhotwatch event\n{:#?}\n", event);
                if let EventKind::Modify(ModifyKind::Data(DataChange::Content)) = event.kind {
                    let filter_map: SmallVec<[_; 4]> = event
                        .paths
                        .iter()
                        .filter_map(|p| {
                            p.file_name()
                                .and_then(|f| f.to_str())
                                .and_then(extract_note_file)
                                .map(|(note_file, _)| (note_file, p))
                        })
                        .collect();

                    let has_updates = !filter_map.is_empty();
                    for (note_file, path) in filter_map {
                        sender
                            .send(MsgToApp::NoteFileChanged(note_file, path.clone()))
                            .unwrap();
                    }
                    if has_updates {
                        ctx.request_repaint();
                    }
                }
            })
            .expect("failed to watch file!");

        let app_io = RealAppIO::new(
            hotkeys_manager,
            hotwatch,
            cc.egui_ctx.clone(),
            msg_queue_tx.clone(),
            persistence_folder.clone(),
            shelv_api_server.to_string(),
            shelv_magic_token.to_string(),
            debug_chat_prompts,
            Version(current_version.to_string()),
        );

        let ctx = cc.egui_ctx.clone();
        let sender = msg_queue_tx.clone();
        TrayIconEvent::set_event_handler(Some(move |ev| {
            match &ev {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Down,
                    ..
                } => {
                    sender.send(MsgToApp::ToggleVisibility).unwrap();
                    ctx.request_repaint();
                }
                _ => {}
            }

            println!("tray event: {:?}", ev);
        }));
        let tray_image = image::load_from_memory_with_format(
            include_bytes!("../assets/shelv-tray-icon-macos-template.png",),
            ImageFormat::Png,
        )
        .unwrap();

        let tray_quit_menu_button = MenuItem::new("Quit", true, None);
        let tray_quit_menu_button_id = tray_quit_menu_button.id().clone();
        let tray_menu = Menu::with_items(&[&tray_quit_menu_button]).unwrap();

        MenuEvent::set_event_handler(Some(move |ev: MenuEvent| {
            println!("tray menu event: {:?}", ev);
            if ev.id == tray_quit_menu_button_id {
                std::process::exit(0);
            }
        }));

        let tray_icon = TrayIconBuilder::new()
            .with_tooltip("Show/Hide Shelv")
            .with_icon(Icon::from_rgba(tray_image.into_bytes(), 64, 64).unwrap())
            .with_menu(Box::new(tray_menu))
            .with_menu_on_left_click(false)
            // TODO macOS
            .with_icon_as_template(true)
            .build()
            .unwrap();

        let v1_save: Option<v1::PersistentState> =
            cc.storage.and_then(|s| get_value(s, "persistent_state"));

        let number_of_notes = 4;
        let (persistent_state, load_kind, update_status) =
            load_and_migrate(number_of_notes, v1_save, &persistence_folder);

        let last_saved = persistent_state.state.last_saved;

        let mut state = AppState::new(AppInitData {
            theme,
            msg_queue: msg_queue_rx,
            persistent_state,
            last_saved,
            load_kind,
        });

        if let UpdateStatus::Updated(new_version) = &update_status {
            // Add update notification if we updated to a new version, AND we have a message at the ready for it
            if let Some(notification) = get_update_notification(new_version, &state.theme) {
                state
                    .deferred_actions
                    .push(AppAction::ShowNotification(notification));
            }
        }

        app_io.start_update_checker();

        Self {
            state,
            app_io,
            tray: tray_icon,
            app_focus_state: AppFocusState {
                is_menu_opened: false,
                internal_focus: None,
                viewport_focused: false,
                focus_id: None,
            },
            persistence_folder,
        }
    }
}

impl<IO: AppIO> eframe::App for MyApp<IO> {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.app_focus_state = compute_app_focus(ctx, &self.state);

        let app_focus = self.app_focus_state.clone();
        let app_state = &mut self.state;

        let mut scripts = app_state
            .settings_scripts
            .take()
            .unwrap_or_else(|| Scripts::new());

        let actions_from_raw_input_commands = app_state
            .commands
            .available_keyboard_commands_for_phase(command::CommandPhase::RawInputHook)
            .find_map(|(keyboard_shortcut, keyboard_binding)| {
                if is_shortcut_match(raw_input.events.iter(), &keyboard_shortcut) {
                    let ctx = CommandContext {
                        app_state,
                        ui_state: app_state.to_ui_state(app_focus),
                        scripts: &mut scripts,
                    };

                    let res = match keyboard_binding {
                        command::KeyboardBinding::CommandInstance(editor_command) => {
                            println!(
                                "---Found RawInputHook match for {:?}, focus = {app_focus:#?}",
                                editor_command.instruction.human_description()
                            );
                            app_state.commands.run(
                                &editor_command.instruction,
                                &editor_command.cond,
                                ctx,
                            )
                        }
                        command::KeyboardBinding::FrameBinding(frame_hotkey) => {
                            frame_hotkey.run(ctx)
                        }
                    };

                    if !res.is_empty() {
                        // Remove the matching key events from raw input
                        raw_input.events.retain(|event| {
                            if let egui::Event::Key {
                                key,
                                pressed: true,
                                modifiers,
                                ..
                            } = event
                            {
                                !(*key == keyboard_shortcut.logical_key
                                    && *modifiers == keyboard_shortcut.modifiers)
                            } else {
                                true
                            }
                        });
                        Some(res)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap_or_default();

        app_state.settings_scripts = Some(scripts);

        // Process any actions that were generated
        if !actions_from_raw_input_commands.is_empty() {
            app_state
                .deferred_actions
                .extend(actions_from_raw_input_commands.into_iter());
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ctx.set_visuals(egui::Visuals::dark());

        let app_state = &mut self.state;

        let selected_note_file = app_state.selected_note;

        let text_edit_id = compute_editor_text_id(selected_note_file);

        let app_focus = self.app_focus_state.clone(); // Render dev tools if enabled

        ctx.input(|input| {
            app_state.dev_tools.dump_input_events(input);
        });

        if app_state.dev_tools.show_dev_tools {
            use eframe::egui::{ViewportBuilder, ViewportId};

            let viewport_id = ViewportId::from_hash_of("dev_tools");

            ctx.show_viewport_immediate(
                viewport_id,
                ViewportBuilder::default()
                    .with_title("Shelv Debug Tools")
                    .with_inner_size([1000.0, 800.0])
                    .with_resizable(true),
                |ctx, _class| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let ui_state = app_state.to_ui_state(app_focus);
                        let dev_actions = app_state.dev_tools.show(
                            ui,
                            Some(app_focus),
                            &ui_state,
                            &app_state.theme,
                            &app_state.notifications,
                        );

                        // Add dev tool actions to deferred actions
                        app_state.deferred_actions.extend(dev_actions);
                    });

                    if ctx.input(|i| i.viewport().close_requested()) {
                        app_state.dev_tools.show_dev_tools = false;
                    }
                },
            );
        }

        // handling message queue
        let mut action_list = EditorCommandOutput::from_iter(
            app_state
                .msg_queue
                .try_iter()
                .map(AppAction::HandleMsgToApp),
        );

        let mut scripts = app_state
            .settings_scripts
            .take()
            .unwrap_or_else(|| Scripts::new());

        // Note that it will consume the input event, so essentially WordJump acts as a modal
        if let Some(word_jump_state) = &app_state.word_jump_state {
            let note = app_state.notes.get(&app_state.selected_note).unwrap();
            let current_note_signature = NoteSignature::new(
                app_state.selected_note,
                (&note.derived_state.structure).opaque_version(),
            );

            if word_jump_state.signature() == current_note_signature
                && app_focus.internal_focus == Some(AppFocus::NoteEditor)
            {
                ctx.input_mut(|input| {
                    if let Some(word_jump_action) = process_word_jump_input(input) {
                        action_list.push(AppAction::WordJump(word_jump_action));
                    }
                });
            } else {
                action_list.push(AppAction::WordJump(WordJumpAction::CancelJumpingMode));
            }
        }

        let focused_id = app_focus.focus_id;

        let mut frame_hotkeys = app_state.commands.prepare_frame_hotkeys();

        // handling commands
        // sych as {tab, enter} inside a list
        let actions_from_keyboard_commands = ctx
            .input_mut(|input| {
                // if !input.keys_down.is_empty() || input.modifiers.any() {
                //     println!("### keys={:?}, mods={:?}", input.keys_down, input.modifiers);
                // }

                // only one command can be handled at a time

                app_state.commands.available_keyboard_commands_for_phase(command::CommandPhase::InsideRender).find_map(
                    |(keyboard_shortcut, keyboard_binding)| {
                        if is_shortcut_match(input.events.iter(), &keyboard_shortcut) {

                            let ctx = CommandContext {
                                app_state,
                                ui_state: app_state.to_ui_state(app_focus),
                                scripts: &mut scripts,
                            };
                            let res = match keyboard_binding {
                                command::KeyboardBinding::CommandInstance(editor_command) => {
                                    println!(
                                        "---Found a match for {:?}, focus = {app_focus:#?}, focused_id = {focused_id:?}",
                                        editor_command.instruction.human_description()
                                    );
                                    app_state.commands.run(
                                        &editor_command.instruction,
                                        &editor_command.cond,
                                        ctx,
                                    )
                                }
                                command::KeyboardBinding::FrameBinding(frame_hotkey) => {
                                    frame_hotkey.run(ctx)
                                }
                            };

                            if !res.is_empty() {
                                // println!(
                                //     "---command {:?} consumed input {:?}\nres_actions={res:#?}",
                                //     editor_command.instruction.human_description(),
                                //     keyboard_shortcut
                                // );

                                // remove the keys from the input

                                input.consume_shortcut(&keyboard_shortcut);
                                Some(res)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    },
                )
            })
            .unwrap_or_default();

        app_state.settings_scripts = Some(scripts);

        action_list.extend(actions_from_keyboard_commands.into_iter());

        action_list.insert_many(0, app_state.deferred_actions.drain(0..));

        // now apply prepared changes, and update text structure and cursor appropriately
        for action in action_list {
            match action {
                AppAction::SlashPalette(SlashPaletteAction::Update) => {
                    // Comment out to stop the spammy logging
                    //  println!("---processing action = {action:#?}")
                }
                _ => println!("---processing action = {action:#?}"),
            }

            let mut action_buffer: SmallVec<[(AppAction, usize); 4]> =
                SmallVec::from_iter([(action, 0)]);

            while let Some((to_process, depth)) = action_buffer.pop() {
                // Log the action before processing
                app_state.dev_tools.log_action(
                    to_process.clone(),
                    depth,
                    crate::dev_tools::ActionPhase::PreRender,
                );

                let new_actions = process_app_action(
                    to_process,
                    ctx,
                    app_state,
                    app_focus,
                    text_edit_id,
                    &mut self.app_io,
                );

                if new_actions.len() > 0 {
                    match new_actions.first() {
                        Some(AppAction::SlashPalette(SlashPaletteAction::Update)) => {
                            // Comment out to stop the spammy logging
                            println!(
                                "---enqueued actions = AppAction::SlashPalette(SlashPaletteAction::Update)"
                            );
                        }
                        Some(AppAction::DeferToPostRender(new_actions)) => {
                            match new_actions.as_ref() {
                                AppAction::SlashPalette(SlashPaletteAction::Update) => {
                                    // do nothing
                                }
                                _ => {
                                    println!("---enqueued actions = {new_actions:#?}");
                                }
                            };
                            // Comment out to stop the spammy logging
                            //println!("---enqueued actions = {new_actions:#?}");
                        }
                        _ => {
                            println!("---enqueued actions = {new_actions:#?}");
                        }
                    }
                }

                action_buffer.extend(new_actions.into_iter().map(|a| (a, depth + 1)));
            }
        }

        let note = app_state.notes.get_mut(&app_state.selected_note).unwrap();
        let text_structure = std::mem::take(&mut note.derived_state.structure);
        let cursor = note.cursor().or(note.last_cursor());

        // if the app is pinned it is OK not re-requesting focus
        // neither hiding if focus lost
        if !app_state.is_pinned {
            let is_frame_actually_focused = ctx.input(|i| i.viewport().focused.unwrap_or(false));

            // handling focus lost
            if app_state.prev_focused != is_frame_actually_focused
                && !is_frame_actually_focused
                && !app_state.dev_tools.show_dev_tools
            {
                println!("lost focus");
                app_state.hidden = true;
                self.app_io.hide_app(HideMode::HideApp);
            }

            app_state.prev_focused = is_frame_actually_focused;
        }

        let opened_files = SmallVec::from_iter(app_state.notes.keys().cloned());
        let edited_note = app_state.notes.get_mut(&app_state.selected_note).unwrap();

        let editor_text = &mut edited_note.text;
        let code_block_annotations = &mut edited_note.derived_state.code_block_annotations;

        let vis_state = AppRenderData {
            selected_note: app_state.selected_note,
            opened_files,
            is_window_pinned: app_state.is_pinned,
            external_files: &app_state.external_files,
            text_edit_id,
            command_list: &app_state.commands,
            byte_cursor: cursor,
            syntax_set: &app_state.syntax_set,
            theme_set: &app_state.theme_set,
            computed_layout: app_state.computed_layout.take(),
            inline_llm_prompt: (&mut app_state.inline_llm_prompt).as_mut(),
            slash_palette: app_state.slash_palette.as_ref(),
            word_jump_state: app_state.word_jump_state.as_ref(),
            render_actions: (app_state.render_actions.drain(..)).collect(),
            frame_hotkeys: &mut frame_hotkeys,
            feedback: (&mut app_state.feedback).as_mut(),
            version_state: &app_state.app_version_state,
            code_block_annotations,
            dev_tools_show: app_state.dev_tools.show_dev_tools,
            notifications: &mut app_state.notifications,
        };

        let RenderAppResult {
            requested_actions: actions,
            updated_text_structure: updated_structure,
            latest_cursor: byte_cursor,
            latest_layout: updated_layout,
            text_changed,
        } = render_app(
            text_structure,
            editor_text,
            vis_state,
            &app_state.theme,
            ctx,
        );

        app_state.commands.add_frame_hotkeys(frame_hotkeys);

        if text_changed {
            println!("----note changed during render");
            app_state
                .add_unsaved_change(UnsavedChange::NoteContentChanged(app_state.selected_note));
        }

        // TODO it seems that this can be done inside process_app_action
        app_state.computed_layout = updated_layout;
        let note = app_state.notes.get_mut(&app_state.selected_note).unwrap();
        note.derived_state.structure = updated_structure;
        match byte_cursor {
            Some(cursor) => {
                if note.cursor().is_none() {
                    println!("[main.rs] Restored cursor from rendered data cursor={cursor:?}");
                }
                note.update_cursor(cursor)
            }
            None => {
                if note.cursor().is_some() {
                    println!("[main.rs] Reseting cursor from rendered data");
                }
                note.reset_cursor()
            }
        }

        // post render processing
        for action in actions {
            let mut action_buffer: SmallVec<[(AppAction, usize); 4]> =
                SmallVec::from_iter([(action, 0)]);

            while let Some((to_proccess, depth)) = action_buffer.pop() {
                // Log the action before processing
                app_state.dev_tools.log_action(
                    to_proccess.clone(),
                    depth,
                    crate::dev_tools::ActionPhase::PostRender,
                );

                let new_actions = process_app_action(
                    to_proccess,
                    ctx,
                    app_state,
                    app_focus,
                    text_edit_id,
                    &mut self.app_io,
                );
                action_buffer.extend(new_actions.into_iter().map(|a| (a, depth + 1)));
            }
        }
    }

    fn on_exit(&mut self) {

        // If you need to abort an exit check `ctx.input(|i| i.viewport().close_requested())`
        // and respond with [`egui::ViewportCommand::CancelClose`].
        //
    }

    // fn on_close_event(&mut self) -> bool {
    //     self.msg_queue.send(MsgToApp::ToggleVisibility).unwrap();
    //     false
    // }

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(1)
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        if let Some(persistent_state) = self.state.should_persist() {
            // set_value(storage, "persistent_state", &persistent_state);
            //
            println!("\npersisted state: {persistent_state:#?}\n");

            match try_save(persistent_state, &self.persistence_folder) {
                Ok(save_state) => {
                    self.state.last_saved = save_state.last_saved;
                }
                Err(err) => {
                    println!("failed to persist state with err={err:#?}")
                }
            };
        }
    }
}

/// Count presses of a key. If non-zero, the presses are consumed, so that this will only return non-zero once.
///
/// Includes key-repeat events.
pub fn is_shortcut_match<'a>(
    input: impl IntoIterator<Item = &'a egui::Event>,
    shortcut: &KeyboardShortcut,
) -> bool {
    let KeyboardShortcut {
        modifiers,
        logical_key,
    } = shortcut.clone();

    input.into_iter().any(|event| {
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

fn main() {
    let _guard = sentry::init((
        "https://10f977d35f32b70d88180f4875543208@o4507879687454720.ingest.us.sentry.io/4507879689945088",
        sentry::ClientOptions {
            release: sentry::release_name!(),
            ..Default::default()
        },
    ));

    let rt = Runtime::new().expect("Unable to create Runtime");
    // Enter the runtime so that `tokio::spawn` is available immediately.
    let _enter = rt.enter();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_resizable(true)
            .with_always_on_top()
            .with_min_inner_size(vec2(500.0, 600.0))
            .with_inner_size(vec2(500.0, 600.0)),

        // max_window_size: Some(vec2(650.0, 750.0)),
        // fullsize_content: true,
        // decorated: false,
        run_and_return: true,
        window_builder: Some(Box::new(|builder| {
            #[cfg(target_os = "macos")]
            {
                // use winit::platform::macos::WindowAttributesExtMacOS;
                return builder
                    .with_fullsize_content_view(true)
                    .with_titlebar_buttons_shown(false)
                    .with_title_shown(false)
                    .with_titlebar_shown(false);
                //.with_tr(true);
            }

            builder
        })),
        event_loop_builder: Some(Box::new(|builder| {
            #[cfg(target_os = "macos")]
            {
                use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
                // EventLoopBuilderExtMacOS::with_activation_policy(
                //     builder,
                //     ActivationPolicy::Accessory,
                // );
                builder.with_activation_policy(ActivationPolicy::Accessory);
            }
        })),

        ..Default::default()
    };

    eframe::run_native(
        "Shelv",
        options,
        Box::new(|cc| Ok(Box::new(MyApp::new(cc)))),
    )
    .unwrap();
}
