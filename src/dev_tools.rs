use std::collections::VecDeque;

use chrono::{DateTime, Local, Utc};
use eframe::egui::{
    self, Align, FontFamily, Id, InputState, Layout, Margin, Panel, Response, RichText, ScrollArea,
    Ui, containers::Frame,
};
use egui_extras::TableBuilder;

use crate::{
    app_actions::{AppAction, AppNotification, AppNotificationAction},
    command::{AppFocus, AppFocusState, EditorCommandOutput, UiState},
    theme::{AppIcon, AppTheme},
    ui::{NotificationId, Notifications},
};

const MAX_ACTION_HISTORY: usize = 1000;
const MAX_INPUT_EVENT_HISTORY: usize = 100;

#[derive(Debug, Clone, Copy)]
pub enum ActionPhase {
    PreRender,
    PostRender,
}

#[derive(Debug, Clone)]
struct ActionLogEntry {
    action: AppAction,
    action_debug: String,
    action_body: String,
    depth: usize,
    phase: ActionPhase,
    timestamp: DateTime<Local>,
}

#[derive(Debug, Clone)]
struct InputEventEntry {
    event: String,
    count: usize,
    timestamp: DateTime<Local>,
}

pub struct DevToolsState {
    pub show_dev_tools: bool,
    action_history: VecDeque<ActionLogEntry>,
    input_event_history: VecDeque<InputEventEntry>,
    show_pointer_events: bool,
    expanded: ExpandedPanes,
}

impl Default for DevToolsState {
    fn default() -> Self {
        Self {
            show_dev_tools: false,
            action_history: VecDeque::with_capacity(MAX_ACTION_HISTORY),
            input_event_history: VecDeque::with_capacity(MAX_INPUT_EVENT_HISTORY),
            show_pointer_events: false,
            expanded: ExpandedPanes::default(),
        }
    }
}

impl DevToolsState {
    pub fn log_action(&mut self, action: AppAction, depth: usize, phase: ActionPhase) {
        if self.action_history.len() >= MAX_ACTION_HISTORY {
            self.action_history.pop_front();
        }

        self.action_history.push_back(ActionLogEntry {
            action: action.clone(),
            action_debug: format!("{:?}", action),
            action_body: format!("{:#?}", action),
            depth,
            phase,
            timestamp: Local::now(),
        });
    }

    pub fn dump_input_events(&mut self, input: &InputState) {
        for event in &input.raw.events {
            if !self.show_pointer_events
                && matches!(
                    event,
                    egui::Event::PointerMoved { .. }
                        | egui::Event::MouseMoved { .. }
                        | egui::Event::Touch { .. }
                        | egui::Event::MouseWheel { .. }
                )
            {
                continue;
            }

            let event_summary = event_summary(event);
            if let Some(last_entry) = self.input_event_history.back_mut() {
                if last_entry.event == event_summary {
                    last_entry.count += 1;
                    last_entry.timestamp = Local::now();
                    return;
                }
            }

            if self.input_event_history.len() >= MAX_INPUT_EVENT_HISTORY {
                self.input_event_history.pop_front();
            }

            self.input_event_history.push_back(InputEventEntry {
                event: event_summary,
                count: 1,
                timestamp: Local::now(),
            });
        }
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        app_focus: Option<AppFocusState>,
        ui_state: &UiState,
        theme: &AppTheme,
        notifications: &Notifications<AppNotification>,
    ) -> EditorCommandOutput {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Shelv Debug Tools").size(theme.fonts.size.h4));
            ui.separator();
            if ui.button("Clear All").clicked() {
                self.action_history.clear();
                self.input_event_history.clear();
            }
        });

        ui.separator();

        let Self {
            action_history,
            input_event_history,
            show_pointer_events,
            expanded,
            ..
        } = self;

        let mut result_actions = EditorCommandOutput::default();

        // Note: the order matters, bottom goes first, so side panels are above it
        show_collapsible_pane(
            ui,
            Side::Bottom,
            &format!("Input Events ({})", input_event_history.len()),
            &mut expanded.input_events,
            |ui| {
                render_input_events_pane(
                    input_event_history.iter().rev(),
                    show_pointer_events,
                    ui,
                    theme,
                )
            },
        );

        show_collapsible_pane(
            ui,
            Side::Left,
            "Focus State",
            &mut expanded.focus_state,
            |ui| render_focus_state_pane(app_focus.as_ref(), ui_state, ui),
        );

        show_collapsible_pane(
            ui,
            Side::Right,
            &format!("Notifications ({})", notifications.count()),
            &mut expanded.notifications,
            |ui| {
                if let Some(actions) = render_notifications_pane(notifications, ui, theme) {
                    result_actions.extend(actions);
                }
            },
        );

        // Actions take the rest of the space, and a central panel can't shrink,
        // so when collapsed it is replaced with a strip on the left, like the side panes
        let actions_title = format!("Actions ({})", action_history.len());
        if expanded.actions {
            egui::CentralPanel::default()
                .frame(pane_frame())
                .show(ui, |ui| {
                    pane_header(ui, &actions_title, &mut expanded.actions, Side::Left);
                    render_actions_pane(action_history.iter().rev(), ui);
                });
        } else {
            Panel::left("dev_tools_Actions_collapsed")
                .resizable(false)
                .exact_size(COLLAPSED_PANE_SIZE)
                .frame(pane_frame())
                .show(ui, |ui| {
                    toggle_button(ui, &mut expanded.actions, Side::Left)
                        .on_hover_text(&actions_title);
                });
        }

        result_actions
    }
}

