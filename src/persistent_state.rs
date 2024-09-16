use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
const CURRENT_VERSION: i32 = 2;

use serde::{Deserialize, Serialize};

#[derive(Debug, Hash, Clone, PartialEq, Ord, PartialOrd, Eq, Copy, Deserialize, Serialize)]
pub enum NoteFile {
    Note(u32),
    Settings,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SaveState {
    version: i32,

    #[serde(default = "default_window_pinned_value")]
    pub is_pinned: bool,
    pub last_saved: u128,
    pub selected: NoteFile,
}

fn default_window_pinned_value() -> bool {
    true
}

pub struct RestoredData {
    pub state: SaveState,
    pub notes: Vec<String>,
    pub settings: String,
}

#[derive(Debug)]
pub struct DataToSave<'a> {
    pub files: Vec<(NoteFile, &'a str)>,
    pub selected: NoteFile,
    pub is_pinned: bool,
}

#[derive(Debug)]
pub enum LoadSaveError {
    FileError(io::Error),
    StateSaveError(serde_json::Error),
}

impl From<io::Error> for LoadSaveError {
    fn from(value: io::Error) -> Self {
        LoadSaveError::FileError(value)
    }
}

impl From<serde_json::Error> for LoadSaveError {
    fn from(value: serde_json::Error) -> Self {
        LoadSaveError::StateSaveError(value)
    }
}

pub enum HydrationResult {
    FolderIsMissing,
    Success(RestoredData),
    Partial(RestoredData, DataToSave<'static>),
}

pub fn load_and_migrate<'s>(
    number_of_notes: u32,
    v1_save: Option<v1::PersistentState>,
    folder: &PathBuf,
) -> RestoredData {
    let load_result = try_hydrate(number_of_notes, &folder);

    match (load_result, v1_save) {
        (Ok(HydrationResult::Success(data)), _) => data,
        (Ok(HydrationResult::FolderIsMissing) | Err(_), v1_save) => {
            let (to_save, data) = match &v1_save {
                Some(v1_save) => fn_migrate_from_v1(&v1_save),
                None => bootstrap(number_of_notes),
            };
            try_save(to_save, &folder).unwrap();
            data
        }
        (Ok(HydrationResult::Partial(data, to_save)), _) => {
            try_save(to_save, &folder).unwrap();
            data
        }
    }
}

fn try_hydrate(number_of_notes: u32, folder: &PathBuf) -> Result<HydrationResult, LoadSaveError> {
    let true = Path::new(&folder).try_exists()? else {
        println!("try_hydrate: {} is missing", folder.to_string_lossy());
        return Ok(HydrationResult::FolderIsMissing);
    };

    let mut retrieved_files: Vec<(NoteFile, String)> = vec![];

    let mut state: Option<SaveState> = None;

    for entry in fs::read_dir(&folder)? {
        let entry = entry?;
        let meta = entry.metadata()?;

        if let (true, Some(file_name)) = (meta.is_file(), entry.file_name().to_str()) {
            println!("try_hydrate: processing {file_name}");

            let note_file = extract_note_file(file_name);

            if let Some((note_file, file_name)) = note_file {
                let content = fs::read_to_string(folder.join(file_name))?;
                println!("try_hydrate: detected file {file_name} as {note_file:?}");
                retrieved_files.push((note_file, content));
            }

            if file_name == "state.json" {
                state = serde_json::from_str(&fs::read_to_string(folder.join(file_name))?).ok();
                println!("try_hydrate: read and parsed state.json");
            }
        }
    }

    let mut missing_notes = vec![];

    let mut notes = vec![];

    for index in 0..number_of_notes {
        let searched_note_file = NoteFile::Note(index);
        if let Some((_, note_content)) = retrieved_files
            .iter()
            .find(|(note_file, _)| *note_file == searched_note_file)
        {
            notes.push(note_content.to_string());
        } else {
            notes.push(get_tutorial_note_content(searched_note_file).to_string());
            println!("try_hydrate: detected missing {searched_note_file:?}");
            missing_notes.push((
                searched_note_file,
                get_tutorial_note_content(searched_note_file),
            ))
        }
    }

    let state_parsed = state.is_some();

    // NOTE that we don't do any version checks yet
    let state = state.unwrap_or_else(|| SaveState {
        version: CURRENT_VERSION,
        is_pinned: true,
        last_saved: get_current_utc_timestamp(),
        selected: NoteFile::Note(0),
    });

    let selected = state.selected;
    let is_pinned = state.is_pinned;

    let restored = RestoredData {
        state,
        notes,
        settings: retrieved_files
            .into_iter()
            .find(|(note_file, _)| *note_file == NoteFile::Settings)
            .map(|(_, content)| content)
            .unwrap_or_else(|| "".to_string()),
    };

    if state_parsed && missing_notes.is_empty() {
        println!("try_hydrate: restored in full");
        Ok(HydrationResult::Success(restored))
    } else {
        println!(
            "try_hydrate: partial restoration, state_parsed={state_parsed}, missing_notes={}",
            !missing_notes.is_empty()
        );
        Ok(HydrationResult::Partial(
            restored,
            DataToSave {
                files: missing_notes,
                selected,
                is_pinned,
            },
        ))
    }
}

pub fn extract_note_file(file_name: &str) -> Option<(NoteFile, &str)> {
    match file_name {
        "settings.md" => Some((NoteFile::Settings, "settings.md")),
        note if note.starts_with("note-") && note.ends_with(".md") => note
            .strip_prefix("note-")
            .and_then(|s| s.strip_suffix(".md"))
            .and_then(|s| s.parse().ok())
            .map(|i: u32| (NoteFile::Note(i - 1), note)),
        _ => None,
    }
}

pub fn try_save<'a>(data: DataToSave<'a>, folder: &PathBuf) -> Result<SaveState, LoadSaveError> {
    let DataToSave {
        files,
        is_pinned,
        selected,
    } = data;

    fs::create_dir_all(folder)?;

    let state = SaveState {
        version: CURRENT_VERSION,
        is_pinned,
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
    get_utc_timestamp(start)
}

pub fn get_utc_timestamp(start: SystemTime) -> u128 {
    start
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

pub fn fn_migrate_from_v1<'s>(
    old_state: &'s v1::PersistentState,
) -> (DataToSave<'s>, RestoredData) {
    let selected = NoteFile::Note(old_state.selected_note);
    let to_save = DataToSave {
        files: old_state
            .notes
            .iter()
            .enumerate()
            .map(|(index, note)| (NoteFile::Note(index as u32), note.as_ref()))
            .chain([(
                NoteFile::Settings,
                get_tutorial_note_content(NoteFile::Settings),
            )])
            .collect(),
        is_pinned: true,
        selected,
    };

    let restored_data = RestoredData {
        state: SaveState {
            version: CURRENT_VERSION,
            last_saved: get_current_utc_timestamp(),
            is_pinned: true,
            selected,
        },
        notes: old_state.notes.iter().map(|s| s.to_string()).collect(),
        settings: get_tutorial_note_content(NoteFile::Settings).to_string(),
    };

    (to_save, restored_data)
}

pub fn bootstrap(number_of_notes: u32) -> (DataToSave<'static>, RestoredData) {
    let selected = NoteFile::Note(0);
    let to_save = DataToSave {
        files: (0..number_of_notes)
            .into_iter()
            .map(|index| NoteFile::Note(index as u32))
            .chain([NoteFile::Settings])
            .map(|file| (file, get_tutorial_note_content(file)))
            .collect(),
        selected,
        is_pinned: true,
    };

    let restored_data = RestoredData {
        state: SaveState {
            is_pinned: true,
            version: CURRENT_VERSION,
            last_saved: get_current_utc_timestamp(),
            selected,
        },
        notes: (0..number_of_notes)
            .into_iter()
            .map(|i| get_tutorial_note_content(NoteFile::Note(i)))
            .map(|s| s.to_string())
            .collect(),
        settings: get_tutorial_note_content(NoteFile::Settings).to_string(),
    };

    (to_save, restored_data)
}

pub fn get_tutorial_note_content(note: NoteFile) -> &'static str {
    match note {
        NoteFile::Settings => include_str!("./default-notes/default-settings.md"),
        NoteFile::Note(0) => include_str!("./default-notes/default-note-1.md"),
        NoteFile::Note(1) => include_str!("./default-notes/default-note-2.md"),
        NoteFile::Note(2) => include_str!("./default-notes/default-note-3.md"),
        NoteFile::Note(3) => include_str!("./default-notes/default-note-4.md"),
        _ => "",
    }
}

// ---------------------- older versions --------------
pub mod v1 {
    use serde::{Deserialize, Serialize};
    #[derive(Debug, Deserialize, Serialize)]
    pub struct PersistentState {
        pub notes: Vec<String>,
        pub selected_note: u32,
    }
}
