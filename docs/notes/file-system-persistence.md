---
Date: 2024/Apr/4
---

# File system persistence

## Version 1 (mvp/first release)

### Folder structure example:

- notes
  - `note-1.md`
  - `note-2.md`
  - `note-3.md`
  - `note-4.md`
- `state.json`
- `settings.md`

### `state.json`

```json
{
  // important for data migrations
  "version": 1,

  // timestamp, needed for bi-directional syncing of notes
  "lastSaved": 63245436,

  // note that it is zero based
  // and potentially can select other things like "selected": "settings"
  // TODO possibly encode it as `"selected": "note:1"`
  "selected": { "note": 0 }

  // TODO think about cursor positions within notes
}
```

### How it works

- we don't have sync nor collaboration yet, and no archiving, so `id`s are not needed (yet?)
- so there is no need to refer to actual notes inside `notes` in `state.json`
- if there we are missing `note-{1-4}.md` create them at the start, assuming we are still on `"version": 1`
- TBD if we want to have `selected` field in `state.json` to be able to refer to settings
- TBD if just a flat folder structure with 6 files is more that enough (e.g. no nesting)
- when we detect a file change in shelv via a deamon of some sort
  - refresh content for all notes including settings, note that it may result in re-running live scripts and refreshing settings
  - update selected note
  - save back all notes (if needed) + save `state.json` to reflect the latest

### Algorithm

1. read the folder where all files should be located
2. if not there then read in `eframe::Storage` then dump the persistent model on disk
3. if some of the files are missing regenerate them, note that we should not clean up files and move to archive folder (yet)
4. do migration based on `state.json` `version` field
5. populate hydration model (persistent model)
6. load the app with it
7. now on `save` call from egui dump stuff on disk in the desired format

### Links

- https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/FileSystemOverview/FileSystemOverview.html#//apple_ref/doc/uid/TP40010672-CH2-SW1
- https://docs.rs/directories-next/2.0.0/directories_next/struct.ProjectDirs.html#method.data_dir

---

## Version 2 (next/future)

### Folder structure example:

- notes
  - `note-1-aty4dsaf.md`
  - `note-2-yu435ga.md`
  - `note-3-fsdffzf23.md`
  - `note-4-fdsxdfbs.md`
- `settings.md`
- `state.json`
- archive
  - `2024-Apr-04-Shelv Technology-osfi7rtps.md`

### `state.json`

```json
{
  // important for data migrations
  "version": 2,

  // timestamp, needed for bi-directional syncing of notes
  "lastSaved": 63245436,

  // note that order here matters
  // and potentially can be different from `note-1-{...}`
  "notes": [
    "note-1-aty4dsaf.md",
    "note-2-yu435ga.md",
    "note-3-fsdffzf23.md",
    "note-4-fdsxdfbs.md"
  ],
  // note that it is zero based
  // and potentially can select other things
  "selected": { "note": 0 }
}
```

### Logic

- at the start we generate empty files for each note slot
  - note that we can generate notes on demand as well, for example when we "unlock" more notes
- if we have all slots already filled in but have some lingering `.md` files => move them to `archive`
  - note that we prepend the file name with `yyyy-mmm-dd` where `mmm` in a format of `Apr` to avoid ambiguety
  - if possible take the first header block (be it `h1` or `h2` etc) and take the first 2-3 words as a file name part
  - final format: `2024-Apr-04-Shelv Technology-osfi7rtps.md`
  - note that `id` might be important again, but possibly we can relax this constraint
- at the start first read `state.json` `version` field and perform data migrations if needed
- note that the most important part is a randomly assigned `id` like so `"fsdffzf23"`
  - it is not a hash, just a random sequence of ascii symbols (lenght TBD, but somewhere between 4 and 8 should be OK)
  - it is likely to be very useful when we will be wiring up CRDT and/or SQLite
- archiving operation
  1.  generate a new `id`
  2.  create a file `note-{index}-{id}.md`
  3.  move the archived file to `archive` folder
  4.  modify `state.json`

### Bidirectional sync

You should be able to edit `.md` files from VSCode (or via other means) and Shelv should be able to pick them up, including reevaluating `js` code blocks and writing it back.

TBD

- write confilcts
- what tools to use for watching files

### Interop with Sync and collaboration

TBD
