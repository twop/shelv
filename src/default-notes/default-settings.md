## Settings

Settings are ```` ```js ```` (or ```` ```ts ````) blocks in this note: each block's `export default`
adds to Shelv's defaults, and changes apply immediately. Shelv's own shortcuts (`Cmd B` for bold,
`Cmd Option S` to show or hide Shelv, ...) are always there; bind the same key to replace one.

Keys: `Cmd`, `Ctrl`, `Option`, `Shift`, then one key: `A`-`Z`, `0`-`9`, `Enter`, `Comma`, `/`, ...

## Custom commands

A command that wraps the selection in `[]`, adds `()` and puts the cursor inside, on `Cmd K` and in the
slash palette as `/link`:

```js
import { action, command, selection, Icon } from "shelv";

export const insertLink = command("link", {
  icon: Icon.phosphor("link"), // names: https://phosphoricons.com
  description: "Insert Markdown Link",
  needs: [selection], // the selected text, "" when nothing is selected
  run: (text) => action.InsertText({ text: `[${text}]({||})` }),
});

export default {
  commands: [insertLink],
  bindings: [{ key: "Cmd K", command: insertLink }],
};
```

*Note:* in inserted text, `{||}` puts the cursor there, and `{|}this will be selected{|}` selects text.

## Moving a default shortcut

```js
import * as defaults from "shelv/defaults";

export default {
  bindings: [
    // { key: "Cmd Shift B", command: defaults.bold }, // bold moves from Cmd B to Cmd Shift B
    // { key: "Cmd P", command: null },                 // removes the "pin" shortcut
  ],
};
```
