#![feature(iter_intersperse)]
#![feature(let_chains)]
#![feature(offset_of)]
#![feature(generic_const_exprs)]

use app_actions::{process_app_action, AppAction, AppIO};
use app_state::{AppInitData, AppState, MsgToApp};
use app_ui::{is_shortcut_match, render_app, AppRenderData, RenderAppResult};
use byte_span::UnOrderedByteSpan;
use command::{CommandContext, EditorCommandOutput, TextCommandContext};
use effects::text_change_effect::{apply_text_changes, TextChange};
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};

use hotwatch::{
    notify::event::{DataChange, ModifyKind},
    Event, EventKind, Hotwatch,
};
use image::ImageFormat;
use persistent_state::{load_and_migrate, try_save, v1, NoteFile};
use smallvec::SmallVec;
use text_structure::TextStructure;
use theme::{configure_styles, get_font_definitions};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
};
// use tray_item::TrayItem;G1

use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, Read},
    path::PathBuf,
    sync::mpsc::{sync_channel, SyncSender},
};

use eframe::{
    egui::{self, Id, ViewportCommand},
    epaint::vec2,
    get_value, set_value, CreationContext,
};

use crate::{
    app_state::UnsavedChange,
    persistent_state::{extract_note_file, get_utc_timestamp},
};

mod app_actions;
mod app_state;
mod app_ui;
mod byte_span;
mod command;
mod commands;
mod effects;
mod egui_hotkey;
mod settings;

mod nord;
mod persistent_state;
mod picker;
mod scripting;
mod text_structure;
mod theme;

pub struct MyApp {
    state: AppState,
    hotwatch: Hotwatch,
    tray: TrayIcon,
    msg_queue: SyncSender<MsgToApp>,
    persistence_folder: PathBuf,
    app_io: RealAppIO,
}

struct RegisteredGlobalHotkey {
    egui_shortcut: egui::KeyboardShortcut,
    system_hotkey: global_hotkey::hotkey::HotKey,
    handler: Box<dyn Fn() -> MsgToApp>,
}

struct RealAppIO {
    hotkeys_manager: GlobalHotKeyManager,
    registered_hotkeys: BTreeMap<u32, RegisteredGlobalHotkey>,
}

impl RealAppIO {
    fn new(hotkeys_manager: GlobalHotKeyManager) -> Self {
        Self {
            hotkeys_manager,
            registered_hotkeys: Default::default(),
        }
    }
}

impl AppIO for RealAppIO {
    fn hide_app(&self) {
        hide_app_on_macos();
    }

    fn try_read_note_if_newer(
        &self,
        path: &PathBuf,
        last_saved: u128,
    ) -> Result<Option<String>, io::Error> {
        try_read_note_if_newer(path, last_saved)
    }

    fn try_map_hotkey(&self, hotkey_id: u32) -> Option<MsgToApp> {
        self.registered_hotkeys
            .get(&hotkey_id)
            .map(|key| (key.handler)())
    }

    fn bind_global_hotkey(
        &mut self,
        shortcut: egui::KeyboardShortcut,
        handler: Box<dyn Fn() -> MsgToApp>,
    ) -> Result<(), String> {
        let system_hotkey = convert_egui_shortcut_to_global_hotkey(shortcut);

        self.hotkeys_manager
            .register(system_hotkey)
            .map_err(|err| err.to_string())?;

        self.registered_hotkeys.insert(
            system_hotkey.id(),
            RegisteredGlobalHotkey {
                egui_shortcut: shortcut,
                system_hotkey,
                handler,
            },
        );

        Ok(())
    }

    fn cleanup_all_global_hotkeys(&mut self) -> Result<(), String> {
        for hotkey in self.registered_hotkeys.values() {
            self.hotkeys_manager
                .unregister(hotkey.system_hotkey)
                .map_err(|err| err.to_string())?;
        }
        self.registered_hotkeys.clear();
        Ok(())
    }
}

impl MyApp {
    pub fn new(cc: &CreationContext) -> Self {
        let theme = Default::default();
        configure_styles(&cc.egui_ctx, &theme);

        let fonts = get_font_definitions();

        cc.egui_ctx.set_fonts(fonts);

        let (msg_queue_tx, msg_queue_rx) = sync_channel::<MsgToApp>(10);

        let mut app_io = RealAppIO::new(GlobalHotKeyManager::new().unwrap());

        // hotkeys_manager.register(global_open_hotkey).unwrap();

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
            include_bytes!("../assets/tray-icon.png",),
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
            .build()
            .unwrap();

        let v1_save: Option<v1::PersistentState> =
            cc.storage.and_then(|s| get_value(s, "persistent_state"));

        let persistence_folder = directories_next::ProjectDirs::from("app", "", "Shelv")
            .map(|proj_dirs| proj_dirs.data_dir().to_path_buf())
            .unwrap();

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

        let state = AppState::new(
            AppInitData {
                theme,
                msg_queue: msg_queue_rx,
                persistent_state,
                last_saved,
            },
            &mut app_io,
        );
        Self {
            state,
            app_io,
            tray: tray_icon,
            msg_queue: msg_queue_tx,
            persistence_folder,
            hotwatch,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let app_state = &mut self.state;
        let text_edit_id = Id::new(match &app_state.selected_note {
            NoteFile::Note(index) => format!("text_edit_id_{}", index),
            NoteFile::Settings => "text_edit_id_settings".to_string(),
        });

        // handling message queue
        let mut action_list = EditorCommandOutput::from_iter(
            app_state
                .msg_queue
                .try_iter()
                .map(AppAction::HandleMsgToApp),
        );

        // handling commands
        // sych as {tab, enter} inside a list
        let actions_from_keyboard_commands = ctx
            .input_mut(|input| {
                // if !input.keys_down.is_empty() || input.modifiers.any() {
                //     println!("### keys={:?}, mods={:?}", input.keys_down, input.modifiers);
                // }

                // only one command can be handled at a time
                app_state
                    .editor_commands
                    .slice()
                    .iter()
                    .find_map(|editor_command| {
                        match &editor_command.shortcut {
                            Some(keyboard_shortcut)
                                if is_shortcut_match(input, &keyboard_shortcut) =>
                            {
                                println!("---Found a match for {}", editor_command.name);
                                let res = (editor_command.try_handle)(CommandContext { app_state });

                                if !res.is_empty() {
                                    // remove the keys from the input
                                    input.consume_shortcut(&keyboard_shortcut);
                                }

                                Some(res)
                            }
                            _ => None,
                        }
                    })
            })
            .unwrap_or_default();

        action_list.extend(actions_from_keyboard_commands.into_iter());

        // now apply prepared changes, and update text structure and cursor appropriately
        for action in action_list {
            let mut next_action = Some(action);

            while let Some(to_proccess) = next_action.take() {
                next_action =
                    process_app_action(to_proccess, ctx, app_state, text_edit_id, &mut self.app_io);
            }
        }

        // note that we have a settings note amoung them
        let note_count = app_state.notes.len() - 1;

        let note = &app_state.notes.get(&app_state.selected_note).unwrap();
        let cursor = note.cursor;

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
                hide_app_on_macos();
            }

            app_state.prev_focused = is_frame_actually_focused;
        }

