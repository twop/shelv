//! Receives requests from the OS and sends them as `OsInboxMsg` to a dedicated channel.
//!
//! Currently: files the OS asks us to open (`open -a Shelv file.md`, Finder "Open With").
//! On macOS they arrive as a `kCoreEventClass/kAEOpenDocuments` Apple Event on the main thread.
//! In a sandboxed build the event also carries the access grant for those files, which is why
//! this is the way to open external files from scripts.
//!
//! On cold start the event is delivered *before* `MyApp::new` runs (between will/did finish launching),
//! the channel buffers it until the app drains it.
//!
//! Usage: `OsInbox::install(sender)` in `main` before the event loop starts, then hand the inbox
//! (keeps the handler registered) and the receiver to the app, which calls `set_egui_ctx(..)`.

use std::sync::mpsc::SyncSender;

#[cfg(target_os = "macos")]
use crate::app_state::MsgToApp;

pub struct OsInbox {
    #[cfg(target_os = "macos")]
    handler: objc2::rc::Retained<macos::ShelvOsEventHandler>,
}

#[cfg(target_os = "macos")]
impl OsInbox {
    /// Must be called on the main thread before the event loop starts,
    /// otherwise the files that launched the app are lost.
    pub fn install(sender: SyncSender<MsgToApp>) -> Self {
        let mtm = objc2_foundation::MainThreadMarker::new()
            .expect("OsInbox::install must be called on the main thread");
        Self {
            handler: macos::ShelvOsEventHandler::install(mtm, sender),
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for OsInbox {
    fn drop(&mut self) {
        // AppKit doesn't retain the handler, so it must not stay registered after it is gone
        self.handler.uninstall();
    }
}

#[cfg(not(target_os = "macos"))]
impl OsInbox {
    pub fn install(_sender: SyncSender<OsInboxMsg>) -> Self {
        Self {}
    }

    pub fn set_egui_ctx(&self, _ctx: egui::Context) {}
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::CStr;
    use std::path::PathBuf;
    use std::sync::mpsc::SyncSender;

    use objc2::rc::{Id, Retained};
    use objc2::runtime::AnyObject;
    use objc2::{
        ClassType, DeclaredClass, class, declare_class, msg_send, msg_send_id, mutability, sel,
    };
    use objc2_foundation::{MainThreadMarker, NSObject};

    use crate::app_state::MsgToApp;

    const fn four_char_code(code: &[u8; 4]) -> u32 {
        u32::from_be_bytes(*code)
    }

    const K_CORE_EVENT_CLASS: u32 = four_char_code(b"aevt");
    const K_AE_OPEN_DOCUMENTS: u32 = four_char_code(b"odoc");
    const KEY_DIRECT_OBJECT: u32 = four_char_code(b"----");

    const WILL_FINISH_LAUNCHING: &CStr = c"NSApplicationWillFinishLaunchingNotification";

    pub struct Ivars {
        sender: SyncSender<MsgToApp>,
    }

    declare_class!(
        pub struct ShelvOsEventHandler;

        // SAFETY:
        // - The superclass NSObject does not have any subclassing requirements.
        // - `ShelvOsEventHandler` does not implement `Drop`.
        unsafe impl ClassType for ShelvOsEventHandler {
            type Super = NSObject;
            // Apple Events and notifications are delivered on the main thread
            type Mutability = mutability::MainThreadOnly;
            const NAME: &'static str = "ShelvOsEventHandler";
        }

        impl DeclaredClass for ShelvOsEventHandler {
            type Ivars = Ivars;
        }

        unsafe impl ShelvOsEventHandler {
            #[method(handleOpenDocumentsEvent:withReplyEvent:)]
            fn handle_open_documents(&self, event: &AnyObject, _reply: &AnyObject) {
                let paths = unsafe { extract_file_paths(event) };
                println!("[os_inbox] open documents event, paths={paths:?}");
                if !paths.is_empty() {
                    self.send(MsgToApp::OpenFiles(paths));
                }
            }

            #[method(applicationWillFinishLaunching:)]
            fn will_finish_launching(&self, _notification: &AnyObject) {
                self.register_apple_event_handlers();
            }
        }
    );

    impl ShelvOsEventHandler {
        /// Handlers are registered on `NSApplicationWillFinishLaunching`: that is Apple's documented
        /// place for it, it runs after AppKit installs its own handlers and before the event that
        /// launched the app is delivered.
        pub fn install(mtm: MainThreadMarker, sender: SyncSender<MsgToApp>) -> Retained<Self> {
            let this = mtm.alloc::<Self>().set_ivars(Ivars { sender });
            let this: Retained<Self> = unsafe { msg_send_id![super(this), init] };
            this.observe_will_finish_launching();
            this
        }

        pub fn uninstall(&self) {
            unsafe {
                let center: Id<AnyObject> =
                    msg_send_id![class!(NSNotificationCenter), defaultCenter];
                let _: () = msg_send![&center, removeObserver: self];

                let manager: Id<AnyObject> =
                    msg_send_id![class!(NSAppleEventManager), sharedAppleEventManager];
                let _: () = msg_send![
                    &manager,
                    removeEventHandlerForEventClass: K_CORE_EVENT_CLASS,
                    andEventID: K_AE_OPEN_DOCUMENTS
                ];
            }
        }

        fn send(&self, msg: MsgToApp) {
            let ivars = self.ivars();
            // Never block: Apple Events are delivered on the main thread, which is also the queue consumer,
            // so a blocking `send` on a full queue would deadlock the app.
            if let Err(err) = ivars.sender.try_send(msg) {
                println!("[os_inbox] dropped request, err={err:?}");
            }
        }

        fn register_apple_event_handlers(&self) {
            // replaces AppKit's own 'odoc' handler (which would forward to the app delegate, owned by winit)
            unsafe {
                let manager: Id<AnyObject> =
                    msg_send_id![class!(NSAppleEventManager), sharedAppleEventManager];
                let _: () = msg_send![
                    &manager,
                    setEventHandler: self,
                    andSelector: sel!(handleOpenDocumentsEvent:withReplyEvent:),
                    forEventClass: K_CORE_EVENT_CLASS,
                    andEventID: K_AE_OPEN_DOCUMENTS
                ];
            }
        }

        fn observe_will_finish_launching(&self) {
            unsafe {
                let center: Id<AnyObject> =
                    msg_send_id![class!(NSNotificationCenter), defaultCenter];
                let name: Id<AnyObject> = msg_send_id![
                    class!(NSString),
                    stringWithUTF8String: WILL_FINISH_LAUNCHING.as_ptr()
                ];
                let _: () = msg_send![
                    &center,
                    addObserver: self,
                    selector: sel!(applicationWillFinishLaunching:),
                    name: &*name,
                    object: std::ptr::null::<AnyObject>()
                ];
            }
        }
    }

    /// The direct object of 'odoc' is a list of file descriptors (usually `typeFileURL`).
    unsafe fn extract_file_paths(event: &AnyObject) -> Vec<PathBuf> {
        unsafe {
            let Some(list): Option<Id<AnyObject>> =
                msg_send_id![event, paramDescriptorForKeyword: KEY_DIRECT_OBJECT]
            else {
                return Vec::new();
            };

            let count: isize = msg_send![&list, numberOfItems];
            let descriptors: Vec<Id<AnyObject>> = if count <= 0 {
                // not a list, a single file descriptor
                vec![list]
            } else {
                // Apple Event lists are 1-based
                (1..=count)
                    .filter_map(|i| msg_send_id![&list, descriptorAtIndex: i])
                    .collect()
            };

            descriptors
                .iter()
                .filter_map(|descriptor| {
                    let url: Option<Id<AnyObject>> = msg_send_id![&**descriptor, fileURLValue];
                    let path: Option<Id<AnyObject>> = msg_send_id![&*url?, path];
                    ns_string_to_string(&*path?)
                })
                .map(PathBuf::from)
                .collect()
        }
    }

    unsafe fn ns_string_to_string(ns_string: &AnyObject) -> Option<String> {
        unsafe {
            let utf8: *const std::ffi::c_char = msg_send![ns_string, UTF8String];
            (!utf8.is_null()).then(|| CStr::from_ptr(utf8).to_string_lossy().into_owned())
        }
    }
}
