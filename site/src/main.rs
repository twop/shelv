use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::Html,
};
use enum_router::router;
use hyped::*;
use shared::{Version, VersionResponse};
use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tailwind_fuse::*;
use tower_http::services::ServeDir;

use crate::rate_limiting::ApiCallRecord;
use crate::updates::{FileSystem, RealFileSystem, UpdateEntry, load_updates, update_page};

mod footer;
mod home;
mod proxy;
mod rate_limiting;
mod ui_components;
mod updates;

pub struct AppState {
    proxy_config: proxy::Config,
    rate_limiter: Mutex<rate_limiting::RateLimiter>,
    updates: Vec<UpdateEntry>,
}

#[derive(TwVariant)]
pub enum TextColor {
    #[tw(default, class = "text-nord5")]
    Default,

    #[tw(class = "text-nord4-darker")]
    Subtle,

    #[tw(class = "text-nord3")]
    VerySubtle,

    #[tw(class = "text-nord8")]
    Primary,

    #[tw(class = "text-nord6")]
    SubHeader,

    #[tw(class = "text-nord6")]
    MainHeader,

    #[tw(class = "text-nord11")]
    Red,
}

#[derive(TwVariant)]
pub enum BackgroundColor {
    #[tw(default, class = "bg-nord0-darker")]
    Default,
    #[tw(class = "bg-nord0-darker")]
    Dark,
    #[tw(class = "bg-nord0-dark")]
    Light,
    #[tw(class = "bg-transparent")]
    Transparent,
}

#[derive(TwVariant)]
pub enum BorderStyle {
    #[tw(default, class = "border-nord3")]
    Default,
    #[tw(class = "border-nord3")]
    LineBreak,
    #[tw(class = "border-nord2")]
    MediaBorder,
}

// Style structs for different component types
#[derive(TwClass)]
#[tw(class = "")]
pub struct StyledText {
    color: TextColor,
    style: TextStyle,
}

#[derive(TwVariant)]
pub enum TextStyle {
    #[tw(class = "text-4xl leading-tight font-semibold sm:text-5xl sm:leading-none")]
    MainHeader,

    #[tw(
        default,
        class = "text-2xl mb-4 leading-8 font-semibold sm:text-3xl sm:leading-9"
    )]
    SubHeader,

    #[tw(class = "text-base sm:text-lg leading-7 sm:leading-8")]
    LargeGeneralText,

    #[tw(class = "text-sm leading-6 sm:text-sm sm:leading-7")]
    SmallGeneralText,

    #[tw(class = "text-sm sm:text-sm")]
    NavMenu,
}

#[derive(TwClass)]
#[tw(class = "")]
pub struct LinkStyle {
    color: TextColor,
    hover: HoverState,
}

#[derive(TwVariant)]
pub enum HoverState {
    #[tw(default, class = "hover:underline")]
    Underline,
    #[tw(class = "hover:text-nord7")]
    ColorChange,
}

#[derive(TwVariant)]
pub enum SpacingSize {
    #[tw(class = "w-full h-4 sm:h-8")]
    Small,

    #[tw(default, class = "w-full h-8 sm:h-16")]
    Medium,

    #[tw(class = "w-full h-16 sm:h-32")]
    Large,
}

#[derive(TwClass)]
#[tw(class = "border-solid border-t-1 w-full")]
pub struct DividerStyle {
    color: BorderStyle,
}

#[derive(TwVariant)]
pub enum ButtonVariant {
    #[tw(
        default,
        class = r#"
        border-1 border-nord4-darker hover:border-nord7 active:border-nord8
        text-nord4 hover:text-nord7 active:text-nord8"#
    )]
    Secondary,

    #[tw(class = r#"
        text-nord4 hover:text-nord7 active:text-nord8"#)]
    SecondaryTextOnly,
}

#[derive(TwVariant)]
pub enum ButtonHeight {
    #[tw(default, class = r#" h-10"#)]
    FixedH10,

    #[tw(class = r#"py-3"#)]
    ContentBased,
}

#[derive(TwVariant)]
pub enum IconSize {
    #[tw(default, class = "size-5")]
    Default,

    #[tw(class = "size-4")]
    Small,

    #[tw(class = "size-3")]
    ExtraSmall,
}

#[derive(TwClass)]
#[tw(class = r#"
    inline-flex items-center
    font-medium text-center no-underline align-middle whitespace-nowrap
    rounded-lg select-none px-3 transition-all duration-150"#)]
pub struct ButtonStyle {
    variant: ButtonVariant,
    height: ButtonHeight,
}

// Enum router definition
#[router(Arc<AppState>)]
pub enum Route {
    #[get("/")]
    Root,
    #[get("/api/min-version")]
    MinVersion,

    #[get("/privacy")]
    Privacy,

    #[get("/updates")]
    UpdatesList,

    #[get("/updates/{version}")]
    UpdateDetail { version: String },

