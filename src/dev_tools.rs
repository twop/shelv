use std::collections::VecDeque;

use chrono::{DateTime, Local};
use eframe::egui::{self, Id, InputState, Margin, RichText, ScrollArea, Ui, containers::Frame};
use egui_extras::TableBuilder;
use egui_tiles::{Behavior, TileId, Tiles, Tree, UiResponse};

use crate::{
    app_actions::AppAction,
    command::{AppFocus, AppFocusState, UiState},
    theme::AppTheme,
};

const MAX_ACTION_HISTORY: usize = 1000;
const MAX_INPUT_EVENT_HISTORY: usize = 100;

#[derive(Debug, Clone, Copy)]
pub enum ActionPhase {
    PreRender,
    PostRender,
}

#[derive(Debug, Clone)]
enum DevToolPane {
    FocusState,
    InputEvents,
    Actions,
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
    tree: Tree<DevToolPane>,
}

impl Default for DevToolsState {
    fn default() -> Self {
        let mut tiles = Tiles::default();

        let focus_tile = tiles.insert_pane(DevToolPane::FocusState);
        let input_events_tile = tiles.insert_pane(DevToolPane::InputEvents);
        let actions_tile = tiles.insert_pane(DevToolPane::Actions);

        let root = tiles.insert_tab_tile(vec![focus_tile, input_events_tile, actions_tile]);
        let tree = Tree::new("dev_tools_tree", root, tiles);

        Self {
            show_dev_tools: false,
            action_history: VecDeque::with_capacity(MAX_ACTION_HISTORY),
            input_event_history: VecDeque::with_capacity(MAX_INPUT_EVENT_HISTORY),
            show_pointer_events: false,
            tree,
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

    pub fn show(&mut self, ui: &mut Ui, app_focus: Option<AppFocusState>, ui_state: &UiState, theme: &AppTheme) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Shelv Debug Tools").size(theme.fonts.size.h4));
            ui.separator();
            if ui.button("Clear All").clicked() {
                self.action_history.clear();
                self.input_event_history.clear();
            }
        });

        ui.separator();

        let mut behavior = DevToolsBehavior {
            app_focus,
            ui_state,
            theme,
            action_history: &self.action_history,
            input_event_history: &self.input_event_history,
            show_pointer_events: &mut self.show_pointer_events,
        };

        self.tree.ui(&mut behavior, ui);
    }
}

struct DevToolsBehavior<'a> {
    app_focus: Option<AppFocusState>,
    ui_state: &'a UiState,
    theme: &'a AppTheme,
    action_history: &'a VecDeque<ActionLogEntry>,
    input_event_history: &'a VecDeque<InputEventEntry>,
    show_pointer_events: &'a mut bool,
}

impl<'a> Behavior<DevToolPane> for DevToolsBehavior<'a> {
    fn pane_ui(&mut self, ui: &mut Ui, _tile_id: TileId, pane: &mut DevToolPane) -> UiResponse {
        Frame::new()
            .inner_margin(Margin::same(2))
            .show(ui, |ui| match pane {
                DevToolPane::FocusState => {
                    render_focus_state_pane(self.app_focus.as_ref(), self.ui_state, ui);
                }
                DevToolPane::InputEvents => {
                    render_input_events_pane(
                        self.input_event_history.iter().rev(),
                        &mut self.show_pointer_events,
                        ui,
                        self.theme,
                    );
                }
                DevToolPane::Actions => {
                    render_actions_pane(self.action_history.iter().rev(), ui);
                }
            });
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &DevToolPane) -> egui::WidgetText {
        match pane {
            DevToolPane::FocusState => "Focus State".into(),
            DevToolPane::InputEvents => {
                format!("Input Events ({})", self.input_event_history.len()).into()
            }
            DevToolPane::Actions => format!("Actions ({})", self.action_history.len()).into(),
        }
    }
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
