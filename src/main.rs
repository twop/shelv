#![feature(iter_intersperse)]

use app::{create_app_state, render, AppIcons, AppInitData, AppState, AsyncMessage};
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};

use image::ImageFormat;

use theme::configure_styles;
use tray_icon::{icon::Icon, menu::MenuEvent, TrayEvent, TrayIcon, TrayIconBuilder};
// use tray_item::TrayItem;

use std::{
    sync::mpsc::{channel, sync_channel, SyncSender},
    thread,
};

pub mod app;
pub mod nord;
pub mod picker;
pub mod theme;

use eframe::{
    egui::{self, TextStyle, Visuals},
    epaint::{vec2, Color32, FontFamily, FontId},
    CreationContext,
};

pub struct MyApp {
    state: AppState,
    hotkeys_manager: GlobalHotKeyManager,
    tray: TrayIcon,
    msg_queue: SyncSender<AsyncMessage>,
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
        let sender = msg_queue_tx.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |ev: GlobalHotKeyEvent| {
            if ev.id == open_hotkey.id() {
                sender.send(AsyncMessage::ToggleVisibility).unwrap();
                ctx.request_repaint();
                println!("handler: {:?}", open_hotkey);
            }
        }));

        let ctx = cc.egui_ctx.clone();
        let sender = msg_queue_tx.clone();
        TrayEvent::set_event_handler(Some(move |ev| {
            sender.send(AsyncMessage::ToggleVisibility).unwrap();
            ctx.request_repaint();

            println!("tray event: {:?}", ev);
        }));
        let tray_image = image::load_from_memory_with_format(
            include_bytes!("../assets/tray-icon.png",),
            ImageFormat::Png,
        )
        .unwrap();

        let tray_icon = TrayIconBuilder::new()
            .with_tooltip("Show/Hide Memoro")
            .with_icon(Icon::from_rgba(tray_image.into_bytes(), 64, 64).unwrap())
            .build()
            .unwrap();

        Self {
            state: create_app_state(AppInitData {
                icons: load_app_icons(theme.colors.button_fg),
                theme,
                msg_queue: msg_queue_rx,
            }),
            hotkeys_manager,
            tray: tray_icon,
            msg_queue: msg_queue_tx,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        render(&mut self.state, ctx, frame);
    }

    fn on_close_event(&mut self) -> bool {
        self.msg_queue.send(AsyncMessage::ToggleVisibility).unwrap();

        return false;
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

    eframe::run_native(
        "Shelv",
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

            //cc.egui_ctx.

            // let mut tray = TrayItem::new("Tray Example", "").unwrap();

            // tray.add_label("Tray Label").unwrap();

            // tray.add_menu_item("Hello", || {
            //     println!("Hello!");
            // })
            // .unwrap();

            // let mut inner = tray.inner_mut();
            // // inner.set_icon("./assets/tray-icon.png").unwrap();
            // inner.add_quit_item("Quit");
            // inner.display();

            // tray_icon.set_visible(true);

            Box::new(MyApp::new(cc))
        }),
    )
    .unwrap();
}

pub fn load_app_icons(stroke_color: Color32) -> AppIcons {
    let [more, gear, question_mark, close] = [
        ("more", include_str!("../assets/icons/more.svg")),
        ("gear", include_str!("../assets/icons/gear.svg")),
        (
            "question-mark",
            include_str!("../assets/icons/question-mark.svg"),
        ),
        ("close", include_str!("../assets/icons/x.svg")),
    ]
    .map(|(name, svg)| {
        let [r, g, b, a] = stroke_color.to_array();
        let svg = svg.replace(
            "stroke=\"white\"",
            format!("stroke=\"rgba({}, {}, {}, {})\"", r, g, b, a).as_str(),
        );
        egui_extras::RetainedImage::from_svg_bytes_with_size(
            name,
            svg.as_bytes(),
            egui_extras::image::FitTo::Size(64, 64),
        )
        .unwrap()
    });

    AppIcons {
        more,
        gear,
        question_mark,
        close,
    }
}
