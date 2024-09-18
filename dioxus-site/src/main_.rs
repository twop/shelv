#![allow(non_snake_case)]

use dioxus::prelude::*;
use dioxus_logger::tracing::{info, Level};
mod home_page;
use home_page::HomePage;

#[derive(Clone, Routable, Debug, PartialEq)]
enum Route {
    #[route("/")]
    Home {},

    #[route("/new")]
    NewHome,

    #[route("/blog/:id")]
    Blog { id: i32 },
}

fn main() {
    // Init logger
    dioxus_logger::init(Level::INFO).expect("failed to init logger");
    info!("starting app");
    launch(App);
}

fn App() -> Element {
    rsx! {
        // Router::<Route> {}
        HomePage {}
    }
}

#[component]
fn Blog(id: i32) -> Element {
    rsx! {
        Link { to: Route::Home {}, "Go to counter" }
        "Blog post {id}"
    }
}

#[component]
fn NewHome() -> Element {
    rsx! {
        HomePage {}
        Link { to: Route::Home {}, "Go to counter" }
    }
}

#[component]
fn Home() -> Element {
    let mut count = use_signal(|| 0);

    rsx! {
        Link { to: Route::Blog { id: count() }, "Go to blog" }
        Link { to: Route::NewHome, "Go to Marketing" }
        div { class: "container mx-auto px-4 py-8",
            h1 { "High-Five counter: {count}" }
            p {
                button { onclick: move |_| count += 1, "Up high!" }
                button { onclick: move |_| count -= 1, "Down low!" }
            }
        }
    }
}