struct ExpandedPanes {
    focus_state: bool,
    input_events: bool,
    actions: bool,
    notifications: bool,
}

impl Default for ExpandedPanes {
    fn default() -> Self {
        Self {
            focus_state: true,
            input_events: true,
            actions: true,
            notifications: true,
        }
    }
}

#[derive(Clone, Copy)]
enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

const COLLAPSED_PANE_SIZE: f32 = 28.0;

fn pane_frame() -> Frame {
    Frame::new().inner_margin(Margin::same(4))
}

/// A panel on the given `side` with a header that has a button to collapse it into a thin strip
fn show_collapsible_pane(
    ui: &mut Ui,
    side: Side,
    title: &str,
    is_expanded: &mut bool,
    add_contents: impl FnOnce(&mut Ui),
) {
    let id = format!("dev_tools_{}", title.split(" (").next().unwrap_or(title));
    let collapsed_id = format!("{id}_collapsed");

    let (expanded_panel, collapsed_panel, default_size) = match side {
        Side::Left => (Panel::left(id), Panel::left(collapsed_id), 240.0),
        Side::Right => (Panel::right(id), Panel::right(collapsed_id), 340.0),
        Side::Top => (Panel::top(id), Panel::top(collapsed_id), 200.0),
        Side::Bottom => (Panel::bottom(id), Panel::bottom(collapsed_id), 220.0),
    };

    let expanded_panel = expanded_panel
        .resizable(true)
        .default_size(default_size)
        .frame(pane_frame());
    let collapsed_panel = collapsed_panel
        .resizable(true)
        .exact_size(COLLAPSED_PANE_SIZE)
        .frame(pane_frame());

    // the header button can't touch `is_expanded` directly, since `show_switched` borrows it,
    // so the closure reports whether the button was clicked
    let toggle_clicked = Panel::show_switched(
        ui,
        is_expanded,
        collapsed_panel,
        expanded_panel,
        |ui, show_expanded| {
            // note: during the collapse animation `show_expanded` is still true
            let mut expanded = show_expanded;
            if show_expanded {
                pane_header(ui, title, &mut expanded, side);
                add_contents(ui);
            } else {
                match side {
                    // a vertical strip is too narrow for the title, show it on hover
                    Side::Left | Side::Right => {
                        toggle_button(ui, &mut expanded, side).on_hover_text(title);
                    }
                    Side::Top | Side::Bottom => pane_header(ui, title, &mut expanded, side),
                }
            }
            expanded != show_expanded
        },
    )
    .inner;

    if toggle_clicked {
        *is_expanded = !*is_expanded;
    }
}

/// Title on the left, collapse/expand button on the right
fn pane_header(ui: &mut Ui, title: &str, is_expanded: &mut bool, side: Side) {
    ui.horizontal(|ui| {
        ui.strong(title);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            toggle_button(ui, is_expanded, side);
        });
    });
    if *is_expanded {
        ui.separator();
    }
}

/// The arrow points to where the pane goes when clicked
fn toggle_button(ui: &mut Ui, is_expanded: &mut bool, side: Side) -> Response {
    use egui_phosphor::light as P;

    let icon = match (side, *is_expanded) {
        (Side::Left, true) | (Side::Right, false) => P::CARET_LEFT,
        (Side::Left, false) | (Side::Right, true) => P::CARET_RIGHT,
        (Side::Top, true) | (Side::Bottom, false) => P::CARET_UP,
        (Side::Top, false) | (Side::Bottom, true) => P::CARET_DOWN,
    };

    let response = ui
        .small_button(RichText::new(icon).family(FontFamily::Name("phosphor".into())))
        .on_hover_text(if *is_expanded { "Collapse" } else { "Expand" });

    if response.clicked() {
        *is_expanded = !*is_expanded;
    }

    response
}

