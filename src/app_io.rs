use std::{
    cmp::Ordering,
    collections::BTreeMap,
    ffi::CString,
    fs::File,
    io::{self, Read},
    path::PathBuf,
    sync::mpsc::SyncSender,
};

use eframe::egui;
use genai::{
    ModelIden, ServiceTarget,
    adapter::AdapterKind,
    chat::{ChatMessage, ChatRequest, ChatStreamEvent, StreamChunk},
    resolver::{AuthData, AuthResolver, Endpoint, ServiceTargetResolver},
};
use global_hotkey::GlobalHotKeyManager;
use shared::{Version, VersionResponse};

use crate::{
    app_actions::{AppIO, HideMode, LLMBlockRequest, LLMPromptRequest, SettingsForAiRequests},
    app_state::{InlineLLMResponseChunk, MsgToApp},
    command::create_ai_keybindings_documentation,
    persistent_state::get_utc_timestamp,
};

use tokio_stream::StreamExt;

pub struct RegisteredGlobalHotkey {
    egui_shortcut: egui::KeyboardShortcut,
    system_hotkey: global_hotkey::hotkey::HotKey,
    handler: Box<dyn Fn() -> MsgToApp>,
}

pub struct RealAppIO {
    pub hotkeys_manager: GlobalHotKeyManager,
    pub registered_hotkeys: BTreeMap<u32, RegisteredGlobalHotkey>,
    pub egui_ctx: egui::Context,
    pub msg_queue: SyncSender<MsgToApp>,
    pub shelv_folder: PathBuf,
    pub shelv_api_server: String,
    pub shelv_magic_token: String,
    pub debug_chat_prompts: bool,
    pub current_version: Version,
}

impl RealAppIO {
    pub fn new(
        hotkeys_manager: GlobalHotKeyManager,
        egui_ctx: egui::Context,
        msg_queue: SyncSender<MsgToApp>,
        shelv_folder: PathBuf,
        shelv_api_server: String,
        shelv_magic_token: String,
        debug_chat_prompts: bool,
        current_version: Version,
    ) -> Self {
        Self {
            hotkeys_manager,
            registered_hotkeys: Default::default(),
            egui_ctx,
            msg_queue,
            shelv_folder,
            shelv_api_server,
            shelv_magic_token,
            debug_chat_prompts,
            current_version,
        }
    }
}

pub const DEFAULT_REAL_LLM_MODEL: &str = "claude-3-5-haiku-20241022";
pub const SHELV_LLM_PROXY_MODEL: &str = "shelv-claude";

impl AppIO for RealAppIO {
    fn hide_app(&self, mode: HideMode) {
        hide_app_on_macos(mode);
    }

    fn try_read_note_if_newer(
        &self,
        path: &PathBuf,
        last_saved: u128,
    ) -> Result<Option<String>, io::Error> {
        try_read_note_if_newer(path, last_saved)
    }

    fn try_map_hotkey(&self, hotkey_id: u32) -> Option<MsgToApp> {
        self.registered_hotkeys
            .get(&hotkey_id)
            .map(|key| (key.handler)())
    }

    fn bind_global_hotkey(
        &mut self,
        shortcut: egui::KeyboardShortcut,
        handler: Box<dyn Fn() -> MsgToApp>,
    ) -> Result<(), String> {
        let system_hotkey = convert_egui_shortcut_to_global_hotkey(shortcut);

        self.hotkeys_manager
            .register(system_hotkey)
            .map_err(|err| err.to_string())?;

        self.registered_hotkeys.insert(
            system_hotkey.id(),
            RegisteredGlobalHotkey {
                egui_shortcut: shortcut,
                system_hotkey,
                handler,
            },
        );

        Ok(())
    }

