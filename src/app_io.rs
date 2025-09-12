use std::{
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

use crate::{
    app_actions::{
        AppIO, ConversationPart, HideMode, LLMBlockRequest, LLMPromptRequest, SettingsForAiRequests,
    },
    app_state::{InlineLLMResponseChunk, LLMBlockResponseChunk, MsgToApp},
    command::create_ai_keybindings_documentation,
    persistent_state::get_utc_timestamp,
};

use tokio_stream::StreamExt;

struct RegisteredGlobalHotkey {
    egui_shortcut: egui::KeyboardShortcut,
    system_hotkey: global_hotkey::hotkey::HotKey,
    handler: Box<dyn Fn() -> MsgToApp>,
}

pub struct RealAppIO {
    hotkeys_manager: GlobalHotKeyManager,
    registered_hotkeys: BTreeMap<u32, RegisteredGlobalHotkey>,
    egui_ctx: egui::Context,
    msg_queue: SyncSender<MsgToApp>,
    shelv_folder: PathBuf,
}

impl RealAppIO {
    pub fn new(
        hotkeys_manager: GlobalHotKeyManager,
        egui_ctx: egui::Context,
        msg_queue: SyncSender<MsgToApp>,
        shelv_folder: PathBuf,
    ) -> Self {
        Self {
            hotkeys_manager,
            registered_hotkeys: Default::default(),
            egui_ctx,
            msg_queue,
            shelv_folder,
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

    fn execute_llm_block(&self, question: LLMBlockRequest, cx: SettingsForAiRequests) {
        let egui_ctx = self.egui_ctx.clone();
        let sender = self.msg_queue.clone();

        // const MODEL_ANTHROPIC: &str = "claude-3-haiku-20240307";

        let LLMBlockRequest {
            conversation,
            output_code_block_address,
            note_id,
        } = question;

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

        let send = move |chunk: String| {
            sender
                .send(MsgToApp::LLMBlockResponseChunk(LLMBlockResponseChunk {
                    chunk: chunk.to_string(),
                    address: output_code_block_address.clone(),
                    note_id,
                }))
                .unwrap();

            egui_ctx.request_repaint();
        };

        tokio::spawn(async move {
            let mut chat_req = ChatRequest::new({
                // Note that some AI apis (lile anthropic) requires to be user -> assistant -> user -> ...
                // that means that markdown parts in between ai blocks either need to be "system" or user
                // in case of "user" they need to be merged with the quesion (ai block content)
                // for now pure string contatenation should work
                //  but potentially we might consider annotating somehow, like <md> </md> with system prompt
                let mut parts = Vec::new();
                let mut current_user_content: Option<String> = None;

                for part in conversation.parts {
                    match part {
                        // TODO potentially use some meta prompt to inject markdown content)
                        ConversationPart::Markdown(content)
                        | ConversationPart::Question(content) => match &mut current_user_content {
                            Some(user_content) => {
                                user_content.push_str("\n\n");
                                user_content.push_str(&content);
                            }
                            None => {
                                current_user_content = Some(content);
                            }
                        },
                        ConversationPart::Answer(content) => {
                            if let Some(user_content) = current_user_content.take() {
                                parts.push(ChatMessage::user(user_content));
                            }
                            parts.push(ChatMessage::assistant(content));
                        }
                    }
                }

                if let Some(current_user_content) = current_user_content {
                    parts.push(ChatMessage::user(current_user_content));
                }

                parts
            });

            let (model, system_prompt, use_shelv_propmpt, token) = llm_settings
                .map(|s| (s.model, s.system_prompt, s.use_shelv_system_prompt.unwrap_or(true), s.token))
                .unwrap_or_else(|| (SHELV_LLM_PROXY_MODEL.to_string(), None, true, None));

            if use_shelv_propmpt {
                chat_req
                    .messages
                    .insert(0, ChatMessage::system(shelv_system_prompt));
            }

            if let Some(system_prompt) = system_prompt {
                chat_req
                    .messages
                    .insert(0, ChatMessage::system(system_prompt));
            }

            let (service_target_resolver, auth_resolver) = if model == SHELV_LLM_PROXY_MODEL {
                prepare_shelv_providers()
            } else {
                prepare_general_providers(token.as_deref())
            };
            // println!("-----llm req: {chat_req:#?}");
            // -- Build the new client with this adapter_config
            let client = genai::Client::builder()
                .with_auth_resolver(auth_resolver)
                .with_service_target_resolver(service_target_resolver)
                .build();

            let chat_res = client
                .exec_chat_stream(model.as_str(), chat_req.clone(), None)
                .await;

            // println!(
            //     "-----llm resp: {:#?}",
            //     match &chat_res {
            //         Ok(resp) => "Ok".to_string(),
            //         Err(e) => "Err".to_string(),
            //     }
            // );

            match chat_res {
                Ok(mut stream) => {
                    while let Some(stream_event) = stream.stream.next().await {
                        match stream_event {
                            Ok(ChatStreamEvent::Chunk(StreamChunk { content })) => send(content),
                            Ok(_) => (),
                            Err(e) => {
                                send(format!("Error getting response: {:#?}", e));
                                break;
                            }
                        }
                    }
                }
                Err(err) => send(format!("{:#?}", err)),
            };
        });
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
        let send = move |chunk: Option<String>| {
            sender
                .send(MsgToApp::InlineLLMResponse {
                    response: (match chunk {
                        Some(chunk) => InlineLLMResponseChunk::Chunk(chunk),
                        None => InlineLLMResponseChunk::End,
                    }),
                    address: selection_location,
                })
                .unwrap();

            egui_ctx.request_repaint();
        };

        let (model, system_prompt, use_shelv_propmpt, token) = llm_settings
            .map(|s| (s.model, s.system_prompt, s.use_shelv_system_prompt.unwrap_or(true), s.token))
            .unwrap_or_else(|| (SHELV_LLM_PROXY_MODEL.to_string(), None, true, None));

        let chat_req = ChatRequest::new(Vec::from_iter(
            use_shelv_propmpt
                .then(|| ChatMessage::system(shelv_system_prompt))
                .into_iter()
                .chain(system_prompt.map(|sp| ChatMessage::system(sp)))
                .chain([
                    ChatMessage::system(include_str!("./prompts/inline-prompt-system-extra.md")),
                    ChatMessage::user({
                        let user_template = include_str!("./prompts/inline-prompt-user.md");

                        for pl in ["{{prompt}}", "{{before}}", "{{selection}}", "{{after}}"] {
                            assert!(
                                user_template.contains(pl),
                                "Template is missing required placeholder: {}",
                                pl
                            );
                        }

                        user_template
                            .replace("{{prompt}}", &prompt)
                            .replace("{{selection}}", &selection)
                            .replace("{{before}}", &before_selection)
                            .replace("{{after}}", &after_selection)
                    }),
                ]),
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

        std::fs::write(debug_file, contents).unwrap();

        tokio::spawn(async move {
            println!("-----llm inline req: {chat_req:#?}");

            let (service_target_resolver, auth_resolver) = if model == SHELV_LLM_PROXY_MODEL {
                prepare_shelv_providers()
            } else {
                prepare_general_providers(token.as_deref())
            };

            let client = genai::Client::builder()
                .with_auth_resolver(auth_resolver)
                .with_service_target_resolver(service_target_resolver)
                .build();

            let chat_res = client
                .exec_chat_stream(model.as_str(), chat_req, None)
                .await;

            match chat_res {
                Ok(mut stream) => {
                    while let Some(stream_event) = stream.stream.next().await {
                        match stream_event {
                            Ok(ChatStreamEvent::Chunk(StreamChunk { content })) => {
                                send(Some(content))
                            }
                            Ok(ChatStreamEvent::End(_)) => send(None),
                            Ok(ChatStreamEvent::Start) => (),
                            Ok(ChatStreamEvent::ReasoningChunk(_)) => (),
                            Err(e) => {
                                send(Some(format!("Error getting response: {:#?}", e)));
                                break;
                            }
                        }
                    }
                }
                Err(err) => send(Some(format!("{:#?}", err))),
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
}

fn create_shelv_claude_providers() -> (AuthData, ModelIden, Endpoint) {
    // Route to our proxy server running on localhost:8080
    let endpoint = Endpoint::from_static("http://localhost:8080/api/llm-claude/v1");
    let auth = AuthData::from_single("shelv-token");
    let model = ModelIden::new(AdapterKind::Anthropic, DEFAULT_REAL_LLM_MODEL);

    (auth, model, endpoint)
}

fn create_non_shelv_claude_providers(
    adapter_kind: AdapterKind,
    token: Option<&str>,
) -> Result<AuthData, genai::resolver::Error> {
    if let Some(token) = token {
        Ok(AuthData::from_single(token))
    } else {
        // Some models like Ollama may not require a token
        Ok(AuthData::from_single(""))
    }
}

fn prepare_shelv_providers() -> (ServiceTargetResolver, AuthResolver) {
    let service_target_resolver = ServiceTargetResolver::from_resolver_fn(
        |service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
            let (auth, model, endpoint) = create_shelv_claude_providers();

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
            let (auth, _, _) = create_shelv_claude_providers();
            Ok(Some(auth))
        },
    );

    (service_target_resolver, auth_resolver)
}

fn prepare_general_providers(token: Option<&str>) -> (ServiceTargetResolver, AuthResolver) {
    let service_target_resolver = ServiceTargetResolver::from_resolver_fn(
        |service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
            // For all other models, pass through unchanged
            Ok(service_target)
        },
    );

    let token_owned = token.map(|t| t.to_string());
    let auth_resolver = AuthResolver::from_resolver_fn(
        move |model_iden: genai::ModelIden| -> Result<Option<genai::resolver::AuthData>, genai::resolver::Error> {
            let genai::ModelIden {
                adapter_kind,
                model_name: _,
            } = model_iden;

            let auth = create_non_shelv_claude_providers(adapter_kind, token_owned.as_deref())?;
            Ok(Some(auth))
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