fn render_focus_state_pane(app_focus: Option<&AppFocusState>, ui_state: &UiState, ui: &mut Ui) {
    if let Some(focus_state) = app_focus {
        ui.horizontal(|ui| {
            ui.label("Menu Opened:");
            ui.label(focus_state.is_menu_opened.to_string());
        });
        ui.horizontal(|ui| {
            ui.label("Viewport Focused:");
            ui.label(focus_state.viewport_focused.to_string());
        });
        ui.horizontal(|ui| {
            ui.label("Internal Focus:");
            match focus_state.internal_focus {
                Some(AppFocus::NoteEditor) => ui.label("Note Editor"),
                Some(AppFocus::InlinePropmptEditor) => ui.label("Inline Prompt Editor"),
                Some(AppFocus::Other(id)) => ui.label(format!("Other({:?})", id)),
                None => ui.label("None"),
            };
        });
        ui.horizontal(|ui| {
            ui.label("Current focused:");
            ui.label(format!("{:?}", focus_state.focus_id));
        });
    } else {
        ui.label("No focus state available");
    }

    ui.separator();

    ui.label("UI State Attributes:");
    for (i, attr) in ui_state.attributes().iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("[{}]", i));
            ui.label(format!("{:?}", attr));
        });
    }
}

fn render_input_events_pane<'a>(
    input_event_history: impl IntoIterator<Item = &'a InputEventEntry>,
    show_pointer_events: &mut bool,
    ui: &mut Ui,
    theme: &AppTheme,
) {
    ui.checkbox(show_pointer_events, "Show pointer/mouse events");

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(200.0)
        .show(ui, |ui| {
            TableBuilder::new(ui)
                .column(egui_extras::Column::auto().resizable(true))
                .column(egui_extras::Column::auto())
                .column(egui_extras::Column::remainder())
                .header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.label(RichText::new("Time").size(theme.fonts.size.h4));
                    });
                    header.col(|ui| {
                        ui.label(RichText::new("Count").size(theme.fonts.size.h4));
                    });
                    header.col(|ui| {
                        ui.label(RichText::new("Event").size(theme.fonts.size.h4));
                    });
                })
                .body(|mut body| {
                    for entry in input_event_history.into_iter() {
                        if !*show_pointer_events
                            && (entry.event.contains("PointerMoved")
                                || entry.event.contains("MouseMoved"))
                        {
                            continue;
                        }

                        body.row(18.0, |mut row| {
                            row.col(|ui| {
                                ui.label(entry.timestamp.format("%H:%M:%S").to_string());
                            });
                            row.col(|ui| {
                                if entry.count > 1 {
                                    ui.label(format!("{}×", entry.count));
                                } else {
                                    ui.label("1");
                                }
                            });
                            row.col(|ui| {
                                ui.label(&entry.event);
                            });
                        });
                    }
                });
        });
}
fn render_actions_pane<'a>(action_history: impl Iterator<Item = &'a ActionLogEntry>, ui: &mut Ui) {
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for entry in action_history {
                ui.horizontal(|ui| {
                    ui.add_space(entry.depth as f32 * 20.0);

                    ui.label(format!(
                        "{:<8}",
                        entry.timestamp.format("%H:%M:%S").to_string()
                    ));
                    ui.separator();

                    let phase_text = match entry.phase {
                        ActionPhase::PreRender => "[pre] ",
                        ActionPhase::PostRender => "[post]",
                    };
                    ui.label(phase_text);
                    ui.separator();

                    egui::CollapsingHeader::new(&entry.action_debug)
                        .default_open(false)
                        .id_salt(Id::new(&entry.action_body).with(entry.timestamp))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("{:#?}", entry.action))
                                    .family(eframe::epaint::FontFamily::Monospace)
                                    .size(10.0),
                            );
                        });
                });
            }
        });
}

