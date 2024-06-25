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
use scripting::execute_live_scripts;
use smallvec::SmallVec;
use text_structure::TextStructure;
use theme::{configure_styles, get_font_definitions};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
// use tray_item::TrayItem;G1

use std::{
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

mod nord;
mod persistent_state;
mod picker;
mod scripting;
mod text_structure;
mod theme;

pub struct MyApp {
    state: AppState,
    hotwatch: Hotwatch,
    hotkeys_manager: GlobalHotKeyManager,
    tray: TrayIcon,
    msg_queue: SyncSender<MsgToApp>,
    persistence_folder: PathBuf,
}

struct RealAppIO;

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
}

impl MyApp {
    pub fn new(cc: &CreationContext) -> Self {
        let theme = Default::default();
        configure_styles(&cc.egui_ctx, &theme);

        let fonts = get_font_definitions();

        cc.egui_ctx.set_fonts(fonts);

        let (msg_queue_tx, msg_queue_rx) = sync_channel::<MsgToApp>(10);
        let hotkeys_manager = GlobalHotKeyManager::new().unwrap();
        let global_open_hotkey = HotKey::new(Some(Modifiers::SHIFT | Modifiers::META), Code::KeyM);

        hotkeys_manager.register(global_open_hotkey).unwrap();

        let open_hotkey = global_open_hotkey.clone();
        let ctx = cc.egui_ctx.clone();
        let sender = msg_queue_tx.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |ev: GlobalHotKeyEvent| {
            if ev.id == open_hotkey.id() && ev.state() == HotKeyState::Pressed {
                sender.send(MsgToApp::ToggleVisibility).unwrap();
                ctx.request_repaint();
                println!("handler for ToggleVisibility: {open_hotkey:?}, ev = {ev:#?}");
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

        let tray_icon = TrayIconBuilder::new()
            .with_tooltip("Show/Hide Shelv")
            .with_icon(Icon::from_rgba(tray_image.into_bytes(), 64, 64).unwrap())
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

        Self {
            state: AppState::new(AppInitData {
                theme,
                msg_queue: msg_queue_rx,
                persistent_state,
                last_saved,
            }),
            hotkeys_manager,
            tray: tray_icon,
            msg_queue: msg_queue_tx,
            persistence_folder,
            hotwatch,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let text_edit_id = Id::new("text_edit");

        let app_state = &mut self.state;
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
            process_app_action(action, ctx, app_state, text_edit_id, &RealAppIO);
        }

        // note that we have a settings note amoung them
        let note_count = app_state.notes.len() - 1;

        let note = &app_state.notes.get(&app_state.selected_note).unwrap();
        let mut cursor = note.cursor;

        let editor_text = &note.text;
        let mut text_structure = app_state
            .text_structure
            .take()
            .unwrap_or_else(|| TextStructure::new(editor_text));

        let orignal_text_version = text_structure.opaque_version();

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

        // handling scheduled JS execution
        if let (Some(text_cursor_range), Some(scheduled_version)) =
            (cursor, app_state.scheduled_script_run_version.take())
        {
            // TODO refactor this block as an AppAction
            // println!("executing live scripts for version = {scheduled_version}",);
            let script_changes = execute_live_scripts(&text_structure, &editor_text);
            if let Some(changes) = script_changes {
                // println!("detected changes from: js\n{changes:?}\n\n");
                if let Ok(updated_cursor) =
                    apply_text_changes(editor_text, text_cursor_range, changes)
                {
                    text_structure = text_structure.recycle(&editor_text);
                    cursor = Some(updated_cursor);
                }
            }
        }

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

        let RenderAppResult(actions, updated_structure, byte_cursor, updated_layout) = render_app(
            text_structure,
            editor_text,
            vis_state,
            &app_state.theme,
            ctx,
        );

        app_state.text_structure = Some(updated_structure);
        app_state.computed_layout = updated_layout;
        app_state
            .notes
            .get_mut(&app_state.selected_note)
            .unwrap()
            .cursor = byte_cursor;

        for action in actions {
            process_app_action(action, ctx, app_state, text_edit_id, &RealAppIO);
        }

        let updated_structure_version = app_state
            .text_structure
            .as_ref()
            .map(|s| s.opaque_version());

        if updated_structure_version != Some(orignal_text_version) {
            app_state
                .unsaved_changes
                .push(UnsavedChange::NoteContentChanged(app_state.selected_note));

            app_state.scheduled_script_run_version = updated_structure_version;
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