    // note that this is how the client will see construct url using genai
    // note that messages are coming from anthropic api url pattern
    #[post("/api/llm-claude/v1/messages")]
    ProxyAnthropicPost,
}

// Route handlers
async fn root() -> Html<String> {
    Html(render_to_string(home::home_page()))
}

async fn min_version() -> Json<VersionResponse> {
    Json(VersionResponse {
        min_version: Version("1.3.0".to_string()),
        latest_version: Version("1.4.0".to_string()),
    })
}

async fn privacy() -> &'static str {
    let privacy_content = include_str!("../../assets/privacy-policy.md");

    privacy_content
}

async fn updates_list(
    State(state): State<Arc<AppState>>,
) -> Result<axum::response::Redirect, StatusCode> {
    let updates = &state.updates;

    if updates.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Redirect to the latest update (first in the list, since sorted newest first)
    let latest = &updates[0];
    Ok(axum::response::Redirect::to(&format!(
        "/updates/{}",
        latest.version.to_route_format()
    )))
}

async fn update_detail(
    State(state): State<Arc<AppState>>,
    Path(version): Path<String>,
) -> Result<Html<String>, StatusCode> {
    // Parse the version from route format (1_3_9) to Version
    let version = updates::Version::parse(&version).map_err(|_| StatusCode::BAD_REQUEST)?;

    // if not found just take the latest
    let update = state
        .updates
        .iter()
        .find(|u| u.version == version)
        .or(state.updates.iter().next())
        .ok_or(StatusCode::NOT_FOUND)?;

    // Build the page with the theme
    let page = update_page(&state.updates, update);

    Ok(Html(render_to_string(page)))
}

async fn proxy_anthropic_post(
    State(state): State<Arc<AppState>>,
    req: axum::extract::Request,
) -> Result<axum::response::Response, (axum::http::StatusCode, String)> {
    // Rate limiting check
    {
        let mut limiter = state.rate_limiter.lock().unwrap();
        if !limiter.try_add_call_record(ApiCallRecord::new(chrono::Local::now())) {
            println!("EXIT: Rate limit exceeded");
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                "Rate limit exceeded. Please try again later.".to_string(),
            ));
        }
    }

    proxy::proxy_anthropic(&state.proxy_config, req).await
}

// HTML rendering helper
fn render_to_string(element: Element) -> String {
    render((
        doctype(),
        html((
            head((
                title("Shelv - Hackable, Local, AI-powered notes"),
                meta().charset("utf-8"),
                meta()
                    .name("viewport")
                    .content("width=device-width, initial-scale=1"),
                link("").rel("preconnect").href("https://rsms.me/"),
                link("")
                    .rel("stylesheet")
                    .href("https://rsms.me/inter/inter.css"),
                link("").rel("stylesheet").href("/assets/app.css"),
                link("").rel("icon").href("/assets/media/favicon.ico"),
            )),
            body(element).class(BackgroundColor::Default.as_class()),
        )),
    ))
}

#[tokio::main]
async fn main() {
    // this loads environment variables from .env file if it exists
    dotenvy::dotenv().ok();

    let shelv_magic_token = std::env::var("SHELV_MAGIC_TOKEN")
        .expect("SHELV_MAGIC_TOKEN environment variable is required");

    let anthropic_api_key = std::env::var("ANTHROPIC_API_KEY").ok();

    // Load updates at startup
    let fs = RealFileSystem;
    let updates_path = PathBuf::from("update-log");
    let updates = if let Some(updates_dir) = fs.as_dir(&updates_path) {
        match load_updates(&fs, &updates_dir) {
            Ok(updates) => {
                println!("Loaded {} update entries", updates.len());
                updates
            }
            Err(e) => {
                eprintln!("Warning: Failed to load updates: {}", e);
                Vec::new()
            }
        }
    } else {
        eprintln!("Warning: Updates directory not found at {:?}", updates_path);
        Vec::new()
    };

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    println!("Server running on http://0.0.0.0:8080");
    println!(
        "DANGER: token last 4 = {:?}",
        anthropic_api_key
            .as_ref()
            .map(|k| &k[((k.len() as isize - 5).max(0) as usize)..])
    );

    let proxy_config = proxy::Config {
        shelv_magic_token,
        anthropic_api_key,
    };

    let rate_limiter = rate_limiting::RateLimiter::new(20, Duration::from_secs(60));

    let state = Arc::new(AppState {
        proxy_config,
        rate_limiter: Mutex::new(rate_limiter),
        updates,
    });

    // Create the main router with enum_router
    let app_router = Route::router();

    // Add static file serving for assets and update-log
    let router = app_router
        .nest_service("/assets", ServeDir::new("assets"))
        .nest_service("/update-log", ServeDir::new("update-log"))
        .with_state(state);

    axum::serve(listener, router).await.unwrap();
}
