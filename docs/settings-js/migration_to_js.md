# Porting your settings from KDL to JavaScript

Shelv's settings note no longer reads ```` ```kdl ```` blocks. Settings are now written in
```` ```js ```` blocks in the same note. KDL blocks are not deleted; they are simply ignored, so bindings
defined in them **stop working until you port them**. Shelv's own default shortcuts keep working: they
no longer come from your note.

Specification: [`spec.md`](spec.md).

## 1. The short version

| KDL | JavaScript |
|---|---|
| `global "Cmd Option S" { ShowHideApp; }` | `const showHide = command("show-hide", { run: action.ShowHideApp })`, then `global: [{ key: "Cmd Option S", command: showHide }]` |
| `bind "Cmd B" { MarkdownBold; }` | `const bold = command("bold", { run: action.MarkdownBold })`, then `bindings: [{ key: "Cmd B", command: bold }]` |
| a built-in such as `MarkdownBold` | the action `action.MarkdownBold`, used only inside a command's `run` |
| `MarkdownCodeBlock lang="js"` | `action.MarkdownCodeBlock({ lang: "js" })` |
| `SwitchToNote 0` | `action.SwitchToNote({ index: 1 })` (**1-based now**) |
| `bind ... alias="x" icon="clock" description="..." { X }` | `const x = command("x", { icon: Icon.phosphor("clock"), description: "...", run: action.X })`, listed in `commands: [x]` |
| `bind alias="x" { X }` (no key) | `commands: [command("x", { run: action.X })]` (palette only, no binding) |
| `InsertText { string "hi{\|\|}" }` | `run: action.InsertText({ text: "hi{\|\|}" })` |
| `InsertText { callFunc "f" }` | `run: () => action.InsertText({ text: f() })` |
| `InsertText { callFunc "f" { selection } }` | `needs: [selection], run: (s) => action.InsertText({ text: f(s) })` |
| `{{selection}}` inside a string | `needs: [selection]` and a template literal |
| (not possible in KDL) moving or removing a default key | `{ key: "Cmd Shift B", command: defaults.bold }` (moves it), `{ key: "Cmd P", command: null }` (removes it), `useDefaults: false` (no defaults at all) |
| `ai { model "..."; token "..."; ... }` | `ai: { model: "...", token: secret("name"), ... }` (or a plain string) |

Every block that contributes settings ends with an `export default { ... }`, and imports what it uses
from `"shelv"`.

## 2. Step by step

1. Open the settings note (`⌘ ,`).
2. For each ```` ```kdl ```` block, write a ```` ```js ```` block with the equivalent `export default`.
   Use the table above and sections 3-6. You can also merge a KDL block into the ```` ```js ```` block
   that defines its functions. A block can also be ```` ```ts ````: Shelv strips the types (it does not
   check them) and runs it the same way ([`spec.md`](spec.md) section 2.1).
3. Check the ✓ annotation next to each block. It lists what the block contributes, for example
   `Contributes: 2 bindings`. A ✗ shows the first error with a path such as `bindings[1].key`.
4. Delete the old ```` ```kdl ```` blocks, or leave them; they have no effect either way.

## 3. Bindings, commands and actions

```kdl
// before
global "Cmd Ctrl Option Shift S" { ShowHideApp }
bind "Cmd Shift B" { MarkdownCodeBlock; }
bind "Cmd 1" { SwitchToNote 0; }
```

```js
// after
import { action, command } from "shelv";

const showHide = command("show-hide", { run: action.ShowHideApp });
const codeBlock = command("code-block", { description: "Toggle Code Block", run: action.MarkdownCodeBlock });
const shelf1 = command("shelf-1", { run: action.SwitchToNote({ index: 1 }) });

export default {
  global: [
    { key: "Cmd Ctrl Option Shift S", command: showHide },
  ],
  bindings: [
    { key: "Cmd Shift B", command: codeBlock },
    { key: "Cmd 1",       command: shelf1 },
  ],
};
```

- **You bind commands, not built-ins.** A KDL built-in such as `MarkdownCodeBlock` is now an *action*,
  `action.MarkdownCodeBlock`, and a binding takes a command that runs it:
  `command(alias, { run: action.X })`. The alias names the command (in errors, and in the palette if
  you list it in `commands`); `description` is optional and defaults to the alias.
