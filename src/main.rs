use app_actions::{
    AppAction, AppIO, HideMode, SlashPaletteAction, compute_app_focus, process_app_action,
};
use app_io::RealAppIO;
use app_state::{AppInitData, AppState, MsgToApp, compute_editor_text_id};
use app_ui::{AppRenderData, RenderAppResult, is_shortcut_match, render_app};
use command::{AppFocusState, CommandContext, EditorCommandOutput};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use hotwatch::{
    Event, EventKind, Hotwatch,
    notify::event::{DataChange, ModifyKind},
};
use image::ImageFormat;
use itertools::Itertools;
use persistent_state::{NoteFile, load_and_migrate, try_save, v1};
use scripting::settings_eval::Scripts;
use smallvec::SmallVec;
use text_structure::TextStructure;
use theme::{configure_styles, get_font_definitions};
use tokio::runtime::Runtime;

use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};
// use tray_item::TrayItem;G1

use std::{path::PathBuf, sync::mpsc::sync_channel};

use eframe::{
    CreationContext,
    egui::{self},
    epaint::vec2,
    get_value,
};

use crate::{app_state::UnsavedChange, persistent_state::extract_note_file};

mod app_actions;
mod app_io;
mod app_state;
mod app_ui;
mod byte_span;
mod command;
mod commands;
mod effects;
mod egui_hotkey;
mod feedback;
mod knus_test;
mod nord;
mod persistent_state;
mod picker;
mod scripting;
mod settings_parsing;
mod taffy_styles;
mod text_structure;
mod theme;

pub struct MyApp<IO: AppIO> {
    state: AppState,
    hotwatch: Hotwatch,
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

        let mut app_io = RealAppIO::new(
            GlobalHotKeyManager::new().unwrap(),
            cc.egui_ctx.clone(),
            msg_queue_tx.clone(),
            persistence_folder.clone(),
        );

        // let open_hotkey = global_open_hotkey.clone();
        let ctx = cc.egui_ctx.clone();
        let sender = msg_queue_tx.clone();

        GlobalHotKeyEvent::set_event_handler(Some(move |ev: GlobalHotKeyEvent| {
            if ev.state() == HotKeyState::Pressed {
                sender.send(MsgToApp::GlobalHotkey(ev.id())).unwrap();
                ctx.request_repaint();
            }
        }));

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
            include_bytes!("../assets/tray-icon-macos-template.png",),
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
        let persistent_state = load_and_migrate(number_of_notes, v1_save, &persistence_folder);

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

        let last_saved = persistent_state.state.last_saved;

        let state = AppState::new(AppInitData {
            theme,
            msg_queue: msg_queue_rx,
            persistent_state,
            last_saved,
        });
        Self {
            state,
            app_io,
            tray: tray_icon,
            app_focus_state: AppFocusState {
                is_menu_opened: false,
                internal_focus: None,
                viewport_focused: false,
            },
            persistence_folder,
            hotwatch,
        }
    }
}

