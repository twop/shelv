use eframe::{
    egui::{self, Align2, Area, Context, Frame, Id, Shadow, Stroke, WidgetText},
    emath::{self, TSTransform},
    epaint::{Color32, vec2},
};

use egui_taffy::{
    Tui, TuiBuilderLogic,
    taffy::{AlignItems, JustifyContent},
    tui,
};
use smallvec::SmallVec;

use crate::{
    taffy_styles::{StyleBuilder, flex_column, flex_row},
    theme::{AppIcon, AppTheme},
    ui_components::IconButton,
};

// NOTE that implementation is inspired by egui-notify
// but it lacked custom UI and custom UI output
// hence, my own implementation
//
/// Unique identifier for notifications
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NotificationId(u64);

impl NotificationId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// State of a notification during its lifecycle
#[derive(Debug, Clone, PartialEq)]
pub enum NotificationState {
    /// Appear animation has not started yet
    Fresh,
    /// Starting to appear
    Appearing,
    /// Fully visible and stable
    Visible,
    // /// Starting to fade out
    Disappearing,
    /// Completely faded, ready for removal
    Disappeared,
}

/// Internal notification data with animation state
#[derive(Debug, Clone)]
struct NotificationData<T> {
    id: NotificationId,
    notification: T,
    state: NotificationState,
    // None -> non expiring
    duration: Option<f32>,
}

impl<T> NotificationData<T> {
    pub fn new(id: NotificationId, notification: T) -> Self {
        Self {
            id,
            notification,
            state: NotificationState::Fresh,
            duration: None,
        }
    }

    pub fn update(&mut self, _ctx: &Context) {
        // Animation is now handled by egui_animate in the show method
        // State transitions will be controlled there
    }

    pub fn should_be_visible(&self) -> bool {
        match self.state {
            NotificationState::Disappearing
            | NotificationState::Disappeared
            // note that fresh should be invisible because we start from it
            | NotificationState::Fresh => false,
            NotificationState::Appearing | NotificationState::Visible => true,
        }
    }

    pub fn transition_animation_state(&self, anim_value: f32) -> NotificationState {
        use NotificationState as N;
        match self.state {
            N::Fresh => N::Appearing,
            N::Appearing if anim_value == 1.0 => N::Visible,
            N::Appearing => N::Appearing,
            N::Visible => N::Visible,
            N::Disappearing if anim_value == 0.0 => N::Disappeared,
            N::Disappearing => N::Disappearing,
            N::Disappeared => N::Disappeared,
        }
    }
}

/// Configuration for notification animations and appearance
pub struct NotificationsConfig {
    /// Width of notification windows
    pub width: f32,
    /// Distance the notification will travel to appear/disappear
    pub slide_distance: f32,
    /// Margin from screen edges
    pub margin: f32,
    /// Spacing between stacked notifications
    pub spacing: f32,
    /// Animation duration for slide + fade
    pub animation_duration: f32,
}

impl NotificationsConfig {
    fn new() -> Self {
        Self {
            width: 300.0,
            slide_distance: 100.0,
            margin: 16.0,
            spacing: 8.0,
            animation_duration: 0.3,
        }
    }
}

pub trait NotificationItem {
    type Output: Send + Sync + Clone + 'static;
    fn title(&self, app_theme: &AppTheme) -> WidgetText;
    fn render(&self, tui: &Tui, app_theme: &AppTheme) -> Option<Self::Output>;
}

/// Main notifications system that tracks and renders notifications
/// T: The logical notification type (e.g., AppNotification)  
pub struct Notifications<T> {
    notifications: Vec<NotificationData<T>>,
    config: NotificationsConfig,
}