- Shortcut strings are unchanged (`"Cmd Shift B"`).
- Built-in names are unchanged, with `action.` in front. Parameters move into an object:
  - `MarkdownCodeBlock lang="js"` becomes `action.MarkdownCodeBlock({ lang: "js" })`;
  - `SwitchToNote 0` becomes `action.SwitchToNote({ index: 1 })`. **The index is now 1-based**, matching
    the shelf labels: `index: 1` is Shelf 1 (KDL's `0`). Valid values are `1`-`4`.
- The full list of actions is in [`spec.md`](spec.md) section 5.
- `ShowHideApp` is still the only global action; a command that runs it can only be bound in `global`.
- **You no longer need to restate defaults.** `Cmd B` for bold, `Cmd Option S` to show or hide Shelv and
  the other defaults are always present. Binding the same key in your note replaces the default.

### Moving or removing a default shortcut

In KDL, binding a built-in to a new key kept its default key too, and a default could not be removed.
Now the default commands can be imported, and binding one **moves** it:

```js
import * as defaults from "shelv/defaults";

export default {
  bindings: [
    { key: "Cmd Shift B", command: defaults.bold }, // bold is on Cmd Shift B only (Cmd B is free)
    { key: "Cmd P", command: null },                 // remove the "pin" shortcut
  ],
};
```

- Only the default command itself moves (`defaults.bold`); a new `command("bold", ...)` of your own is a
  different command, and the default keeps `Cmd B`.
- To keep the default key and add another, bind `defaults.bold` to both keys.
- `useDefaults: false` drops all defaults, including `Cmd Comma` (settings) and `Tab` / `Enter` in
  lists; bind back the ones you want from `defaults`. See [`spec.md`](spec.md) section 2.

## 4. Palette metadata: `alias`, `icon`, `description`

In KDL these were attributes on `bind`. Now they belong to a command, created with
`command(alias, { ... })`, and the palette lists the commands in `commands`:

```kdl
// before
bind "Cmd Shift J" icon="\u{EB24}" alias="js" description="Insert javascript code block" {
    MarkdownCodeBlock lang="js"
}
```

```js
// after
import { action, command, Icon } from "shelv";

export const jsBlock = command("js", {
  icon: Icon.phosphor("file-js"),
  description: "Insert javascript code block",
  run: action.MarkdownCodeBlock({ lang: "js" }),
});

export default {
  commands: [jsBlock],                               // in the palette as "js"
  bindings: [{ key: "Cmd Shift J", command: jsBlock }],
};
```

- **Icons:**
  - Use `Icon.phosphor("name")` with the kebab-case name from <https://phosphoricons.com>
    (`icon="calendar-heart"` becomes `Icon.phosphor("calendar-heart")`).
  - If your KDL used a glyph such as `"\u{EB24}"`, look up its name: `\u{EB24}` is `file-js`.
  - Unknown names are reported when the settings load.
- **The palette is a list now.** In KDL a `bind` with an `alias` was in the palette. Now a command is in
  the palette when it is listed in `commands`, in that order; `bindings` only assigns keys.
- **A command without a key:** a KDL `bind` with an `alias` but no shortcut is just an entry in
  `commands` with no binding: `commands: [jsBlock]`.
- **Defaults:** the default icon is `user-circle-gear`, and the default description is the alias.

## 5. `InsertText` and JavaScript functions

`InsertText` becomes `action.InsertText({ text })`. A KDL `callFunc` becomes a function `run` that
**returns** that command.

```kdl
// before
bind "Cmd Y" icon="calendar-heart" alias="date" description="Insert current date (YYYY/mon/DD)" {
    InsertText {
        callFunc "getCurrentDate"
    }
}
```

```js
// after (in the block that defines getCurrentDate, or any later block)
export const insertDate = command("date", {
  icon: Icon.phosphor("calendar-heart"),
  description: "Insert current date (YYYY/mon/DD)",
  run: () => action.InsertText({ text: getCurrentDate() }),
});
```

- **Your functions do not change.** `getCurrentDate()` still returns a string. Only the binding changes:
  it calls the function directly instead of naming it in a string, so a typo in the name is now an
  error when the settings load instead of a silent no-op.
- **`callFunc "f" { selection }` becomes `needs: [selection]`.** The selected text is passed to `run` as
  its first argument (`""` when nothing is selected):

  ```js
  export const backticks = command("backticks", {
    icon: Icon.phosphor("code"), description: "Wrap in backticks",
    needs: [selection],
    run: (text) => action.InsertText({ text: wrapInBackticks(text) }),
  });
  ```

- **`callFunc` on an exported string** becomes the string itself:
  `run: action.InsertText({ text: signature })`.
- **Errors are visible now.** If a function throws, or `run` returns something other than a command,
  Shelv shows a notification. Before, nothing happened.

### Magic strings

| Before | After |
|---|---|
| `{||}`: cursor here | unchanged |
| `{|}text{|}`: select `text` | unchanged |
| `{{selection}}`: the selected text | **removed**: take the selection with `needs: [selection]` and build the string in JS |

```kdl
// before
bind "Cmd K" icon="link" alias="link" description="Insert Markdown Link" {
   InsertText {
        string "[{{selection}}]({||})"
    }
}
```

```js
// after
export const insertLink = command("link", {
  icon: Icon.phosphor("link"), description: "Insert Markdown Link",
  needs: [selection],
  run: (text) => action.InsertText({ text: `[${text}]({||})` }),
});
```

## 6. AI settings

```kdl
// before
ai {
    model "gpt-4o-mini"
    systemPrompt "Answer tersely."
    token "sk-..."
    useShelvSystemPrompt #true
}
```

```js
// after
import { secret } from "shelv";

export default {
  ai: {
    model: "gpt-4o-mini",
    systemPrompt: "Answer tersely.",
    token: secret("open_api_token"),
    useShelvSystemPrompt: true,
  },
};
```

- The field names are unchanged. `model` is required whenever you write an `ai` section.
- If several blocks define `ai`, the last one wins, as before.
- Deleting your `ai` section now takes effect immediately: Shelv falls back to its own model. Before,
  the old settings stayed active until restart.
- **Move the token out of the note** with `secret("name")` (import `secret` from `"shelv"`). Shelv
  resolves it when it makes an AI request; the token never appears in the note or in JS
  ([`spec.md`](spec.md) section 4.1). Secrets live in the macOS keychain; store one with
  `security add-generic-password -s Shelv -a open_api_token -w`. A plain string still works.

## 7. Sharing code between blocks

Nothing changes:

- Every `export` of a block (other than `export default`) is available in all later blocks without an
  import.
- Each block imports what it needs from `"shelv"` itself.
- A later block must not declare a name that an earlier block already exports.

## 8. Worked example: a complete settings note

### Before

````markdown
## Shortcuts
```kdl
global "Cmd Ctrl Option Shift S" {
    ShowHideApp
}

// Toggle Code Block: Creates or removes code block annotations
bind "Cmd Shift B" { MarkdownCodeBlock; }
```

```kdl
// toggle javascript code block
bind "Cmd Shift J" icon="\u{EB24}" alias="js" description="Insert javascript code block" {
 MarkdownCodeBlock lang="js"
}
```

## Snippets
```js
const monthNames = ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec'];

export function getCurrentDate() {
    const now = new Date();
    const year = now.getFullYear();
    const month = monthNames[now.getMonth()];
    const day = String(now.getDate()).padStart(2, '0');
    return `${year}/${month}/${day}`;
}
```

```kdl
bind "Cmd Y" icon="calendar-heart" alias="date" description="Insert current date (YYYY/mon/DD)" {
    InsertText {
        callFunc "getCurrentDate"
    }
}
```

```js
export function getCurrentTime() {
    const now = new Date();
    let hours = now.getHours();
    const minutes = String(now.getMinutes()).padStart(2, '0');
    const ampm = hours >= 12 ? 'pm' : 'am';
    hours = hours % 12;
    hours = hours ? hours : 12;
    return `${hours}:${minutes}${ampm}`;
}
```

```kdl
bind "Cmd T" icon="clock" alias="time" description="Insert current time (hh:mm am/pm)" {
    InsertText {
        callFunc "getCurrentTime"
    }
}
```

```kdl
bind "Cmd K" icon="link" alias="link" description="Insert Markdown Link" {
   InsertText {
        string "[{{selection}}]({||})"
    }
}
```

```js
export function wrapInBackticks(selectedText) {
    if (!selectedText) {
        return "`{||}`";
    }
    return `\`${selectedText}\`{||}`
}
```

```kdl
bind "Cmd /" icon="code" alias="backticks" description="Wrap in backticks" {
    InsertText {
        callFunc "wrapInBackticks" {
            selection
        }
    }
}
```
````

### After

Each KDL block merges into the JS block that belongs to it. The functions are unchanged.

````markdown
## Shortcuts
```js
import { action, command, Icon } from "shelv";