    fn cleanup_all_global_hotkeys(&mut self) -> Result<(), String> {
        for hotkey in self.registered_hotkeys.values() {
            self.hotkeys_manager
                .unregister(hotkey.system_hotkey)
                .map_err(|err| err.to_string())?;
        }
        self.registered_hotkeys.clear();
        Ok(())
    }

    fn execute_llm_block(&self, _question: LLMBlockRequest, _cx: SettingsForAiRequests) {
        unreachable!("That should not be called, the feature is removed")
    }

    fn execute_llm_prompt(&self, quesion: LLMPromptRequest, cx: SettingsForAiRequests) {
        let egui_ctx = self.egui_ctx.clone();
        let sender = self.msg_queue.clone();

        let LLMPromptRequest {
            prompt,
            selection,
            selection_location,
            before_selection,
            after_selection,
        } = quesion;

        let SettingsForAiRequests {
            commands,
            llm_settings,
        } = cx;
        let shelv_system_prompt = include_str!("./prompts/shelv-system-prompt.md").replace(
            "{{current_keybindings}}",
            &create_ai_keybindings_documentation(commands),
        );

        // cloned because it is goint to be used inside async block
        let llm_settings = llm_settings.cloned();

        // None -> end of the stream
        let send = move |chunk: InlineLLMResponseChunk| {
            sender
                .send(MsgToApp::InlineLLMResponse {
                    response: (chunk),
                    address: selection_location,
                })
                .unwrap();

            egui_ctx.request_repaint();
        };

        let (model, system_prompt, use_shelv_propmpt, token) = llm_settings
            .map(|s| {
                (
                    s.model,
                    s.system_prompt,
                    s.use_shelv_system_prompt.unwrap_or(true),
                    s.token,
                )
            })
            .unwrap_or_else(|| (SHELV_LLM_PROXY_MODEL.to_string(), None, true, None));

        let chat_req = ChatRequest::new(Vec::from_iter(
            use_shelv_propmpt
                .then(|| ChatMessage::system(shelv_system_prompt))
                .into_iter()
                .chain(system_prompt.map(|sp| ChatMessage::system(sp)))
                .chain([ChatMessage::user({
                    let user_template = include_str!("./prompts/inline-prompt-system-extra.md");

                    for pl in ["{{prompt}}", "{{before}}", "{{selection}}", "{{after}}"] {
                        assert!(
                            user_template.contains(pl),
                            "Template is missing required placeholder: {}",
                            pl
                        );
                    }

                    user_template
                        .replace("{{prompt}}", &prompt)
                        .replace("{{before}}", &before_selection)
                        .replace("{{selection}}", &selection)
                        .replace("{{after}}", &after_selection)
                })]),
        ));

        // Dump chat request to file for debugging and history
        let time = {
            let now = std::time::SystemTime::now();
            let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap();
            let secs = duration.as_secs();
            let hours = (secs / 3600) % 24;
            let minutes = (secs / 60) % 60;
            let seconds = secs % 60;
            format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
        };
        let debug_folder = PathBuf::from("prompts");
        let debug_file = debug_folder.join(format!("req_{}.md", time));

        if !debug_folder.exists() {
            std::fs::create_dir(&debug_folder).unwrap();
        }

        let mut contents = String::new();
        for msg in &chat_req.messages {
            contents.push_str(&format!(
                "---{}---\n{}\n\n",
                match msg.role {
                    genai::chat::ChatRole::System => "System",
                    genai::chat::ChatRole::User => "User",
                    genai::chat::ChatRole::Assistant => "Assistant",
                    _ => "Unknown",
                },
                msg.content.text_as_str().unwrap()
            ));
        }

        let (service_target_resolver, auth_resolver) = if model == SHELV_LLM_PROXY_MODEL {
            prepare_shelv_providers(&self.shelv_api_server, &self.shelv_magic_token)
        } else {
            prepare_general_providers(token.as_deref())
        };

        let debug_chat_prompts = self.debug_chat_prompts;
        tokio::spawn(async move {
            if debug_chat_prompts {
                println!("-----llm inline req: {chat_req:#?}");
                if let Err(err) = std::fs::write(&debug_file, contents) {
                    println!("failed to dump llm logs err: {err:#?}");
                } else {
                    println!("written llm logs here: {debug_file:#?}");
                }
            }

            let client = genai::Client::builder()
                .with_auth_resolver(auth_resolver)
                .with_service_target_resolver(service_target_resolver)
                .build();

            let chat_res = client
                .exec_chat_stream(model.as_str(), chat_req, None)
                .await;

            use InlineLLMResponseChunk::*;
            match chat_res {
                Ok(mut stream) => {
                    while let Some(stream_event) = stream.stream.next().await {
                        match stream_event {
                            Ok(ChatStreamEvent::Chunk(StreamChunk { content })) => {
                                send(Chunk(content))
                            }
                            Ok(ChatStreamEvent::End(_)) => send(End),
                            Ok(ChatStreamEvent::Start) => (),
                            Ok(ChatStreamEvent::ReasoningChunk(_)) => (),
                            Err(e) => {
                                send(ResponseError(format!(
                                    "Error getting response chunk: {debug:#?}\n{err}",
                                    debug = e,
                                    err = e
                                )));
                                break;
                            }
                        }
                    }
                }
                Err(err) => {
                    send(ResponseError(format!("Error sending request: {:#?}", err)));
                }
            };
        });
    }

