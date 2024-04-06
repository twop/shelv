use std::{
    fs, io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
const CURRENT_VERSION: i32 = 2;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
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

pub enum HydrationResult {
    FolderIsMissing,
    Success(RestoredData),
}

pub fn try_hydrate(number_of_notes: u32, folder: PathBuf) -> Result<HydrationResult, SaveError> {
    todo!()
}

pub fn try_save<'a>(data: DataToSave<'a>, folder: PathBuf) -> Result<State, SaveError> {
    let DataToSave { files, selected } = data;

    fs::create_dir_all(folder)?;

    let state = State {
        version: CURRENT_VERSION,
        last_saved: get_current_utc_timestamp(),
        selected,
    };

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

fn get_current_utc_timestamp() -> u128 {
    let start = SystemTime::now();
    let now = start
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    now
}

pub fn fn_migrate_from_v1(old_state: &v1::PersistentState) -> (DataToSave, RestoredData) {
    let selected = NoteFile::Note(old_state.selected_note);
    let to_save = DataToSave {
        files: old_state
            .notes
            .iter()
            .enumerate()
            .map(|(index, note)| (NoteFile::Note(index as u32), note.as_ref()))
            // TODO settings note
            // .chain([(NoteFile::Settings, "")])
            .collect(),
        selected,
    };

    let restored_data = RestoredData {
        state: State {
            version: CURRENT_VERSION,
            last_saved: get_current_utc_timestamp(),
            selected,
        },
        notes: old_state.notes.iter().map(|s| s.to_string()).collect(),
        // TODO settings note
        settings: "".to_string(),
    };

    (to_save, restored_data)
}

pub fn bootstrap(number_of_notes: u32) -> (DataToSave<'static>, RestoredData) {
    let selected = NoteFile::Note(0);
    let to_save = DataToSave {
        // TODO fill out welcome notes
        files: (0..number_of_notes)
            .into_iter()
            .map(|index| (NoteFile::Note(index as u32), ""))
            // TODO settings note
            // .chain([(NoteFile::Settings, "")])
            .collect(),
        selected,
    };

    // TODO fill out welcome notes
    let restored_data = RestoredData {
        state: State {
            version: CURRENT_VERSION,
            last_saved: get_current_utc_timestamp(),
            selected,
        },
        notes: (0..number_of_notes)
            .into_iter()
            .map(|_| "".to_owned())
            .collect(),
        // TODO settings note
        settings: "".to_string(),
    };

    (to_save, restored_data)
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
