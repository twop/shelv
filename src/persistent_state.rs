use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct PersistentState {
    pub note: String,
    pub selected_note: u32,
}
