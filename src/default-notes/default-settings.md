## Shortcuts
```kdl
// All changes inside kdl and js blocks will apply immediately!

// Format: `global "[key1] [key2] ..." {ShortcutName;}`
// Alt keys: Shift (⇧), Cmd (⌘), Option (⌥), Ctrl (⌃), Enter (⏎)
// Alpha-numeric keys: A-Z or `0-9`

global "Cmd Option S" {ShowHideApp;}
```

## Custom commands

Here is an example of a simple custom command that wraps selection with `[]`, adds `()` and sets the cursor inside

```kdl
// (Cmd K): Insert Markdown Link
// you can find more icon names here: https://phosphoricons.com/ 
bind "Cmd K" icon="link" alias="link" description="Insert Markdown Link" {
   InsertText {
        string "[{{selection}}]({||})"
    }
}
```
*Note:*
- `{{selection}}` -> the currently selected text in a note, can be empty
- `{||}` -> cursor with no selection
- `{|}this will be selected{|}` -> cursor with selection
