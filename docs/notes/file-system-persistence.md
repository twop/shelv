---
Date: 2024/Apr/4
---

# File system persistence

## Folder structure example:

- notes
  - `note-1-aty4dsaf.md`
  - `note-2-yu435ga.md`
  - `note-3-fsdffzf23.md`
  - `note-4-fdsxdfbs.md`
- `state.json`
- archive
  - `2024-Apr-04-Shelv Technology-osfi7rtps.md`

## `state.json`

```json
{
  // important for data migrations
  "version": 1,

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

## Logic

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
      - all archiving rules apply
  4.  modify `state.json`

## Bidirectional sync

You should be able to edit `.md` files from VSCode (or via other means) and Shelv should be able to pick them up, including reevaluating `js` code blocks and writing it back.

TBD

- write confilcts
- what tools to use for watching files

## Interop with Sync and collaboration

TBD