// toggle javascript code block
export const jsBlock = command("js", {
  icon: Icon.phosphor("file-js"), description: "Insert javascript code block",
  run: action.MarkdownCodeBlock({ lang: "js" }),
});

const codeBlock = command("code-block", {
  description: "Toggle Code Block: Creates or removes code block annotations",
  run: action.MarkdownCodeBlock,
});

const showHide = command("show-hide", { run: action.ShowHideApp });

export default {
  commands: [jsBlock],
  global: [
    { key: "Cmd Ctrl Option Shift S", command: showHide },
  ],
  bindings: [
    { key: "Cmd Shift B", command: codeBlock },
    { key: "Cmd Shift J", command: jsBlock },
  ],
};
```

## Snippets
```js
import { action, command, Icon } from "shelv";

const monthNames = ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec'];

export function getCurrentDate() {
    const now = new Date();
    const year = now.getFullYear();
    const month = monthNames[now.getMonth()];
    const day = String(now.getDate()).padStart(2, '0');
    return `${year}/${month}/${day}`;
}

export const insertDate = command("date", {
  icon: Icon.phosphor("calendar-heart"), description: "Insert current date (YYYY/mon/DD)",
  run: () => action.InsertText({ text: getCurrentDate() }),
});

export default { commands: [insertDate], bindings: [{ key: "Cmd Y", command: insertDate }] };
```

```js
import { action, command, Icon } from "shelv";

