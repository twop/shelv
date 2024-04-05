use std::{
    fs, io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
enum NoteFile {
    Note(u32),
    Settings,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct State {
    version: i32,
    last_saved: u128,
    selected: NoteFile,
}

pub struct RestoredData {
    pub state: State,
    pub notes: Vec<String>,
    pub settings: String,
}

pub struct DataToSave<'a> {
    pub files: Vec<(NoteFile, &'a str)>,
    pub selected: NoteFile,
}

#[derive(Debug)]
pub enum SaveError {
    FileError(io::Error),
    StateSaveError(serde_json::Error),
}

impl From<io::Error> for SaveError {
    fn from(value: io::Error) -> Self {
        SaveError::FileError(value)
    }
}

impl From<serde_json::Error> for SaveError {
    fn from(value: serde_json::Error) -> Self {
        SaveError::StateSaveError(value)
    }
}

pub fn save<'a>(data: DataToSave<'a>, folder: PathBuf) -> Result<State, SaveError> {
    let DataToSave { files, selected } = data;

    let start = SystemTime::now();
    let now = start
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    let state = State {
        version: 1,
        last_saved: now,
        selected,
    };

    fs::create_dir_all(folder)?;

    fs::write(
        folder.join("state.json"),
        serde_json::to_string_pretty(&state)?,
    )?;

    for (note, content) in files {
        let file_name = match note {
            NoteFile::Note(zero_based_index) => format!("note-{}.md", zero_based_index + 1),
            NoteFile::Settings => "settings.md".to_string(),
        };

        fs::write(folder.join(file_name), content)?;
    }

    Ok(state)
}

// ---------------------- older versions --------------
mod v1 {
    use serde::{Deserialize, Serialize};
    #[derive(Debug, Deserialize, Serialize)]
    pub struct PersistentState {
        pub notes: Vec<String>,
        pub selected_note: u32,
    }
}
