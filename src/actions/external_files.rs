use std::path::PathBuf;

use smallvec::SmallVec;

use crate::{
    app_actions::{AppAction, AppIO, SwitchToNoteTarget},
    app_state::{AppState, Note, NoteDerivedState, UnsavedChange},
    persistent_state::{ExternalFile, ExternalFileId, NoteId},
};

/// Opens an external file by reading it from disk and adding it to the application state.
/// If the file is already open, it simply switches to that file.
pub fn open_external_file(
    path: PathBuf,
    state: &mut AppState,
    app_io: &mut impl AppIO,
) -> SmallVec<[AppAction; 1]> {
    // Check if this file is already open
    let already_open = state.external_files.iter().find(|f| f.path == path);

    if let Some(existing_file) = already_open {
        // File is already open, just switch to it
        SmallVec::from([AppAction::SwitchToNote(SwitchToNoteTarget::TargetNote {
            note_file: NoteId::ExternalFileId(existing_file.id),
            via_shortcut: false,
        })])
    } else {
        // Read the file and add it to the system
        match app_io.read_file(&path) {
            Ok(content) => {
                // Generate a unique ID for this external file based on its path
                let file_id = ExternalFileId::from_pathbuf(&path);

                let note_id = NoteId::ExternalFileId(file_id);

                // Create the external file entry
                let external_file = ExternalFile {
                    id: file_id,
                    path: path.clone(),
                };

                // FIXME show a notification that failed was failed to be watched
                let _ = app_io.watch_external_file(&external_file);

                state.external_files.push(external_file);

                // Create a note for this external file
                let derived_state = NoteDerivedState::new_from(&content);
                let note = Note::new(content, derived_state);

                state.notes.insert(note_id, note);
                state.add_unsaved_change(UnsavedChange::NoteContentChanged(note_id));

                // Switch to the newly opened file
                SmallVec::from([AppAction::SwitchToNote(SwitchToNoteTarget::TargetNote {
                    note_file: note_id,
                    via_shortcut: true,
                })])
            }
            Err(err) => {
                println!("Failed to open external file {}: {}", path.display(), err);
                SmallVec::new()
            }
        }
    }
}

/// Closes an external file by removing it from the application state.
/// If the closed file was currently selected, switches to the first note.
pub fn close_external_file(
    file_id: ExternalFileId,
    state: &mut AppState,
    app_io: &mut impl AppIO,
) -> SmallVec<[AppAction; 1]> {
    let note_id = NoteId::ExternalFileId(file_id);

    // Remove the note from the notes map
    state.notes.remove(&note_id);

    // Remove from external files list
    state.external_files.retain(|f| f.id != file_id);

    let _ = app_io.unwatch_external_file(file_id);

    // If this was the selected note, switch to the first note
    if state.selected_note == note_id {
        SmallVec::from([AppAction::SwitchToNote(SwitchToNoteTarget::TargetNote {
            note_file: NoteId::Note(0),
            via_shortcut: true,
        })])
    } else {
        SmallVec::new()
    }
}
