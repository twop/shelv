#![feature(iter_intersperse)]
#![feature(let_chains)]

use app_state::{AppInitData, AppState, MsgToApp};
use app_ui::render_app;
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};

use image::ImageFormat;

use persistent_state::PersistentState;
use theme::{configure_styles, get_font_definitions};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};
// use tray_item::TrayItem;

use std::sync::mpsc::{sync_channel, SyncSender};

use eframe::{
    egui::{self},
    epaint::vec2,
    get_value, set_value, CreationContext,
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
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        render_app(&mut self.state, ctx, frame);
    }

    fn on_close_event(&mut self) -> bool {
        self.msg_queue.send(MsgToApp::ToggleVisibility).unwrap();
        return false;
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
