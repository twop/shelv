#[cfg(feature = "reload")]
use hot_lib::*;
#[cfg(not(feature = "reload"))]
use lib::*;

#[cfg(feature = "reload")]
#[hot_lib_reloader::hot_module(dylib = "lib")]
mod hot_lib {
    use eframe::egui;
    pub use lib::AppState;
    pub use lib::AppTheme;

    hot_functions_from_file!("lib/src/lib.rs");
    hot_functions_from_file!("lib/src/theme.rs");
    hot_functions_from_file!("lib/src/nord.rs");

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
}

impl MyApp {
    pub fn new(cc: &CreationContext) -> Self {
        let theme = Default::default();
        configure_styles(&cc.egui_ctx, &theme);

        Self {
            state: create_app_state(theme),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        render(&mut self.state, ctx, frame);
    }
}

fn main() {
    let options = eframe::NativeOptions {
        follow_system_theme: true,
        default_theme: eframe::Theme::Dark,
        min_window_size: Some(vec2(350.0, 450.0)),
        max_window_size: Some(vec2(350.0, 450.0)),
        resizable: true,
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