    fn open_shelv_folder(&self) -> Result<(), Box<dyn std::error::Error>> {
        open_folder_in_finder(&self.shelv_folder)
    }

    fn capture_sentry_message<F>(
        &self,
        message: &str,
        level: sentry::Level,
        scope: F,
    ) -> sentry::types::Uuid
    where
        F: FnOnce(&mut sentry::Scope),
    {
        return sentry::with_scope(scope, || sentry::capture_message(message, level));
    }

    fn copy_to_clipboard(&self, text: String) {
        self.egui_ctx.copy_text(text);
    }

    fn start_update_checker(&self) {
        let sender = self.msg_queue.clone();
        let current_version = self.current_version.clone();
        let shelv_min_version_url = format!("{}/api/min-version", self.shelv_api_server);
        tokio::spawn(async move {
            loop {
                let client = reqwest::Client::new();
                let response = client.get(shelv_min_version_url.clone()).send().await;

                match response {
                    Ok(response) => {
                        let version_response = response.json::<VersionResponse>().await;

                        match version_response {
                            Ok(version_response) => {
                                if let Some(msg) = match (
                                    compare_versions(
                                        &current_version,
                                        &version_response.latest_version,
                                    ),
                                    compare_versions(
                                        &current_version,
                                        &version_response.min_version,
                                    ),
                                ) {
                                    (_, Ordering::Less) => {
                                        Some(MsgToApp::UpdateRequired(version_response.min_version))
                                    }
                                    (Ordering::Less, _) => Some(MsgToApp::UpdateAvailable(
                                        version_response.latest_version,
                                    )),
                                    _ => None,
                                } {
                                    sender.send(msg).unwrap();
                                }
                            }
                            Err(err) => {
                                // Keep check though, hopefully someone notices they messed up and fixes it.
                                println!("Error parsing min version response: {err:#?}");
                                // TODO maybe log parsing errors to sentry?
                            }
                        }
                    }
                    Err(err) => {
                        // Keep checking though, might be intermittent network issue.
                        println!("Error requesting min version response: {err:#?}");
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(10 * 60)).await;
            }
        });
    }

    fn open_app_store_for_shelv_update(&self) {
        open_url_with_nsworkspace("itms-apps://apps.apple.com/app/shelv-notes/id649947868")
            .unwrap_or_else(|err| {
                println!("Error opening app store URL: {}", err);
            });
    }
}

fn prepare_shelv_providers(
    api_server: &str,
    magic_token: &str,
) -> (ServiceTargetResolver, AuthResolver) {
    let api_server = api_server.to_string();
    let magic_token = magic_token.to_string();

    let service_target_resolver = ServiceTargetResolver::from_resolver_fn(
        move |_service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
            // TODO use router enum string representation, but that will require project restructuring due to reuse
            let endpoint = Endpoint::from_owned(format!("{}/api/llm-claude/v1/", &api_server));
            let auth = AuthData::from_single(magic_token.clone());
            let model = ModelIden::new(AdapterKind::Anthropic, DEFAULT_REAL_LLM_MODEL);

            Ok(ServiceTarget {
                endpoint,
                auth,
                model,
            })
        },
    );

