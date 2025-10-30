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
use crate::updates::{FileSystem, RealFileSystem, UpdateEntry, load_updates, markdown_to_html};

mod footer;
mod home;
mod proxy;
mod rate_limiting;
mod ui_components;
mod updates;

// Import UI components
use ui_components::{ThemeColor, WaveDirection, content, space, theme, wave};

// Application state that includes updates
pub struct AppState {
    proxy_config: proxy::Config,
    rate_limiter: Mutex<rate_limiting::RateLimiter>,
    updates: Vec<UpdateEntry>,
}

// Semantic color variants using tailwind_fuse
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
        latest_version: Version("1.3.9".to_string()),
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

    // Find the update entry
    let update = state
        .updates
        .iter()
        .find(|u| u.version == version)
        .ok_or(StatusCode::NOT_FOUND)?;

    // Build the updates list for the sidebar
    let updates_list_items: Vec<Element> = state
        .updates
        .iter()
        .map(|u| {
            let is_active = u.version == version;
            let link_class = if is_active {
                tw_join!(
                    TextStyle::SmallGeneralText,
                    TextColor::Primary,
                    "font-semibold"
                )
            } else {
                tw_join!(
                    TextStyle::SmallGeneralText,
                    LinkStyle {
                        color: TextColor::Subtle,
                        hover: HoverState::ColorChange
                    }
                    .to_class()
                )
            };

            let display_text = format!(
                "{}{}",
                u.version.to_file_format(),
                u.optional_name
                    .as_ref()
                    .map(|n| format!(" - {}", n))
                    .unwrap_or_default()
            );

            li(a(display_text)
                .href(&format!("/updates/{}", u.version.to_route_format()))
                .class(&link_class))
            .class("mb-2")
        })
        .collect();

    // Convert markdown to HTML with image path resolution
    let html_content = markdown_to_html(&update.markdown_content, update.folder_path.path());

    // Build the page with the theme
    let page = update_page(
        updates_list_items,
        &update.version,
        &update.optional_name,
        html_content,
    );

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

fn update_page(
    updates_list: Vec<Element>,
    current_version: &updates::Version,
    optional_name: &Option<String>,
    markdown_html: String,
) -> Element {
    div((
        theme(
            ThemeColor::Dark,
            content((page_header(), space(SpacingSize::Small))),
        ),
        wave(WaveDirection::Up, ThemeColor::Dark, SpacingSize::Medium),
        theme(
            ThemeColor::Light,
            content((
                space(SpacingSize::Small),
                // Two-column layout
                div((
                    // Sidebar with updates list
                    div((
                        h2("Updates").class(&tw_join!(
                            TextStyle::SubHeader,
                            TextColor::SubHeader,
                            "mb-4"
                        )),
                        ul(updates_list).class("space-y-1"),
                    ))
                    .class("w-full md:w-64 mb-8 md:mb-0 md:pr-8 md:border-r border-nord3"),
                    // Main content area
                    div((
                        h1(format!(
                            "Update {}{}",
                            current_version.to_file_format(),
                            optional_name
                                .as_ref()
                                .map(|n| format!(" - {}", n))
                                .unwrap_or_default()
                        ))
                        .class(&tw_join!(
                            TextStyle::MainHeader,
                            TextColor::MainHeader,
                            "mb-6"
                        )),
                        // Markdown content container - styled container only, no markdown styling
                        div(danger(&markdown_html)).class(&tw_join!(
                            TextStyle::LargeGeneralText,
                            TextColor::Default,
                            "prose prose-nord max-w-none"
                        )),
                    ))
                    .class("flex-1 md:pl-8"),
                ))
                .class("flex flex-col md:flex-row"),
                space(SpacingSize::Large),
            )),
        ),
        // Footer section
        footer::footer_section(),
    ))
    .class(&tw_join!(
        "flex flex-col",
        BackgroundColor::Default.as_class()
    ))
}

fn page_header() -> Element {
    div((
        div(
            a(shelv_logo())
                .href("/")
                .class("inline-flex items-center space-x-2 leading-6 font-medium transition ease-in-out duration-150"),
        ),
        // Desktop navigation - visible on md screens and up
        div(Vec::from([("Updates", "/updates")])
            .into_iter()
            .map(|(name, link_to)| {
                a(name).href(link_to).class(&tw_join!(
                    TextStyle::NavMenu,
                    LinkStyle {
                        color: TextColor::Subtle,
                        hover: HoverState::ColorChange
                    }
                    .to_class()
                ))
            })
            .collect::<Vec<_>>())
        .class("hidden md:flex gap-x-8"),
        // Discord icon - always visible
        div(a(discord_icon(IconSize::Default))
            .href(include_str!("../../distribution/discord_invite.txt").trim())
            .class(&tw_join!(
                ButtonVariant::SecondaryTextOnly,
                TextColor::Subtle
            ))),
    ))
    .class("flex justify-between items-center py-6")
}

fn shelv_logo() -> impl Render {
    let svg_content = include_str!("../assets/icons/shelv-logo.svg");
    danger(svg_content.replace("<class>", "shelv-logo"))
}

fn discord_icon(size: IconSize) -> impl Render {
    let svg_content = include_str!("../assets/icons/discord.svg");
    let classes = tw_join!(size, "fill-current inline");
    danger(svg_content.replace("<class>", &classes))
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
