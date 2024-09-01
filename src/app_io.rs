use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, Read},
    path::PathBuf,
    sync::mpsc::SyncSender,
};

use eframe::egui;
use genai::{
    chat::{ChatMessage, ChatRequest, ChatStreamEvent, StreamChunk},
    resolver::AuthResolver,
};
use global_hotkey::GlobalHotKeyManager;

use crate::{
    app_actions::{AppIO, ConversationPart, LLMRequest},
    app_state::{LLMResponseChunk, MsgToApp},
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

impl AppIO for RealAppIO {
    fn hide_app(&self) {
        hide_app_on_macos();
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

    fn ask_llm(&self, question: LLMRequest) {
        let egui_ctx = self.egui_ctx.clone();
        let sender = self.msg_queue.clone();

        // const MODEL_ANTHROPIC: &str = "claude-3-haiku-20240307";

        let LLMRequest {
            conversation,
            output_code_block_address,
            note_id,
            system_prompt,
            model,
        } = question;

        let send = move |chunk: String| {
            sender
                .send(MsgToApp::LLMResponseChunk(LLMResponseChunk {
                    chunk: chunk.to_string(),
                    address: output_code_block_address.clone(),
                    note_id,
                }))
                .unwrap();

            egui_ctx.request_repaint();
        };

        let key_path = self.shelv_folder.join(".llm_key");

        tokio::spawn(async move {
            let mut chat_req = ChatRequest::new(
                conversation
                    .parts
                    .into_iter()
                    .map(|p| match p {
                        ConversationPart::Markdown(content) => ChatMessage::system(content),
                        ConversationPart::Question(content) => ChatMessage::user(content),
                        ConversationPart::Answer(content) => ChatMessage::assistant(content),
                    })
                    .collect(), //[ChatMessage::system(context), ChatMessage::user(question)].into(),
            );

            if let Some(system_prompt) = system_prompt {
                chat_req
                    .messages
                    .insert(0, ChatMessage::system(system_prompt));
            }

            let auth_resolver = AuthResolver::from_resolver_fn(
                move |model_iden: genai::ModelIden| -> Result<Option<genai::resolver::AuthData>, genai::resolver::Error> {
                    let genai::ModelIden {
                        adapter_kind,
                        model_name,
                    } = model_iden;


                    let key = fs::read_to_string(&key_path).map_err(|e| {
                        genai::resolver::Error::Custom(format!("Failed to read .llm_key: {}, here: {:?}", e, key_path))
                    })?;

                    let key = key.trim();
                    if key.is_empty() {
                        return Err(genai::resolver::Error::ApiKeyEnvNotFound {
                            env_name: ".llm_key".to_string(),
                        });
                    }

                    Ok(Some(genai::resolver::AuthData::from_single(key)))
                },
            );

            // -- Build the new client with this adapter_config
            let client = genai::Client::builder()
                .with_auth_resolver(auth_resolver)
                .build();

            let chat_res = client
                .exec_chat_stream(model.as_str(), chat_req.clone(), None)
                .await;

            match chat_res {
                Ok(mut stream) => {
                    while let Some(Ok(stream_event)) = stream.stream.next().await {
                        match stream_event {
                            ChatStreamEvent::Chunk(StreamChunk { content }) => send(content),

                            _ => (),
                        }
                    }
                }
                Err(err) => send(err.to_string()),
            };
        });
    }
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

fn hide_app_on_macos() {
    // https://developer.apple.com/documentation/appkit/nsapplication/1428733-hide
    use objc2::rc::Id;
    use objc2::{class, msg_send, msg_send_id, runtime::Object};
    unsafe {
        let app: Id<Object> = msg_send_id![class!(NSApplication), sharedApplication];
        let arg = app.as_ref();
        let _: () = msg_send![&app, hide:arg];
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