    let auth_resolver = AuthResolver::from_resolver_fn(
        move |_model_iden: genai::ModelIden| -> Result<Option<genai::resolver::AuthData>, genai::resolver::Error> {
            // For shelv-claude models, auth is handled by service target resolver
            Ok(None)
        },
    );

    (service_target_resolver, auth_resolver)
}

fn prepare_general_providers(token: Option<&str>) -> (ServiceTargetResolver, AuthResolver) {
    let service_target_resolver = ServiceTargetResolver::from_resolver_fn(
        |service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
            Ok(service_target)
        },
    );

    let token_owned = token.map(|t| t.to_string());
    let auth_resolver = AuthResolver::from_resolver_fn(
        move |model_iden: genai::ModelIden| -> Result<Option<genai::resolver::AuthData>, genai::resolver::Error> {
            let genai::ModelIden {
                adapter_kind: _,
                model_name: _,
            } = model_iden;

            let auth = token_owned.map(|token|AuthData::from_single(token)) ;
            Ok(auth)
        },
    );

    (service_target_resolver, auth_resolver)
}

fn try_read_note_if_newer(path: &PathBuf, last_saved: u128) -> Result<Option<String>, io::Error> {
    let mut file = File::open(path)?;
    let meta = file.metadata()?;
    let modified_at = meta.modified()?;

    if get_utc_timestamp(modified_at) > last_saved + 10 {
        // println!(
        //     "updating note {note_file:?}, \nlast_saved={}\nmodified_at={}",
        //     self.last_saved,
        //     get_utc_timestamp(modified_at)
        // );
        let mut content = String::new();
        file.read_to_string(&mut content)?;

        Ok(Some(content))
    } else {
        Ok(None)
    }
}

fn hide_app_on_macos(mode: HideMode) {
    // https://developer.apple.com/documentation/appkit/nsapplication/1428733-hide
    use objc2::rc::Id;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send, msg_send_id};
    unsafe {
        let app: Id<AnyObject> = msg_send_id![class!(NSApplication), sharedApplication];
        let arg = app.as_ref();

        match mode {
            HideMode::HideApp => {
                let _: () = msg_send![&app, hide:arg];
            }
            HideMode::YieldFocus => {
                // this for whatever reason actually works
                // I was unable to find a better way to just yield focus instead of doin hide + unhide
                let _: () = msg_send![&app, hide:arg];
                let _: () = msg_send![&app, unhideWithoutActivation];
            }
        }
    }
}

