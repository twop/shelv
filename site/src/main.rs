use axum::{extract::Path, response::{Html, Json}};
use enum_router::router;
use hyped::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::SocketAddr;
use tailwind_fuse::*;
use tokio;
use tower_http::services::ServeDir;

// Type-safe button styling with Nord colors
#[derive(TwClass)]
#[tw(class = "flex items-center justify-center rounded-lg font-medium transition-colors")]
struct ButtonStyle {
    size: ButtonSize,
    color: ButtonColor,
}

#[derive(TwVariant)]
enum ButtonSize {
    #[tw(default, class = "h-10 px-4 py-2 text-sm")]
    Default,
    #[tw(class = "h-9 px-3 py-2 text-sm")]
    Small,
    #[tw(class = "h-11 px-8 py-2 text-base")]
    Large,
}

#[derive(TwVariant)]
enum ButtonColor {
    #[tw(default, class = "bg-nord0 text-nord4 hover:bg-nord1")]
    Primary,
    #[tw(class = "bg-nord11 text-nord6 hover:bg-nord12")]
    Danger,
    #[tw(class = "bg-nord14 text-nord0 hover:bg-nord13")]
    Success,
}

// Enum router definition
#[router]
pub enum Route {
    #[get("/")]
    Root,
}

// Route handlers
async fn root() -> Html<String> {
    let content = div((
        h1("Welcome to Shelv").class("text-4xl font-bold text-nord6 mb-8"),
        p("A hackable playground for ephemeral thoughts.").class("text-nord4 text-lg mb-6"),
        create_button_html(
            &ButtonStyle { 
                size: ButtonSize::Large, 
                color: ButtonColor::Primary 
            }, 
            "Get Started".to_string()
        ),
    )).class("min-h-screen bg-nord0 flex flex-col items-center justify-center p-8");

    Html(render_to_string(content))
}

// HTML rendering helper
fn render_to_string(element: Element) -> String {
    render((
        doctype(),
        html((
            head((
                title("Shelv - Hackable Playground for Ephemeral Thoughts"),
                meta().charset("utf-8"),
                meta().name("viewport").content("width=device-width, initial-scale=1"),
                link("").rel("stylesheet").href("/assets/app.css"),
            )),
            body(element),
        )),
    ))
}

// Type-safe button component
fn create_button_html(button_style: &ButtonStyle, text: String) -> Element {
    element("button", text).class(&button_style.to_class())
}

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([127, 0, 0, 1], 4000));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    
    println!("Server running on http://127.0.0.1:4000");
    
    // Create the main router with enum_router
    let app_router = Route::router();
    
    // Add static file serving for assets
    let router = app_router.nest_service("/assets", ServeDir::new("assets"));
    
    axum::serve(listener, router).await.unwrap();
}