impl<IO: AppIO> eframe::App for MyApp<IO> {
    fn raw_input_hook(&mut self, ctx: &egui::Context, _raw_input: &mut egui::RawInput) {
        self.app_focus_state = compute_app_focus(ctx, &self.state);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ctx.set_visuals(egui::Visuals::dark());

        let app_state = &mut self.state;

        let selected_note_file = app_state.selected_note;

        let text_edit_id = compute_editor_text_id(selected_note_file);

        // handling message queue
        let mut action_list = EditorCommandOutput::from_iter(
            app_state
                .msg_queue
                .try_iter()
                .map(AppAction::HandleMsgToApp),
        );

        let app_focus = self.app_focus_state.clone();
        let focused_id = ctx.memory(|m| m.focused());

        let mut scripts = app_state
            .settings_scripts
            .take()
            .unwrap_or_else(|| Scripts::new());

        // handling commands
        // sych as {tab, enter} inside a list
        let actions_from_keyboard_commands = ctx
            .input_mut(|input| {
                // if !input.keys_down.is_empty() || input.modifiers.any() {
                //     println!("### keys={:?}, mods={:?}", input.keys_down, input.modifiers);
                // }

                // only one command can be handled at a time

                app_state.commands.available_keyboard_commands().find_map(
                    |(keyboard_shortcut, keyboard_binding)| {
                        if is_shortcut_match(input, &keyboard_shortcut) {

                            let ctx = CommandContext {
                                app_state,
                                app_focus,
                                ui_state: app_state.to_ui_state(),
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
                                        editor_command.scope,
                                        ctx,
                                    )
                                }
                                command::KeyboardBinding::FrameBinding(frame_hotkey) => {
                                    (frame_hotkey.run)(ctx)
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

            let mut action_buffer: SmallVec<[AppAction; 4]> = SmallVec::from_iter([action]);

            while let Some(to_process) = action_buffer.pop() {
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

                action_buffer.extend(new_actions);
            }
        }

        // note that we have a settings note amoung them
        let note_count = app_state.notes.len() - 1;

        let note = &app_state.notes.get(&app_state.selected_note).unwrap();
        let cursor = note.cursor().or(note.last_cursor());

        let editor_text = &note.text;

        let text_structure = app_state
            .text_structure
            .take()
            .unwrap_or_else(|| TextStructure::new(editor_text));

        // if the app is pinned it is OK not re-requesting focus
        // neither hiding if focus lost
        if !app_state.is_pinned {
            let is_frame_actually_focused = ctx.input(|i| i.viewport().focused.unwrap_or(false));

            // handling focus lost
            if app_state.prev_focused != is_frame_actually_focused && !is_frame_actually_focused {
                println!("lost focus");
                app_state.hidden = true;
                self.app_io.hide_app(HideMode::HideApp);
            }

            app_state.prev_focused = is_frame_actually_focused;
        }

        let editor_text = &mut app_state
            .notes
            .get_mut(&app_state.selected_note)
            .unwrap()
            .text;
        let mut frame_hotkeys = app_state.commands.prepare_frame_hotkeys();

        let vis_state = AppRenderData {
            selected_note: app_state.selected_note,
            is_window_pinned: app_state.is_pinned,
            note_count,
            text_edit_id,
            command_list: &app_state.commands,
            byte_cursor: cursor,
            syntax_set: &app_state.syntax_set,
            theme_set: &app_state.theme_set,
            computed_layout: app_state.computed_layout.take(),
            inline_llm_prompt: (&mut app_state.inline_llm_prompt).as_mut(),
            slash_palette: app_state.slash_palette.as_ref(),
            render_actions: (app_state.render_actions.drain(..)).collect(),
            frame_hotkeys: &mut frame_hotkeys,
            feedback: (&mut app_state.feedback).as_mut(),
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
        app_state.text_structure = Some(updated_structure);
        app_state.computed_layout = updated_layout;
        let note = app_state.notes.get_mut(&app_state.selected_note).unwrap();
        match byte_cursor {
            Some(cursor) => {
                if note.cursor().is_none() {
                    println!("[MAIN] Restored cursor from rendered data");
                }
                note.update_cursor(cursor)
            }
            None => {
                if note.cursor().is_some() {
                    println!("[MAIN] Reseting cursor from rendered data");
                }
                note.reset_cursor()
            }
        }

        // post render processing
        for action in actions {
            let mut action_buffer: SmallVec<[AppAction; 4]> = SmallVec::from_iter([action]);

            while let Some(to_proccess) = action_buffer.pop() {
                let new_actions = process_app_action(
                    to_proccess,
                    ctx,
                    app_state,
                    app_focus,
                    text_edit_id,
                    &mut self.app_io,
                );
                action_buffer.extend(new_actions);
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
            .with_min_inner_size(vec2(350.0, 450.0))
            .with_inner_size(vec2(350.0, 450.0)),

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