        let editor_text = &mut app_state
            .notes
            .get_mut(&app_state.selected_note)
            .unwrap()
            .text;

        let vis_state = AppRenderData {
            selected_note: app_state.selected_note,
            is_window_pinned: app_state.is_pinned,
            note_count,
            text_edit_id,
            command_list: &app_state.editor_commands,
            byte_cursor: cursor,
            syntax_set: &app_state.syntax_set,
            theme_set: &app_state.theme_set,
            computed_layout: app_state.computed_layout.take(),
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

        if text_changed {
            app_state
                .add_unsaved_change(UnsavedChange::NoteContentChanged(app_state.selected_note));
        }

        // TODO it seems that this can be done inside process_app_action
        app_state.text_structure = Some(updated_structure);
        app_state.computed_layout = updated_layout;
        app_state
            .notes
            .get_mut(&app_state.selected_note)
            .unwrap()
            .cursor = byte_cursor;

        // post render processing
        for action in actions {
            let mut next_action = Some(action);

            while let Some(to_proccess) = next_action.take() {
                next_action =
                    process_app_action(to_proccess, ctx, app_state, text_edit_id, &mut self.app_io);
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

fn try_read_note_if_newer(path: &PathBuf, last_saved: u128) -> Result<Option<String>, io::Error> {
    let mut file = File::open(path)?;
    let meta = file.metadata()?;
    let modified_at = meta.modified()?;

    if get_utc_timestamp(modified_at) > last_saved + 10 {
        // println!(
        //     "updating note {note_file:?}, \nlast_saved={}\nmodified_at={}",
        //     self.last_saved,
        //     get_utc_timestamp(modified_at)
        // );
        let mut content = String::new();
        file.read_to_string(&mut content)?;

        Ok(Some(content))
    } else {
        Ok(None)
    }
}

fn main() {
    let options = eframe::NativeOptions {
        default_theme: eframe::Theme::Dark,
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

    eframe::run_native("Shelv", options, Box::new(|cc| Box::new(MyApp::new(cc)))).unwrap();
}

fn hide_app_on_macos() {
    // https://developer.apple.com/documentation/appkit/nsapplication/1428733-hide
    use objc2::rc::Id;
    use objc2::{class, msg_send, msg_send_id, runtime::Object};
    unsafe {
        let app: Id<Object> = msg_send_id![class!(NSApplication), sharedApplication];
        let arg = app.as_ref();
        let _: () = msg_send![&app, hide:arg];
    }
}

fn convert_egui_shortcut_to_global_hotkey(
    shortcut: egui::KeyboardShortcut,
) -> global_hotkey::hotkey::HotKey {
    let mut modifiers = Modifiers::empty();
    if shortcut.modifiers.alt {
        modifiers |= Modifiers::ALT;
    }
    if shortcut.modifiers.ctrl {
        modifiers |= Modifiers::CONTROL;
    }
    if shortcut.modifiers.shift {
        modifiers |= Modifiers::SHIFT;
    }
    if shortcut.modifiers.mac_cmd {
        modifiers |= Modifiers::META;
    }

    use egui::Key::*;
    use global_hotkey::hotkey::Code::*;

    let code = match shortcut.logical_key {
        A => KeyA,
        B => KeyB,
        C => KeyC,
        D => KeyD,
        E => KeyE,
        F => KeyF,
        G => KeyG,
        H => KeyH,
        I => KeyI,
        J => KeyJ,
        K => KeyK,
        L => KeyL,
        M => KeyM,
        N => KeyN,
        O => KeyO,
        P => KeyP,
        Q => KeyQ,
        R => KeyR,
        S => KeyS,
        T => KeyT,
        U => KeyU,
        V => KeyV,
        W => KeyW,
        X => KeyX,
        Y => KeyY,
        Z => KeyZ,
        // TODO: Add more mappings as needed
        // TODO2: is there a way not to do it manually?
        _ => KeyA, // Default to KeyA for unmapped keys
    };

    HotKey::new(Some(modifiers), code)
}