fn open_folder_in_finder(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    use objc2::rc::Id;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send, msg_send_id};
    unsafe {
        let workspace: Id<AnyObject> = msg_send_id![class!(NSWorkspace), sharedWorkspace];

        // Convert the Rust string to a C string
        let c_path = CString::new(path.to_str().ok_or("Invalid UTF-8 in path")?)?;
        let ns_string: Id<AnyObject> =
            msg_send_id![class!(NSString), stringWithUTF8String:c_path.as_ptr()];

        let _: bool = msg_send![
            &workspace,
            selectFile: std::ptr::null::<AnyObject>()
            inFileViewerRootedAtPath: &*ns_string
        ];

        Ok(())
    }
}
fn convert_egui_shortcut_to_global_hotkey(
    shortcut: egui::KeyboardShortcut,
) -> global_hotkey::hotkey::HotKey {
    let mut modifiers = global_hotkey::hotkey::Modifiers::empty();
    if shortcut.modifiers.alt {
        modifiers |= global_hotkey::hotkey::Modifiers::ALT;
    }
    if shortcut.modifiers.ctrl {
        modifiers |= global_hotkey::hotkey::Modifiers::CONTROL;
    }
    if shortcut.modifiers.shift {
        modifiers |= global_hotkey::hotkey::Modifiers::SHIFT;
    }
    if shortcut.modifiers.mac_cmd {
        modifiers |= global_hotkey::hotkey::Modifiers::META;
    }

    use egui::Key::*;
    use global_hotkey::hotkey::Code::*;

    let code = match shortcut.logical_key {
        A => KeyA,
        B => KeyB,
        C => KeyC,
        D => KeyD,
        E => KeyE,
        F => KeyF,
        G => KeyG,
        H => KeyH,
        I => KeyI,
        J => KeyJ,
        K => KeyK,
        L => KeyL,
        M => KeyM,
        N => KeyN,
        O => KeyO,
        P => KeyP,
        Q => KeyQ,
        R => KeyR,
        S => KeyS,
        T => KeyT,
        U => KeyU,
        V => KeyV,
        W => KeyW,
        X => KeyX,
        Y => KeyY,
        Z => KeyZ,
        // TODO: Add more mappings as needed
        // TODO2: is there a way not to do it manually?
        _ => KeyA, // Default to KeyA for unmapped keys
    };

    global_hotkey::hotkey::HotKey::new(Some(modifiers), code)
}

fn open_url_with_nsworkspace(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    use objc2::rc::Id;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send, msg_send_id};

    unsafe {
        let workspace: Id<AnyObject> = msg_send_id![class!(NSWorkspace), sharedWorkspace];

        // Convert the Rust string to a C string
        let c_url = CString::new(url)?;
        let ns_string: Id<AnyObject> =
            msg_send_id![class!(NSString), stringWithUTF8String:c_url.as_ptr()];

        // Create NSURL from string
        let nsurl: Id<AnyObject> = msg_send_id![class!(NSURL), URLWithString:&*ns_string];

        // Open the URL
        let success: bool = msg_send![&workspace, openURL:&*nsurl];

        if success {
            Ok(())
        } else {
            Err("NSWorkspace failed to open URL".into())
        }
    }
}

fn compare_versions(version_a: &Version, version_b: &Version) -> Ordering {
    let parse_versions = |Version(v): &Version| {
        v.split('.')
            .map(|s| s.parse::<u32>().unwrap())
            .collect::<Vec<u32>>()
    };

    let current_version = parse_versions(version_a);
    let min_version = parse_versions(version_b);

    for (current, min) in current_version.iter().zip(min_version.iter()) {
        use std::cmp::Ordering;
        match current.cmp(min) {
            Ordering::Less => return Ordering::Less,
            Ordering::Greater => return Ordering::Greater,
            Ordering::Equal => continue,
        }
    }

    Ordering::Equal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        let v = |v: &str| Version(v.to_string());
        let test_cases = [
            (v("1.2.2"), v("1.2.2"), Ordering::Equal),
            (v("1.2.3"), v("1.2.2"), Ordering::Greater),
            (v("1.3.2"), v("1.2.2"), Ordering::Greater),
            (v("2.0.0"), v("1.9.9"), Ordering::Greater),
            (v("1.2.1"), v("1.2.2"), Ordering::Less),
            (v("1.1.9"), v("1.2.0"), Ordering::Less),
            (v("1.9.9"), v("2.0.0"), Ordering::Less),
        ];
        for (a, b, expected) in test_cases {
            assert_eq!(compare_versions(&a, &b), expected);
        }
    }
}
