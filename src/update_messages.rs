use fxhash::FxHasher;
use std::hash::{Hash, Hasher};

use crate::theme::AppTheme;
use crate::{
    app_actions::{AppAction, AppNotification, AppNotificationAction},
    command::EditorCommandOutput,
    theme::AppIcon,
    ui::notifications::NotificationId,
};

/// Generate a NotificationId from a string using fxhash
pub fn notification_id_from_string(s: &str) -> NotificationId {
    let mut hasher = FxHasher::default();
    s.hash(&mut hasher);
    NotificationId::new(hasher.finish())
}

/// Get update notification for a specific version
pub fn get_update_notification(version: &str, theme: &AppTheme) -> Option<AppNotification> {
    match version {
        // "1.3.9" => {
        "1.4.0" => {
            let notification_id = notification_id_from_string(&format!("update-{}", version));
            Some(AppNotification {
                id: notification_id,
                title: Some((
                    theme.colors.success_fg_color,
                    AppIcon::Check,
                    format!("Updated to {version}"),
                )),
                message: "Word Jump mode: navigate to any word with a couple of keystrokes"
                    .to_string(),

                action: Some(AppNotificationAction {
                    button_text: "Read changelog".to_string(),
                    icon: Some(AppIcon::HomeSite),
                    handler: Box::new(EditorCommandOutput::from_iter([
                        AppAction::CloseNotification(notification_id),
                        AppAction::defer(AppAction::OpenLink(
                            // "http://127.0.0.1:8080/updates/1_4_0".to_string(),
                            "https://shelv.app/updates/1_4_0".to_string(),
                        )),
                    ])),
                }),
            })
        }
        _ => None,
    }
}