impl<T: Clone> Notifications<T> {
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
            config: NotificationsConfig::new(),
        }
    }

    pub fn with_config(config: NotificationsConfig) -> Self {
        Self {
            notifications: Vec::new(),
            config,
        }
    }

    /// Add a new notification or update existing one
    pub fn add(&mut self, id: NotificationId, notification: &T) {
        if self.notifications.iter().find(|n| n.id == id).is_none() {
            self.notifications
                .push(NotificationData::new(id, notification.clone()));
        }
    }

    /// Manually dismiss a notification
    pub fn dismiss(&mut self, id: NotificationId) {
        if let Some(data) = self.notifications.iter_mut().find(|n| n.id == id) {
            data.state = NotificationState::Disappeared;
        }
    }

    // /// Update all notifications (call this each frame)
    // pub fn update(&mut self, ctx: &Context) {
    //     // Update all notification states
    //     for data in self.notifications.iter_mut() {
    //         data.update(ctx);
    //     }

    //     self.notifications
    //         .retain(|data| data.state != NotificationState::Disappeared);
    // }

    /// Render all notifications as a stack of frames
    pub fn show(&mut self, ctx: &Context, theme: &AppTheme) -> SmallVec<[T::Output; 4]>
    where
        T: NotificationItem,
    {
        let mut outputs = SmallVec::new();

        if self.notifications.is_empty() {
            return outputs;
        }

        // Calculate position for notifications (stack from bottom up)
        let mut y_offset = 0.0;

        // Process notifications in reverse order to handle state changes
        for data in self.notifications.iter_mut() {
            let notification_id = Id::new("notification").with(data.id.0);
            let should_show = data.state == NotificationState::Visible;

            let area_response = Area::new(notification_id)
                .anchor(
                    Align2::RIGHT_BOTTOM,
                    vec2(-self.config.margin, -(self.config.margin + y_offset)),
                )
                .movable(false)
                .interactable(true)
                .show(ctx, |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    ui.set_max_width(self.config.width);

                    let vis_anim_value = ctx.animate_bool_with_time_and_easing(
                        notification_id.with("animation"),
                        data.should_be_visible(),
                        self.config.animation_duration,
                        emath::easing::cubic_in,
                    );
                    ui.set_opacity(vis_anim_value);

                    let slide_distance = match data.state {
                        NotificationState::Fresh
                        | NotificationState::Appearing
                        | NotificationState::Visible => -self.config.slide_distance,
                        NotificationState::Disappearing | NotificationState::Disappeared => {
                            self.config.slide_distance
                        }
                    };

                    ctx.set_transform_layer(
                        ui.layer_id(),
                        TSTransform::from_translation(
                            ((1.0 - vis_anim_value) * slide_distance, 0.0).into(),
                        ),
                    );
                    data.state = data.transition_animation_state(vis_anim_value);

                    let frame = Frame::window(ui.style()).inner_margin(8.0);

                    frame.show(ui, |ui| {
                        // Create taffy layout for title + close button + content
                        let tui_id = notification_id.with("content");

                        // Use taffy for the layout
                        tui(ui, tui_id)
                            .style(
                                flex_column()
                                    .width(self.config.width - 32.0) // Account for frame margins
                                    .auto_height()
                                    .gap(4.0),
                            )
                            .show(|t| {
                                t.style(
                                    flex_row()
                                        .justify_content(JustifyContent::SpaceBetween)
                                        .align_items(AlignItems::Center)
                                        .width(self.config.width - 48.0) // Account for margins
                                        .auto_height(),
                                )
                                .add(|t| {
                                    // Title
                                    let title_text = data.notification.title(theme);
                                    t.ui_add(egui::Label::new(title_text));

                                    if t.ui_add(
                                        IconButton::new(AppIcon::Close, theme)
                                            .tooltip("Disimiss", None),
                                    )
                                    .clicked()
                                    {
                                        data.state = NotificationState::Disappearing;
                                    }
                                });

                                t.style(
                                    flex_column()
                                        .width(self.config.width - 48.0) // Account for margins
                                        .auto_height(),
                                )
                                .add(|t| {
                                    if let Some(output) = data.notification.render(t, theme) {
                                        // // Store output for later collection
                                        // ui.ctx().data_mut(|d| {
                                        //     d.insert_temp(
                                        //         notification_id.with("output"),
                                        //         output,
                                        //     )
                                        // });
                                    }
                                });
                            });
                    });
                });

            if should_show {
                y_offset += self.config.spacing + area_response.response.rect.height();
            }
        }

        self.notifications
            .retain(|data| data.state != NotificationState::Disappeared);

        outputs
    }

    /// Get the configuration for positioning and animation settings
    pub fn config(&self) -> &NotificationsConfig {
        &self.config
    }

    /// Get number of active notifications
    pub fn count(&self) -> usize {
        self.notifications.len()
    }

    /// Check if there are any notifications
    pub fn is_empty(&self) -> bool {
        self.notifications.is_empty()
    }

    /// Clear all notifications immediately
    pub fn clear(&mut self) {
        self.notifications.clear();
    }
}
