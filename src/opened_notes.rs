use crate::persistent_state::{ExternalFile, NoteId};

use super::app_state::Note;

/// Container for open notes, maintaining insertion order for UI display and navigation.
/// Provides a BTreeMap-like API (get, get_mut, insert, remove) but backed by a Vec
/// to preserve the order notes were opened.
pub struct OpenNotes {
    notes: Vec<(NoteId, Note)>,
    pub selected_note: NoteId,
    external_files: Vec<ExternalFile>,
}

impl OpenNotes {
    pub fn new(notes: Vec<(NoteId, Note)>, selected_note: NoteId) -> Self {
        Self {
            notes,
            selected_note,
            external_files: Vec::new(),
        }
    }

    pub fn get(&self, note_id: &NoteId) -> Option<&Note> {
        self.notes
            .iter()
            .find(|(id, _)| id == note_id)
            .map(|(_, note)| note)
    }

    pub fn get_mut(&mut self, note_id: &NoteId) -> Option<&mut Note> {
        self.notes
            .iter_mut()
            .find(|(id, _)| id == note_id)
            .map(|(_, note)| note)
    }

    /// Insert a note. If it already exists, replaces it in place.
    /// Otherwise appends to the end.
    pub fn insert(&mut self, note_id: NoteId, note: Note) {
        if let Some(entry) = self.notes.iter_mut().find(|(id, _)| id == &note_id) {
            // Replace existing note
            entry.1 = note;
        } else {
            // Append new note
            self.notes.push((note_id, note));
        }
    }

    /// Remove a note by its ID, returning the note if it existed.
    pub fn remove(&mut self, note_id: NoteId) -> Option<Note> {
        let index = self.notes.iter().position(|(id, _)| *id == note_id)?;

        if let NoteId::ExternalFileId(file_id) = note_id {
            self.external_files.retain(|f| f.id != file_id);
        }

        Some(self.notes.remove(index).1)
    }

    /// Iterate over all notes in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &(NoteId, Note)> {
        self.notes.iter()
    }

    /// Find the index of a note in the Vec.
    pub fn find_index(&self, note_id: &NoteId) -> Option<usize> {
        self.notes.iter().position(|(id, _)| id == note_id)
    }

    /// changes selected note by `amount` (note can be negative or positive)
    ///   if exceeds len of opened, then it circles
    ///   returns a newly selected NoteId if selection was successful
    pub fn shift_note_selection_by(&mut self, amount: i32) -> Option<NoteId> {
        let current = self.selected_note;
        let len = self.notes.len();

        if len == 0 || len == 1 {
            return None;
        }

        let current_index = self
            .notes
            .iter()
            .enumerate()
            .find_map(|(pos, note)| (note.0 == current).then(|| pos))?;

        let mut new_index = current_index as i32 + amount;

        while new_index < 0 {
            // that will handle negative shift by wrapping it from the right
            //  e.g. if current_index == 0, and amount == -1, then the new index is going to be
            //  new_index = len - 1 (which is the last element)
            new_index = new_index + len as i32;
        }

        // this will handle the other direction,
        //   if current_index == len - 1, and amount == 1
        //   then new_index == 0 (which is the first element)
        // note that casting to usize is safe, due to it being positive
        let new_index = new_index as usize % len;

        let new_selection = self.notes[new_index].0;

        Some(new_selection)
    }

    /// Get a note by its index in the Vec.
    pub fn get_by_index(&self, index: usize) -> Option<&(NoteId, Note)> {
        self.notes.get(index)
    }

    /// Check if there are no open notes.
    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::NoteDerivedState;

    fn make_note(text: &str) -> Note {
        Note::new(text.to_string(), NoteDerivedState::new_from(text))
    }

    #[test]
    fn test_insert_and_get() {
        let mut notes = OpenNotes::new(vec![], NoteId::Note(0));

        notes.insert(NoteId::Note(0), make_note("note 0"));
        notes.insert(NoteId::Note(1), make_note("note 1"));

        assert_eq!(notes.get(&NoteId::Note(0)).unwrap().text, "note 0");
        assert_eq!(notes.get(&NoteId::Note(1)).unwrap().text, "note 1");
        assert!(notes.get(&NoteId::Note(2)).is_none());
    }

    #[test]
    fn test_insert_replace() {
        let mut notes = OpenNotes::new(vec![], NoteId::Note(0));

        notes.insert(NoteId::Note(0), make_note("original"));
        notes.insert(NoteId::Note(0), make_note("replaced"));

        assert_eq!(notes.get(&NoteId::Note(0)).unwrap().text, "replaced");
        assert_eq!(notes.len(), 1); // Should not duplicate
    }

    #[test]
    fn test_remove() {
        let mut notes = OpenNotes::new(vec![], NoteId::Note(0));

        notes.insert(NoteId::Note(0), make_note("note 0"));
        notes.insert(NoteId::Note(1), make_note("note 1"));

        let removed = notes.remove(NoteId::Note(0));
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().text, "note 0");
        assert!(notes.get(&NoteId::Note(0)).is_none());
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn test_insertion_order() {
        let mut notes = OpenNotes::new(vec![], NoteId::Note(0));

        notes.insert(NoteId::Note(2), make_note("note 2"));
        notes.insert(NoteId::Note(0), make_note("note 0"));
        notes.insert(NoteId::Note(1), make_note("note 1"));

        let ids: Vec<NoteId> = notes.keys().copied().collect();
        assert_eq!(ids, vec![NoteId::Note(2), NoteId::Note(0), NoteId::Note(1)]);
    }

    #[test]
    fn test_find_index() {
        let mut notes = OpenNotes::new(vec![], NoteId::Note(0));

        notes.insert(NoteId::Note(0), make_note("note 0"));
        notes.insert(NoteId::Note(1), make_note("note 1"));
        notes.insert(NoteId::Settings, make_note("settings"));

        assert_eq!(notes.find_index(&NoteId::Note(0)), Some(0));
        assert_eq!(notes.find_index(&NoteId::Note(1)), Some(1));
        assert_eq!(notes.find_index(&NoteId::Settings), Some(2));
        assert_eq!(notes.find_index(&NoteId::Note(99)), None);
    }

    #[test]
    fn test_get_by_index() {
        let mut notes = OpenNotes::new(vec![], NoteId::Note(0));

        notes.insert(NoteId::Note(0), make_note("note 0"));
        notes.insert(NoteId::Note(1), make_note("note 1"));

        let (id, note) = notes.get_by_index(0).unwrap();
        assert_eq!(*id, NoteId::Note(0));
        assert_eq!(note.text, "note 0");

        let (id, note) = notes.get_by_index(1).unwrap();
        assert_eq!(*id, NoteId::Note(1));
        assert_eq!(note.text, "note 1");

        assert!(notes.get_by_index(2).is_none());
    }
}
