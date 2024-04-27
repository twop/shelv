#![feature(iter_intersperse)]
#![feature(let_chains)]

use app_actions::{process_app_action, TextChange};
use app_state::{AppInitData, AppState, MsgToApp};
use app_ui::{is_shortcut_match, render_app, AppRenderData, RenderAppResult};
use byte_span::UnOrderedByteSpan;
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
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
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};
// use tray_item::TrayItem;G1

use std::{
    fs::{self, File},
    io::{self, Read},
    path::PathBuf,
    sync::mpsc::{sync_channel, SyncSender},
};

use eframe::{
    egui::{self, Id},
    epaint::vec2,
    get_value, set_value, CreationContext,
};

use crate::{
    app_actions::apply_text_changes,
    app_state::UnsavedChange,
    persistent_state::{extract_note_file, get_utc_timestamp},
};

mod app_actions;
mod app_state;
mod app_ui;
mod byte_span;
mod commands;
mod egui_hotkey;
mod md_shortcut;
mod nord;
mod persistent_state;
mod picker;
mod scripting;
mod text_structure;
mod theme;

pub struct MyApp {
    state: AppState,
    last_saved: u128,
    hotwatch: Hotwatch,
    hotkeys_manager: GlobalHotKeyManager,
    tray: TrayIcon,
    msg_queue: SyncSender<MsgToApp>,
    persistence_folder: PathBuf,
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
            if ev.id == open_hotkey.id() {
                sender.send(MsgToApp::ToggleVisibility).unwrap();
                ctx.request_repaint();
                println!("handler: {:?}", open_hotkey);
            }
        }));

        let ctx = cc.egui_ctx.clone();
        let sender = msg_queue_tx.clone();
        TrayIconEvent::set_event_handler(Some(move |ev| {
            sender.send(MsgToApp::ToggleVisibility).unwrap();
            ctx.request_repaint();

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

        Self {
            last_saved: persistent_state.state.last_saved,
            state: AppState::new(AppInitData {
                theme,
                msg_queue: msg_queue_rx,
                persistent_state,
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
        while let Ok(msg) = app_state.msg_queue.try_recv() {
            match msg {
                MsgToApp::ToggleVisibility => {
                    app_state.hidden = !app_state.hidden;

                    if app_state.hidden {
                        hide_app_on_macos();
                    } else {
                        frame.set_visible(!app_state.hidden);
                        frame.focus();
                        ctx.memory_mut(|mem| mem.request_focus(text_edit_id));
                    }
                }
                MsgToApp::NoteFileChanged(note_file, path) => {
                    if let NoteFile::Note(index) = &note_file {
                        // println!("change detected, {note_file:?} at {path:?}");
                        let last_saved = self.last_saved;

                        match try_read_note_if_newer(path, last_saved) {
                            Ok(Some(note_content)) => {
                                app_state.notes[*index as usize].text = note_content;
                                app_state.unsaved_changes.push(UnsavedChange::LastUpdated);
                            }
                            Ok(None) => {
                                // no updates needed we already have the newest version
                            }
                            Err(err) => {
                                // failed to read note file
                                println!("failed to read {path:#?}, err={err:#?}");
                            }
                        }
                    }
                }
            }
        }

        let note = &mut app_state.notes[app_state.selected_note as usize];
        let mut cursor: Option<UnOrderedByteSpan> = note.cursor;

        let editor_text = &mut note.text;
        let mut text_structure = app_state
            .text_structure
            .take()
            .unwrap_or_else(|| TextStructure::new(editor_text));

        let orignal_text_version = text_structure.opaque_version();

        // handling focus lost
        let is_frame_actually_focused = frame.info().window_info.focused;
        if app_state.prev_focused != is_frame_actually_focused {
            if is_frame_actually_focused {
                println!("gained focus");
                ctx.memory_mut(|mem| mem.request_focus(text_edit_id))
            } else {
                println!("lost focus");
                app_state.hidden = true;
                hide_app_on_macos();
            }
            app_state.prev_focused = is_frame_actually_focused;
        }

        // restore focus, it seems that there is a lag
        if !app_state.hidden && !is_frame_actually_focused {
            frame.focus()
        }

        // handling commands
        // sych as {tab, enter} inside a list
        let changes: Option<(Vec<TextChange>, UnOrderedByteSpan)> =
            cursor.clone().and_then(|byte_range| {
                ctx.input_mut(|input| {
                    // only one command can be handled at a time
                    app_state.editor_commands.iter().find_map(|editor_command| {
                        let keyboard_shortcut = editor_command.shortcut();
                        if is_shortcut_match(input, &keyboard_shortcut) {
                            let res = editor_command.try_handle(
                                &text_structure,
                                &editor_text,
                                byte_range.ordered(),
                            );
                            if res.is_some() {
                                // remove the keys from the input
                                input.consume_shortcut(&keyboard_shortcut);
                            }
                            res.map(|changes| (changes, byte_range))
                        } else {
                            None
                        }
                    })
                })
            });

        // now apply prepared changes, and update text structure and cursor appropriately
        if let Some((changes, byte_range)) = changes {
            if let Ok(updated_cursor) = apply_text_changes(editor_text, byte_range, changes) {
                text_structure = text_structure.recycle(&editor_text);
                cursor = Some(updated_cursor);
            }
        };

        // handling scheduled JS execution
        if let (Some(text_cursor_range), Some(scheduled_version)) =
            (cursor, app_state.scheduled_script_run_version.take())
        {
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
            text_edit_id,
            font_scale: app_state.font_scale,
            byte_cursor: cursor,
            md_shortcuts: &app_state.md_annotation_shortcuts,
            syntax_set: &app_state.syntax_set,
            theme_set: &app_state.theme_set,
            computed_layout: app_state.computed_layout.take(),
        };

        let RenderAppResult(actions, updated_structure, byte_cursor, updated_layout) = render_app(
            text_structure,
            editor_text,
            vis_state,
            &app_state.app_shortcuts,
            &app_state.theme,
            ctx,
        );

        app_state.text_structure = Some(updated_structure);
        app_state.computed_layout = updated_layout;
        app_state.notes[app_state.selected_note as usize].cursor = byte_cursor;

        for action in actions {
            process_app_action(action, ctx, app_state, text_edit_id);
        }

        let updated_structure_version = app_state
            .text_structure
            .as_ref()
            .map(|s| s.opaque_version());

        if updated_structure_version != Some(orignal_text_version) {
            app_state
                .unsaved_changes
                .push(UnsavedChange::NoteContentChanged(NoteFile::Note(
                    app_state.selected_note,
                )));

            app_state.scheduled_script_run_version = updated_structure_version;
        }
    }

    fn on_close_event(&mut self) -> bool {
        self.msg_queue.send(MsgToApp::ToggleVisibility).unwrap();
        false
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(1)
    }

    fn save(&mut self, _torage: &mut dyn eframe::Storage) {
        if let Some(persistent_state) = self.state.should_persist() {
            // set_value(storage, "persistent_state", &persistent_state);
            //
            println!("\npersisted state: {persistent_state:#?}\n");

            match try_save(persistent_state, &self.persistence_folder) {
                Ok(save_state) => {
                    self.last_saved = save_state.last_saved;
                }
                Err(err) => {
                    println!("failed to persist state with err={err:#?}")
                }
            };
        }
    }
}

fn try_read_note_if_newer(path: PathBuf, last_saved: u128) -> Result<Option<String>, io::Error> {
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
        initial_window_size: Some(vec2(350.0, 450.0)),
        min_window_size: Some(vec2(350.0, 450.0)),
        // max_window_size: Some(vec2(650.0, 750.0)),
        // fullsize_content: true,
        // decorated: false,
        resizable: true,
        always_on_top: true,
        run_and_return: true,
        window_builder: Some(Box::new(|builder| {
            #[cfg(target_os = "macos")]
            {
                use winit::platform::macos::WindowBuilderExtMacOS;
                return builder
                    .with_fullsize_content_view(true)
                    .with_titlebar_buttons_hidden(true)
                    .with_title_hidden(true)
                    .with_titlebar_transparent(true);
            }

            builder
        })),
        event_loop_builder: Some(Box::new(|builder| {
            #[cfg(target_os = "macos")]
            {
                use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
                builder.with_activation_policy(ActivationPolicy::Accessory);
            }
        })),

        ..Default::default()
    };

    eframe::run_native("Shelv", options, Box::new(|cc| Box::new(MyApp::new(cc)))).unwrap();
}

fn hide_app_on_macos() {
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
