#![feature(iter_intersperse)]
#![feature(let_chains)]

use app_actions::{process_app_action, TextChange};
use app_state::{AppInitData, AppState, MsgToApp};
use app_ui::{is_shortcut_match, render_app, AppRenderData, RenderAppResult};
use boa_engine::{Context, Source};
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};

use image::ImageFormat;

use persistent_state::PersistentState;
use text_structure::TextStructure;
use theme::{configure_styles, get_font_definitions};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};
// use tray_item::TrayItem;

use std::sync::mpsc::{sync_channel, SyncSender};

use eframe::{
    egui::{
        self,
        text::{CCursor, CCursorRange},
        Id,
    },
    epaint::vec2,
    get_value, set_value, CreationContext,
};

use crate::{
    app_actions::apply_text_changes, app_ui::char_index_from_byte_index, text_structure::ByteRange,
};

mod app_actions;
mod app_state;
mod app_ui;
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
    hotkeys_manager: GlobalHotKeyManager,
    tray: TrayIcon,
    msg_queue: SyncSender<MsgToApp>,
}

impl MyApp {
    pub fn new(cc: &CreationContext) -> Self {
        let theme = Default::default();
        configure_styles(&cc.egui_ctx, &theme);

        let mut fonts = get_font_definitions();

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

        let persistent_state: Option<PersistentState> =
            cc.storage.and_then(|s| get_value(s, "persistent_state"));

        Self {
            state: AppState::new(AppInitData {
                theme,
                msg_queue: msg_queue_rx,
                persistent_state,
            }),
            hotkeys_manager,
            tray: tray_icon,
            msg_queue: msg_queue_tx,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let text_edit_id = Id::new("text_edit");

        let app_state = &mut self.state;
        let note = &mut app_state.notes[app_state.selected_note as usize];
        let mut cursor: Option<ByteRange> = note.cursor.clone();

        let editor_text = &mut note.text;
        let mut text_structure = app_state
            .text_structure
            .take()
            .unwrap_or_else(|| TextStructure::new(editor_text));

        // handling commands
        // sych as {tab, enter} inside a list
        let changes: Option<(Vec<TextChange>, ByteRange)> = cursor.clone().and_then(|byte_range| {
            ctx.input_mut(|input| {
                // only one command can be handled at a time
                app_state.editor_commands.iter().find_map(|editor_command| {
                    let keyboard_shortcut = editor_command.shortcut();
                    if is_shortcut_match(input, &keyboard_shortcut) {
                        let res = editor_command.try_handle(
                            &text_structure,
                            &editor_text,
                            byte_range.clone(),
                        );
                        if res.is_some() {
                            // remove the keys from the input
                            input.consume_shortcut(&keyboard_shortcut);
                        }
                        res.map(|changes| (changes, byte_range.clone()))
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

        let RenderAppResult(actions, updated_structure, ccursor_range, updated_layout) = render_app(
            text_structure,
            editor_text,
            vis_state,
            &app_state.app_shortcuts,
            &app_state.theme,
            ctx,
        );

        app_state.text_structure = Some(updated_structure);
        app_state.computed_layout = updated_layout;
        app_state.notes[app_state.selected_note as usize].cursor = ccursor_range;

        for action in actions {
            process_app_action(action, ctx, app_state, text_edit_id)
        }
    }

    fn on_close_event(&mut self) -> bool {
        self.msg_queue.send(MsgToApp::ToggleVisibility).unwrap();
        false
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Some(persistent_state) = self.state.should_persist() {
            set_value(storage, "persistent_state", &persistent_state);
        }
    }
}

fn main() {
    let options = eframe::NativeOptions {
        default_theme: eframe::Theme::Dark,
        initial_window_size: Some(vec2(350.0, 450.0)),
        min_window_size: Some(vec2(350.0, 450.0)),
        max_window_size: Some(vec2(650.0, 750.0)),
        // fullsize_content: true,
        // decorated: false,
        resizable: true,
        always_on_top: true,
        run_and_return: true,
        window_builder: Some(Box::new(|builder| {
            #[cfg(target_os = "macos")]
            use winit::platform::macos::WindowBuilderExtMacOS;
            #[cfg(target_os = "macos")]
            return builder
                .with_fullsize_content_view(true)
                .with_titlebar_buttons_hidden(true)
                .with_title_hidden(true)
                .with_titlebar_transparent(true);

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
