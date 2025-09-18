## Shortcuts
```settings
// All changes inside this block will apply immediately!

// Format: `global "[key1] [key2] ..." {ShortcutName;}`
// Alt keys: Shift (⇧), Cmd (⌘), Option (⌥), Ctrl (⌃), Enter (⏎)
// Alpha-numeric keys: A-Z or `0-9`

global "Cmd Option S" {ShowHideApp;}
```

## Custom commands

Here is an example of a simple custom command that wraps selection with `[]`, adds `()` and sets the cursor inside
- `{selection}` -> the currently selected text in a note, can be empty
- `{||}` -> cursor with no selection
- `{|}this will be selected{|}` -> cursor with selection

```kdl
// (Cmd K): Insert Markdown Link
// you can find more icons unicode symbols here: https://phosphoricons.com/ 
bind "Cmd K" icon="\u{E2E2}" alias="link" description="Insert Markdown Link" {
   InsertText {
        as_is "[{{selection}}]({||})"
    }
}
```
