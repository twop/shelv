use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
#[cfg(feature = "reload")]
use hot_lib::*;
use lib::AsyncMessage;

#[cfg(not(feature = "reload"))]
use lib::create_app_state;

use std::{
    sync::mpsc::{channel, sync_channel},
    thread,
};

#[cfg(feature = "reload")]
#[hot_lib_reloader::hot_module(dylib = "lib")]
mod hot_lib {
    use eframe::egui;
    pub use lib::configure_styles;
    pub use lib::AppInitData;
    pub use lib::AppState;
    pub use lib::AppTheme;

    hot_functions_from_file!("lib/src/lib.rs");
    // hot_functions_from_file!("lib/src/theme.rs");
    // hot_functions_from_file!("lib/src/nord.rs");

    #[lib_change_subscription]
    pub fn subscribe() -> hot_lib_reloader::LibReloadObserver {}
}

// -=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-

use eframe::{
    egui::{self, TextStyle, Visuals},
    epaint::{vec2, FontFamily, FontId},
    CreationContext,
};

pub struct MyApp {
    state: AppState,
    hotkeys_manager: GlobalHotKeyManager,
}

impl MyApp {
    pub fn new(cc: &CreationContext) -> Self {
        let theme = Default::default();
        configure_styles(&cc.egui_ctx, &theme);

        let (msg_queue_tx, msg_queue_rx) = sync_channel::<AsyncMessage>(10);
        let hotkeys_manager = GlobalHotKeyManager::new().unwrap();
        let global_open_hotkey = HotKey::new(Some(Modifiers::SHIFT | Modifiers::META), Code::KeyM);

        hotkeys_manager.register(global_open_hotkey).unwrap();

        let open_hotkey = global_open_hotkey.clone();
        let ctx = cc.egui_ctx.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |ev: GlobalHotKeyEvent| {
            if ev.id == open_hotkey.id() {
                msg_queue_tx
                    .send(AsyncMessage::OpenWithGlobalHotkey)
                    .unwrap();
                ctx.request_repaint();
                println!("handler: {:?}", open_hotkey);
            }
        }));

        Self {
            state: create_app_state(AppInitData {
                theme,
                msg_queue: msg_queue_rx,
            }),
            hotkeys_manager,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        render(&mut self.state, ctx, frame);
    }

    fn on_close_event(&mut self) -> bool {
        self.state.hidden = true;
        return false;
    }
}

fn main() {
    let options = eframe::NativeOptions {
        follow_system_theme: true,
        default_theme: eframe::Theme::Dark,
        min_window_size: Some(vec2(350.0, 450.0)),
        max_window_size: Some(vec2(350.0, 450.0)),
        resizable: true,
        always_on_top: true,
        run_and_return: true,
        event_loop_builder: Some(Box::new(|builder| {
            #[cfg(target_os = "macos")]
            {
                use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
                builder.with_activation_policy(ActivationPolicy::Accessory);
            }
        })),

        ..Default::default()
    };

    eframe::run_native(
        "Memento",
        options,
        Box::new(|cc| {
            // When hot reload is enabled, repaint after every lib change
            #[cfg(feature = "reload")]
            {
                let ctx = cc.egui_ctx.clone();
                std::thread::spawn(move || loop {
                    hot_lib::subscribe().wait_for_reload();
                    ctx.request_repaint();
                });
            }
            Box::new(MyApp::new(cc))
        }),
    )
    .unwrap();
}
