---
Date: 2024/Mar/17
---

# Blockers for launch:

- Snippets (do more with liveblocks) <-- Simon
  - more complex, need to determine API / data model
  - mv: shortcut that can produce code (register editor command)
  - how to represent the "settings" note (visually)
- Lock / keep open (keep shelv visible out of focus)
  - **Flexible window sizing** (not sure why it was limited)
  - Saved window size / position (like warp)
- File system storage
  - Example: note1.mdx
  - **Archive? or send to** <whatever app>
    - Requirements:
      - note clears up
      - it is saved somewhere I can access later (prob file at least)
      - ideally: there is some UI for browsing this in shelv
      - basically: do not fear, you will lose nothing (but you will probably never look at it again)
    - maybe even part of the file system
- More than 4 notes (or something... )
  - 1-9 shorcuts (maybe 10 max notes)
  - Mirza: 4 does not feel like enough. I also never know which note # is which doc
  - named pages?
    - look at the first # heading
    - Special shelvs: "TODO" list
    - (maybe even name the window)
      - Title: "Shelv - TODOs" (also could be taken from first heading)
- "Pro version" <-- Mirza
  - in app purchase? (free to download + use intially)
    - some limitations (# of shelvs, annoying "trial" UI, etc.)
  - subscription? (maybe when we have sync)
  - (there is some work to integrate w/ Apple regardless)
  - https://tauri.app/
  - cargo bundle
- Polish
  - list edge-cases (fml)
    - shift tab, enter on empty list item, etc
  - auto-scroll on more text at end of file (we can control this in egui)
  - js blocks that do not compile / aren't intended to have output (how do we handle this?)
    - mirza: honestly, not sure how I feel about the output blocks having the #hash

# Post-launch

- Multiplayer/sync (huge feature, prob post-launch and pro feature)
  - "share on web version"

```js
// this does not generate output
```

```js
// this does not generate output
```

```js#namedblock
// this generates output
fdsfsdfsdf
```

```#namedblock

```

```js#t3456

```

```filesystem
settings.mdx
// these are 4 notes
// note that "todos" can be taken from h1
todos-slippy-lemur.mdx
note-fat-panda.mdx
note-#t45f.mdx
note-#t45f.mdx
state.json

// archive folder
archive
// after archiving
-> todos-slippy-lemur.mdx
```

zoom?
state.json content:

```json
{
  notes: [
    {id: "note-slippy-lemur.mdx", cursor: [1,45]},
    {id: "note-fat-panda.mdx", cursor: [1,45]},
  ]
}

mirza q:
* how do  zoom zoom? I sa
```
