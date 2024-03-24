use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct PersistentState {
    pub notes: Vec<String>,
    pub settings_note: String,
    pub selected_note: u32,
}
