#![allow(non_snake_case)]

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use dioxus::{fullstack::Config, prelude::*};
use dioxus_logger::tracing;
mod home_page;
use home_page::HomePage;

#[derive(Clone, Routable, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
    dioxus_logger::init(tracing::Level::INFO).expect("failed to init logger");
    tracing::info!("starting app");

    // launch(App);

    // // let serve_on_addr = SocketAddr::new("[::]:8080".into(), 8080);
    // // launch_fullstack(app);
    // LaunchBuilder::new()
    //     .with_cfg(server_only! {Config::new().addr(serve_on_addr)})
    //     .launch(App);

    #[allow(dead_code)]
    #[cfg(feature = "server")]
    {
        let serve_on_addr: SocketAddr = "[::]:8080".parse().unwrap();
        LaunchBuilder::fullstack()
            .with_cfg(Config::new().addr(serve_on_addr))
            .launch(App);
    }
    #[allow(dead_code)]
    #[cfg(feature = "web")]
    {
        launch(App);
    }
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

// fn App() -> Element {
//     rsx! {
//         Router::<Route> {}
//     }
// }

// #[component]
// fn Blog(id: i32) -> Element {
//     rsx! {
//         Link { to: Route::Home {}, "Go to counter" }
//         "Blog post {id}"
//     }
// }

// #[component]
// fn Home() -> Element {
//     let mut count = use_signal(|| 0);
//     let mut text = use_signal(|| String::from("..."));

//     rsx! {
//         Link {
//             to: Route::Blog {
//                 id: count()
//             },
//             "Go to blog"
//         }
//         div {
//             h1 { "High-Five counter: {count}" }
//             button { onclick: move |_| count += 1, "Up high!" }
//             button { onclick: move |_| count -= 1, "Down low!" }
//             button {
//                 onclick: move |_| async move {
//                     if let Ok(data) = get_server_data().await {
//                         tracing::info!("Client received: {}", data);
//                         text.set(data.clone());
//                         post_server_data(data).await.unwrap();
//                     }
//                 },
//                 "Get Server Data"
//             }
//             p { "Server data: {text}"}
//         }
//     }
// }

#[server(PostServerData)]
async fn post_server_data(data: String) -> Result<(), ServerFnError> {
    tracing::info!("Server received: {}", data);
    Ok(())
}

#[server(GetServerData)]
async fn get_server_data() -> Result<String, ServerFnError> {
    Ok("Hello from the server!".to_string())
}