export function getCurrentTime() {
    const now = new Date();
    let hours = now.getHours();
    const minutes = String(now.getMinutes()).padStart(2, '0');
    const ampm = hours >= 12 ? 'pm' : 'am';
    hours = hours % 12;
    hours = hours ? hours : 12;
    return `${hours}:${minutes}${ampm}`;
}

export const insertTime = command("time", {
  icon: Icon.phosphor("clock"), description: "Insert current time (hh:mm am/pm)",
  run: () => action.InsertText({ text: getCurrentTime() }),
});

export default { commands: [insertTime], bindings: [{ key: "Cmd T", command: insertTime }] };
```

```js
import { action, command, selection, Icon } from "shelv";

export const insertLink = command("link", {
  icon: Icon.phosphor("link"), description: "Insert Markdown Link",
  needs: [selection],
  run: (text) => action.InsertText({ text: `[${text}]({||})` }),
});

export default { commands: [insertLink], bindings: [{ key: "Cmd K", command: insertLink }] };
```

```js
import { action, command, selection, Icon } from "shelv";

export function wrapInBackticks(selectedText) {
    if (!selectedText) {
        return "`{||}`";
    }
    return `\`${selectedText}\`{||}`
}

export const backticks = command("backticks", {
  icon: Icon.phosphor("code"), description: "Wrap in backticks",
  needs: [selection],
  run: (text) => action.InsertText({ text: wrapInBackticks(text) }),
});

export default { commands: [backticks], bindings: [{ key: "Cmd /", command: backticks }] };
```
````

## 9. Behaviour differences to be aware of

| | KDL | JavaScript |
|---|---|---|
| Defaults | built into Shelv; the default note repeated the global hotkey | built into Shelv (base layer), including the global hotkey |
| Failing function | silently does nothing | notification with the error |
| Function name typo | error when the settings load | `ReferenceError` when the settings load |
| `{{selection}}` | substituted in all inserted text | removed (`needs: [selection]`) |
| Palette entries | every `bind` with an `alias` | the commands listed in `commands`, in that order |
| What a binding takes | a built-in (`{ MarkdownBold; }`) | a command (`command("bold", { run: action.MarkdownBold })`) |
| Several actions on one key | not supported ("only 1 command per binding") | `run: [action.A, action.B]` |
| Binding a default built-in to a new key | added a key; the default key stayed | `defaults.<name>` moves to the new key |
| Removing a default shortcut | not possible | `{ key, command: null }`, or `useDefaults: false` for all of them |
| Removing a global hotkey | stayed registered until restart | released immediately |
| Removing the `ai` section | stayed active until restart | default model immediately |
| Invalid entry in a block | the rest of that block was applied up to the error | nothing from that block is applied |

## 10. Troubleshooting

| Annotation | Fix |
|---|---|
| `bindings[0].command: expected a command, got a function` | wrap the function: `command("name", { run: () => action.InsertText({ text: f() }) })` |
| `bindings[0].command: expected a command, got an action` | wrap it: `command("bold", { run: action.MarkdownBold })` |
| `global[0].command: expected a global command` | a global binding needs a command whose run is `action.ShowHideApp` |
| `command(): the alias is now the first argument: command("x", { ... })` | move `alias` out of the object: `command("x", { run })` |
| `...run: expected an action, got a string` (notification) | return `action.InsertText({ text })`, not the string |
| `...needs: needs requires a function run` | `needs` only works with `run: (...) => ...` |
| `...: action.SwitchToNote needs { index }` | call it: `action.SwitchToNote({ index: 1 })` |
| `...needs[0]: expected a capability, got a string` | `needs` takes only capabilities such as `selection`; use constants directly in `run` |
| `...icon: unknown icon "calender"` | check the name on phosphoricons.com |
| `SyntaxError: duplicate ...` | a name is exported by an earlier block; rename it or reuse the earlier export |