fn render_notifications_pane(
    notifications: &Notifications<AppNotification>,
    ui: &mut Ui,
    theme: &AppTheme,
) -> Option<EditorCommandOutput> {
    let mut actions = EditorCommandOutput::default();

    // Buttons to create different notification types
    ui.horizontal(|ui| {
        ui.label(RichText::new("Create Test Notifications:").size(theme.fonts.size.h4));
    });

    ui.horizontal_wrapped(|ui| {
        // Simple message notification
        if ui.button("Simple Message").clicked() {
            let notification = AppNotification {
                id: NotificationId::new(chrono::Utc::now().timestamp_millis() as u64),
                title: None,
                message: "This is a simple notification message.".to_string(),
                action: None,
            };
            actions.push(AppAction::ShowNotification(notification));
        }

        // Notification with title and icon
        if ui.button("With Title & Icon").clicked() {
            let notification = AppNotification {
                id: NotificationId::new(chrono::Utc::now().timestamp_millis() as u64 + 1),
                title: Some((
                    eframe::epaint::Color32::from_rgb(255, 165, 0),
                    AppIcon::Bug,
                    "Warning".to_string(),
                )),
                message: "This notification has a title with an icon and color.".to_string(),
                action: None,
            };
            actions.push(AppAction::ShowNotification(notification));
        }

        // Notification with action button
        if ui.button("With Action Button").clicked() {
            let notification = AppNotification {
                id: NotificationId::new(chrono::Utc::now().timestamp_millis() as u64 + 2),
                title: Some((
                    eframe::epaint::Color32::from_rgb(0, 255, 0),
                    AppIcon::Check,
                    "Success".to_string(),
                )),
                message: "This notification has an action button.".to_string(),
                action: Some(AppNotificationAction {
                    button_text: "Do Something".to_string(),
                    icon: Some(AppIcon::Play),
                    handler: Box::new(EditorCommandOutput::from([AppAction::StartTutorial])),
                }),
            };
            actions.push(AppAction::ShowNotification(notification));
        }

        // Notification with action button
        if ui.button("Long one").clicked() {
            let notification = AppNotification {
                id: NotificationId::new(chrono::Utc::now().timestamp_millis() as u64 + 2),
                title: Some((
                    eframe::epaint::Color32::from_rgb(0, 255, 0),
                    AppIcon::Check,
                    "Success".to_string(),
                )),
                message: "This is a long one, should be wrapped".repeat(5),
                action: Some(AppNotificationAction {
                    button_text: "Do Something".to_string(),
                    icon: Some(AppIcon::Play),
                    handler: Box::new(EditorCommandOutput::from([AppAction::StartTutorial])),
                }),
            };
            actions.push(AppAction::ShowNotification(notification));
        }

        // Error notification
        if ui.button("Error Notification").clicked() {
            let notification = AppNotification {
                id: NotificationId::new(chrono::Utc::now().timestamp_millis() as u64 + 3),
                title: Some((
                    eframe::epaint::Color32::from_rgb(255, 0, 0),
                    AppIcon::Error,
                    "Error".to_string(),
                )),
                message: "Something went wrong! This is an error notification.".to_string(),
                action: Some(AppNotificationAction {
                    button_text: "Retry".to_string(),
                    icon: Some(AppIcon::Play),
                    handler: Box::new(EditorCommandOutput::from([AppAction::ToggleDevTools])),
                }),
            };
            actions.push(AppAction::ShowNotification(notification));
        }
    });

    ui.separator();

    // Display current notification state
    ui.label(RichText::new("Current Notification State:").size(theme.fonts.size.h4));

    ScrollArea::vertical()
        .auto_shrink([false, false])
        // .max_height(300.0)
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("Total notifications: {}", notifications.count()))
                    .size(theme.fonts.size.small),
            );
            ui.separator();

            ui.label(RichText::new("Notifications Debug Info:").size(theme.fonts.size.small));
            ui.label(
                RichText::new(format!("{:#?}", notifications))
                    .family(eframe::epaint::FontFamily::Monospace)
                    .size(theme.fonts.size.small),
            );
        });

    if actions.is_empty() {
        None
    } else {
        Some(actions)
    }
}

fn event_summary(event: &egui::Event) -> String {
    match event {
        egui::Event::PointerMoved { .. } => "PointerMoved { .. }".to_owned(),
        egui::Event::MouseMoved { .. } => "MouseMoved { .. }".to_owned(),
        egui::Event::Zoom { .. } => "Zoom { .. }".to_owned(),
        egui::Event::Touch { phase, .. } => format!("Touch {{ phase: {phase:?}, .. }}"),
        egui::Event::MouseWheel { unit, .. } => format!("MouseWheel {{ unit: {unit:?}, .. }}"),

        _ => format!("{event:?}"),
    }
}
