use std::{
    collections::BTreeMap,
    ffi::{c_void, CString},
    fs::{self, File},
    io::{self, Read},
    path::PathBuf,
    sync::mpsc::SyncSender,
};

use accessibility_sys::{
    AXUIElementCreateSystemWide, AXUIElementGetTypeID, __AXUIElement, kAXTitleAttribute,
    kAXURLAttribute,
};
use eframe::egui;
use genai::{
    adapter::AdapterKind,
    chat::{ChatMessage, ChatRequest, ChatStreamEvent, StreamChunk},
    resolver::AuthResolver,
};
use global_hotkey::GlobalHotKeyManager;

use crate::{
    app_actions::{AppIO, ConversationPart, LLMBlockRequest, LLMPromptRequest},
    app_state::{InlineLLMResponseChunk, LLMBlockResponseChunk, MsgToApp},
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

    fn execute_llm_block(&self, question: LLMBlockRequest) {
        let egui_ctx = self.egui_ctx.clone();
        let sender = self.msg_queue.clone();

        // const MODEL_ANTHROPIC: &str = "claude-3-haiku-20240307";

        let LLMBlockRequest {
            conversation,
            output_code_block_address,
            note_id,
            system_prompt,
            model,
        } = question;

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

            if let Some(system_prompt) = system_prompt {
                chat_req
                    .messages
                    .insert(0, ChatMessage::system(system_prompt));
            }

            let auth_resolver = prepare_auth_resolver();
            // println!("-----llm req: {chat_req:#?}");
            // -- Build the new client with this adapter_config
            let client = genai::Client::builder()
                .with_auth_resolver(auth_resolver)
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

    fn execute_llm_prompt(&self, quesion: LLMPromptRequest) {
        let egui_ctx = self.egui_ctx.clone();
        let sender = self.msg_queue.clone();

        let LLMPromptRequest {
            prompt,
            selection,
            model,
            system_prompt,
            selection_location,
            before_selection,
            after_selection,
        } = quesion;
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

        tokio::spawn(async move {
            let chat_req = ChatRequest::new(vec![
                ChatMessage::system(system_prompt.unwrap_or_default()),
                ChatMessage::system(
                    [
                        "selection is  <selection>{selection_body}</selection>",
                        "prompt is marked as <prompt>{prompt_body}</prompt>",
                        "content above selection marked as <before></before>",
                        "content after selection marked as <after></after>",
                        "answer the prompt quesion targeting <selection>, the asnwer will replace <selection> block",
                        "using the context provided in <before> and <after>",
                        "do not include <selection></selection> into response",
                        "AVOID any extra comments or introductionary content, output ONLY the result",
                    ]
                    .join("\n"),
                ),
                ChatMessage::user(format!(
                    "<before>{before_selection}</before>
                    <selection>{selection}</selection>
                    <after>{after_selection}</after>
                    <prompt>{prompt}</prompt>"
                )),
            ]);

            println!("-----llm inline req: {chat_req:#?}");

            let auth_resolver = prepare_auth_resolver();

            let client = genai::Client::builder()
                .with_auth_resolver(auth_resolver)
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
}

fn prepare_auth_resolver() -> AuthResolver {
    let auth_resolver = AuthResolver::from_resolver_fn(
        move |model_iden: genai::ModelIden| -> Result<Option<genai::resolver::AuthData>, genai::resolver::Error> {
            let genai::ModelIden {
                adapter_kind,
                model_name,
            } = model_iden;

            if adapter_kind != AdapterKind::Anthropic {
                return Err(genai::resolver::Error::Custom("Currently we only support Anthropic models".to_string()));
            }

            // YES it is OK to hardcode it here, it is heavily rate limited AND unique for this specific usage
            let key = "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";

            Ok(Some(genai::resolver::AuthData::from_single(key)))
        },
    );

    auth_resolver
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
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send, msg_send_id};
    unsafe {
        let app: Id<AnyObject> = msg_send_id![class!(NSApplication), sharedApplication];
        let arg = app.as_ref();
        let _: () = msg_send![&app, hide:arg];
    }
}

// Swift code:
// extension AXUIElement {
//   static var focusedElement: AXUIElement? {
//     systemWide.element(for: kAXFocusedUIElementAttribute)
//   }

//   var selectedText: String? {
//     rawValue(for: kAXSelectedTextAttribute) as? String
//   }

//   private static var systemWide = AXUIElementCreateSystemWide()

//   private func element(for attribute: String) -> AXUIElement? {
//     guard let rawValue = rawValue(for: attribute), CFGetTypeID(rawValue) == AXUIElementGetTypeID() else { return nil }
//     return (rawValue as! AXUIElement)
//   }

//   private func rawValue(for attribute: String) -> AnyObject? {
//     var rawValue: AnyObject?
//     let error = AXUIElementCopyAttributeValue(self, attribute as CFString, &rawValue)
//     return error == .success ? rawValue : nil
//   }
// }
use accessibility_sys::{
    kAXFocusedUIElementAttribute, kAXSelectedTextAttribute, AXUIElementCopyAttributeValue,
};
use core_foundation::{
    array::{CFArray, CFArrayGetValueAtIndex, CFArrayRef}, base::{
        CFCopyTypeIDDescription, CFGetTypeID, CFIndex, CFRange, CFRelease, CFTypeRef, FromVoid, TCFType, TCFTypeRef
    }, string::{CFString, CFStringGetTypeID, CFStringRef}, url::{CFURLGetTypeID, CFURL}
};
use std::ptr;

#[derive(Debug)]
pub struct CoppiedTextContext {
    pub selected_text: Option<String>,
    pub url: Option<String>,
    pub document: Option<String>,
    pub window_title: Option<String>,
    pub application_title: Option<String>,
}

pub fn get_focused_element_text() -> Option<CoppiedTextContext> {
    use accessibility_sys::*;

    fn get_element(system_wide: *mut __AXUIElement, attribute: &str) -> Option<*mut __AXUIElement> {
        unsafe {
            let mut raw_value: CFTypeRef = ptr::null();
            let cf_string = CFString::new(attribute);
            let result = AXUIElementCopyAttributeValue(
                system_wide,
                cf_string.as_concrete_TypeRef(),
                &mut raw_value,
            );

            if result != 0
                || raw_value.is_null()
                || CFGetTypeID(raw_value) != AXUIElementGetTypeID()
            {
                return None;
            }

            Some(raw_value as *mut __AXUIElement)
        }
    }

    fn get_attribute(element: *mut __AXUIElement, attribute: &str) -> Option<String> {
        unsafe {
            let mut raw_value: CFTypeRef = ptr::null();
            let cf_string = CFString::new(attribute);
            let result = AXUIElementCopyAttributeValue(
                element,
                cf_string.as_concrete_TypeRef(),
                &mut raw_value,
            );

            if result != 0 || raw_value.is_null() || CFGetTypeID(raw_value) != CFStringGetTypeID() {
                return None;
            }

            let rust_string = CFString::from_void(raw_value).to_string();

            CFRelease(raw_value);

            Some(rust_string)
        }
    }

    fn get_url_attribute(element: *mut __AXUIElement, attribute: &str) -> Option<String> {
        unsafe {
            let mut raw_value: CFTypeRef = ptr::null();
            let cf_string = CFString::new(attribute);
            let result = AXUIElementCopyAttributeValue(
                element,
                cf_string.as_concrete_TypeRef(),
                &mut raw_value,
            );

            if result != 0 || raw_value.is_null() || CFGetTypeID(raw_value) != CFURLGetTypeID() {
                return None;
            }

            let rust_string = CFURL::from_void(raw_value).get_string().to_string();

            CFRelease(raw_value);

            Some(rust_string)
        }
    }

    fn get_children_elements(element: *mut __AXUIElement) -> Option<Vec<*mut __AXUIElement>> {
        unsafe {
            let mut raw_value: CFTypeRef = ptr::null();
            let cf_string = CFString::new(kAXChildrenAttribute);
            let result = AXUIElementCopyAttributeValue(
                element,
                cf_string.as_concrete_TypeRef(),
                &mut raw_value,
            );

            let mut cfindex: CFIndex = 0;
            let cfindex_ptr: *mut CFIndex = &mut cfindex as *mut _ as *mut CFIndex;
            let children_count = AXUIElementGetAttributeValueCount(element, cf_string.as_concrete_TypeRef(), cfindex_ptr);

            if result != 0 || raw_value.is_null() {
                return None;
            }

            let children = CFArray::<CFTypeRef>::from_void(raw_value);

            let mut elements = Vec::new();
            for i in 0..children_count {
                let child = CFArrayGetValueAtIndex(raw_value as CFArrayRef, CFIndex::from(i as isize));
                if CFGetTypeID(child) == AXUIElementGetTypeID() {
                    elements.push(child as *mut __AXUIElement);
                }
            }

            CFRelease(raw_value);

            Some(elements)
        }
    }

    fn get_range_attribute(element: *mut __AXUIElement, attribute: &str) -> Option<CFRange> {
        unsafe {
            let mut raw_value: CFTypeRef = ptr::null();
            let cf_string = CFString::new(attribute);
            let result = AXUIElementCopyAttributeValue(
                element,
                cf_string.as_concrete_TypeRef(),
                &mut raw_value,
            );

            if result != 0 || raw_value.is_null() {
                println!("Error getting range attribute");
                return None;
            }

            println!("Found Range");


            let mut selectedCFRange: CFRange = CFRange {
                location: 0,
                length: 0,
            };
            let cfrange_ptr: *mut c_void = &mut selectedCFRange as *mut _ as *mut c_void;
            let selectedRangeValue: AXValueRef = raw_value as AXValueRef;
            AXValueGetValue(selectedRangeValue, kAXValueTypeCFRange,cfrange_ptr );

            CFRelease(raw_value);

            Some(selectedCFRange)
        }
    }

    pub fn cfstring_to_string(r: CFStringRef) -> String {
        format!("{}", unsafe { &*CFString::from_void(r.as_void_ptr()) })
    }

    fn scan_hieararchy_for_attribute(
        element: *mut __AXUIElement,
        attribute: &str,
    ) -> Option<String> {
        let mut current_element = element;

        loop {
            println!(
                "Scanning {} {} {}",
                get_attribute(current_element, kAXRoleAttribute).unwrap_or_default(),
                get_attribute(current_element, kAXTitleAttribute).unwrap_or_default(),
                get_attribute(current_element, kAXDescriptionAttribute).unwrap_or_default()
            );

            let value = get_url_attribute(current_element, attribute).or(get_attribute(current_element, attribute));
            
            if value.is_some() {
                println!("Found Attribute!");

                return value;
            }

            let Some(parent_element) = get_element(current_element, kAXParentAttribute) else {
                println!("Reached the top of the hierarchy");
                return None;
            };

            current_element = parent_element;
        }

        
    }

    unsafe {
        // 1. Get the system-wide AXUIElement
        let system_wide_element = AXUIElementCreateSystemWide();

        // 2. Get the focused UI element
        let focused_element = get_element(system_wide_element, kAXFocusedUIElementAttribute)?;

        // 3. Get the selected text
        let selected_text = get_attribute(focused_element, kAXSelectedTextAttribute);

        let children_elements = get_children_elements(focused_element);

        println!("Found {:?} children", children_elements.as_ref().map(|c| c.len()).unwrap_or(0));
        if let Some(children_elements) = children_elements {
            for child in children_elements {
                let value = get_attribute(child, kAXSelectedTextAttribute);
                if value.is_some() {
                    println!("Child has selected text!");
                }
            }
        }

        // 4. Additional attributes
        let window_element = get_element(focused_element, kAXWindowAttribute);

        let window_title = get_element(focused_element, kAXWindowAttribute)
            .map(|window| get_attribute(window, kAXTitleAttribute))
            .flatten();
        let application_title = window_element
            .map(|window_element| {
                get_element(window_element, kAXParentAttribute).map(|application_element| {
                    get_attribute(application_element, kAXTitleAttribute)
                }).flatten()
            })
            .flatten();
        
        let url = scan_hieararchy_for_attribute(focused_element, kAXURLAttribute);
        let document = scan_hieararchy_for_attribute(focused_element, kAXDocumentAttribute);

        let selected_range = get_range_attribute(focused_element, kAXSelectedTextRangeAttribute);
        println!("Selected Range: {:?}", selected_range);

        Some(CoppiedTextContext {
            selected_text,
            url,
            document,
            window_title,
            application_title
        })
    }
}

// CFRange selectedCFRange;
// AXValueRef selectedRangeValue = NULL;

// // Access selected range attribute from focused element
// AXError selectedRangeError = AXUIElementCopyAttributeValue(focussedElement, kAXSelectedTextRangeAttribute, (CFTypeRef *)&selectedRangeValue);
// if (selectedRangeError == kAXErrorSuccess)

// {

// NSLog(@”\nSelected Range: %@”,selectedRangeValue);

// //Selected Range is retrieved successfully, then get the range into CFRange type object

// AXValueGetValue(selectedRangeValue, kAXValueCFRangeType, &selectedCFRange);
// }
// else
// {
// NSLog(@”Error while retrieving selected range”);
// }

// // The length and location of the selected text will be selectedCFRange.length and selectedCFRange.location

// NSLog(@”\nLength: %ld, Location: %ld”,selectedCFRange.length, selectedCFRange.location);

// CGRect selectedRect;

// AXValueRef selectedBounds = NULL;
// // Get the selected bounds value from the selected range

// AXError selectedBoundsError = AXUIElementCopyParameterizedAttributeValue(focussedElement, kAXBoundsForRangeParameterizedAttribute, selectedRangeValue, (CFTypeRef *)&selectedBounds);
// CFRelease(selectedRangeValue);

// if (selectedBoundsError == kAXErrorSuccess)

// {

// AXValueGetValue(selectedBounds, kAXValueCGRectType, &selectedRect);
// NSLog(@”Selection bounds: %@”, NSStringFromRect(NSRectFromCGRect(selectedRect)));   // Selection Rect retrieved
// }

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
