# Shelv settings in JavaScript: v1 API spec

Status: **specification, not implemented.** Companion docs: [`plan.md`](plan.md) (how we get there),
[`migration_to_js.md`](migration_to_js.md) (porting an existing KDL settings note).

v1 is a **port**: everything the KDL settings could express today, expressed in JS, and nothing more.
Section 10 outlines what comes later, so that v1 names do not block it.

## 1. Where settings live

- Settings stay in the **settings note** (`settings.md`, `⌘ ,`), edited in Shelv like any note.
- Every ```` ```js ```` or ```` ```ts ```` code block in the settings note is a settings block. Blocks
  are evaluated top to bottom on every edit, exactly as today.
- **TypeScript blocks** (section 2.1) have their types stripped, not checked: Shelv runs them as
  JavaScript. Type errors show in an editor or with `tsc` (map `"shelv"` to `system.d.ts`), not in the
  ✓/✗ annotation.
- Each block shows its result next to it (the existing ✓/✗ annotation):
  - ✓ `Contributes: 4 bindings, 1 global · Shares: getCurrentDate, wrapInBackticks`
  - ✗ the first error, with a path into the settings value (section 9).
- ```` ```kdl ```` blocks are **ignored**: they are ordinary code blocks with no effect.

## 2. Blocks are ES modules

- Each block is an ES module.
- **`export default`** is the block's settings contribution (section 4). A block without a default export
  contributes nothing, but can still share code.
- **Every other export** is shared: later blocks can use it without writing an import (Shelv adds
  `import { name } from ...` lines for all exports of earlier blocks, as today). `default` is never
  shared.
  - Consequence: a later block must not declare a name that an earlier block exports.
- **`import { ... } from "shelv"`** gives access to the API (section 3). Each block imports what it uses.

### 2.1 TypeScript blocks

```ts
import { action, command, selection, type Settings } from "shelv";

export const quote = command("quote", {
  needs: [selection],
  run: (text: string) => action.InsertText({ text: `"${text}"{||}` }),
});

export default { commands: [quote] } satisfies Settings;
```

- Shelv strips the type syntax before evaluating the block (the same kind of type stripping as Node's
  `--experimental-strip-types`); it does not type-check.
- The types are replaced by whitespace, so line and column numbers in errors point at the block as
  written.
- **Erasable syntax only** (TypeScript's `erasableSyntaxOnly`): type annotations, `type` / `interface`,
  `as`, `satisfies`, `import type`, generics. Syntax that needs code generation, `enum`, `namespace`
  with values, and constructor parameter properties, is a load-time error on its line.
- `ts` and `js` blocks mix freely in one note; exports are shared between them as in section 2. Only
  runtime exports are shared: an exported `type` or `interface` exists only for the editor.
- The same applies to ```` ```ts ```` blocks in regular notes, which run like ```` ```js ```` blocks.

```js
import { action, command, selection, Icon } from "shelv";

export function wrapInBackticks(text) {
  return text ? `\`${text}\`{||}` : "`{||}`";
}

export const backticks = command("backticks", {
  icon: Icon.phosphor("code"), description: "Wrap in backticks",
  needs: [selection],
  run: (text) => action.InsertText({ text: wrapInBackticks(text) }),
});

export default {
  commands: [backticks],                            // in the slash palette as "backticks"
  bindings: [{ key: "Cmd /", command: backticks }], // and on Cmd /
};
```

### Evaluation order and merging

1. **Base layer**: Shelv's built-in `default-settings.ts` (section 8), first, unless `useDefaults: false`.
2. The settings note's js blocks, top to bottom.

Contributions merge in that order:

| Field | Merge rule |
|---|---|
| `useDefaults` | last one wins; `false` drops the whole base layer (its `commands`, `bindings` and `global`), default `true` |
| `commands` | appended in order (palette order); a command whose alias is already listed **replaces** the earlier entry in place |
| `bindings`, `global` | appended in order, each binding applying the rules below |
| `ai` | last one wins |

Each binding in the note is applied in order:

1. **Same key: later wins.** A binding on a key that is already bound replaces that binding.
2. **A default command moves.** The first time the note binds a command from `"shelv/defaults"`
   (section 3.1), that command's base-layer bindings are removed. Further bindings of it in the note add
   up, so `{ key: "Cmd Shift B", command: defaults.bold }` moves bold off `Cmd B`, and binding it twice
   gives it two keys. "The same command" means the same value (identity), not the same alias: your own
   `command("bold", ...)` is a different command and leaves the default alone.
3. **`command: null` frees a key**: whatever is bound to it so far (base layer or an earlier block) is
   removed, and nothing replaces it.

```js
import * as defaults from "shelv/defaults";

export default {
  bindings: [
    { key: "Cmd Shift B", command: defaults.bold }, // bold moves: Cmd B is free now
    { key: "Cmd P", command: null },                 // no "pin" shortcut
  ],
};
```

Starting from nothing and picking defaults back:

```js
import * as defaults from "shelv/defaults";

export default {
  useDefaults: false,
  commands: [defaults.bold, defaults.code],
  bindings: [
    { key: "Tab", command: defaults.indent },
    { key: "Enter", command: defaults.splitListItem },
    { key: "Cmd Comma", command: defaults.settings },
  ],
};
```

`useDefaults: false` removes **every** default, including `Cmd Comma` (open settings), `Tab` / `Enter`
in lists and the global show/hide hotkey; bind the ones you want back.

A block either applies completely or not at all: one invalid entry means nothing from that block is
applied, and its annotation shows the error. Other blocks are unaffected.

## 3. The `"shelv"` module (v1)

| Export | Kind | Purpose |
|---|---|---|
| `action` | namespace of built-in actions (what a command's `run` does) | section 5 |
| `command(alias, def)` | constructor | a named command with palette metadata, inputs and `run` (section 6) |
| `selection` | capability | the selected text, for `needs` (section 6.3) |
| `secret(name)` | constructor | a reference to a secret, for `ai.token` (section 4.1) |
| `Icon` | namespace | `Icon.phosphor("name")`, type-checked icon names (section 7) |

Everything exported by `"shelv"` is a **pure constructor or constant**: calling it builds a value and
has no effect on the app. Settings code cannot touch the app, the file system, the network or the
clipboard; Shelv reads the returned values and acts on them.

### 3.1 The `"shelv/defaults"` module

The base layer (section 8) is also a module: every default command is a named export, the **same
values** the base layer binds. Import it as a namespace, since names such as `hide` or `settings` would
otherwise clash with your own:

```js
import * as defaults from "shelv/defaults";
defaults.bold;     // command("bold", { run: action.MarkdownBold, ... }), bound to Cmd B by default
defaults.showHide; // the global command, bound to Cmd Option S by default
```

Use them to move a default (section 2), to put one in your palette (`commands: [defaults.pin]`), or to
pick defaults back after `useDefaults: false`. The names are listed in section 5 (`shelf-1` is
`defaults.shelf1`, `split-list-item` is `defaults.splitListItem`).

Source: `src/scripting/system.ts`, one TypeScript file with the types, the runtime and the generated icon
name union. Shelv embeds it as is and strips its types when it loads (section 2.1). `just build-system`
generates `system.d.ts`, the typings for editors and `tsc` (map `"shelv"` to it with tsconfig `paths`).

## 4. The settings value (`export default`)

```ts
interface Settings {
  useDefaults?: boolean;     // false: no base layer at all (section 2); default true
  commands?: Command[];      // the slash palette, in order
  global?:   GlobalBinding[];
  bindings?: Binding[];
  ai?:       AiSettings;
}

interface Binding {
  key: Shortcut;
  command: Command | null;         // a command(...) value (actions are not bindable on their own); null frees the key
}

interface GlobalBinding {
  key: Shortcut;                   // system-wide hotkey
  command: Command<"global"> | null; // a command whose run is a global action (v1: action.ShowHideApp)
}

interface AiSettings {
  model: string;                   // genai model name; "shelv-claude" is Shelv's proxy model
  systemPrompt?: string;           // extra instructions, added after Shelv's prompt
  token?: string | Secret<string>; // API token: plain string, or secret("name") (section 4.1)
  useShelvSystemPrompt?: boolean;  // default true
}
```

- **Only commands are bound or listed.** `bindings`, `global` and `commands` take `command(...)` values;
  actions (`action.*`, section 5) only appear inside a command's `run`. So every key and palette entry
  has an alias and a description, which Shelv shows in the palette, in hints and in errors, and Shelv
  itself has no built-in descriptions: the base layer writes them out.
- **`commands` and `bindings` are independent.** A command listed in `commands` is in the slash palette;
  a command in `bindings` runs on its key. A command can be in either or both; the palette shows the key
  of the last binding of the same command.
- **Global commands** (`run: action.ShowHideApp`) have the type `Command<"global">` and go only in
  `global`; every other command goes only in `bindings` / `commands`.
- Without any `ai` contribution, Shelv uses its proxy model `"shelv-claude"` (today's behaviour).
- Unknown fields are errors (`bindings[0].comand: unknown field`), so typos do not silently do nothing.

### 4.1 Secrets: `secret(name)`

```js
import { secret } from "shelv";

export default {
  ai: { model: "gpt-4o-mini", token: secret("open_api_token") },
};
```

- `secret(name)` returns an opaque reference. Shelv resolves it in Rust when it makes an AI request.
  The token value **never enters JS** and never appears in the note.
- Type: a branded value that carries the secret's name:

  ```ts
  declare const secretBrand: unique symbol;
  export interface Secret<T extends string, Name extends string = string> {
    readonly [secretBrand]: { value: T; name: Name };
  }
  export function secret<const Name extends string>(name: Name): Secret<string, Name>;
  ```

- A missing secret is an error when the AI request is made (a notification naming the secret), not
  when the settings load.
- **Storage: the macOS keychain.** `secret("open_api_token")` is a generic password item with service
  `Shelv` and account `open_api_token`. Until Shelv has a UI for it, add one from a terminal:

  ```sh
  security add-generic-password -s Shelv -a open_api_token -w   # prompts for the value
  ```

  The first time Shelv reads an item it did not create, macOS may ask to allow access.
- **Placeholder: storing secrets from Shelv.** _To be designed._ A Shelv UI that writes the keychain
  item itself (likely required for the sandboxed App Store build). Until then, use the `security` command
  above.

### Shortcuts

`"Cmd Shift B"`: space-separated modifiers, then exactly one key.

- Modifiers: `Cmd`, `Ctrl`, `Option` (or `Alt`), `Shift`.
- Keys: egui key names: `A`-`Z`, `0`-`9`, `Enter`, `Space`, `Tab`, `Escape`, `Up`/`Down`/`Left`/`Right`,
  `Comma` (or `,`), `Slash` (or `/`), `OpenBracket` (or `[`), `F1`-`F20`, ... Invalid names are
  load-time errors.

## 5. Built-in actions: `action.*`

An action is something Shelv can do: toggle bold, switch to a shelf, hide the window. Actions are the
building blocks of a command's `run` (section 6); they are not bindable on their own. Actions with
parameters are called to create an instance: `action.MarkdownCodeBlock({ lang: "js" })`.

The base layer (section 8) wraps each action in a named command with a description; the table lists
those commands, importable from `"shelv/defaults"` under their camelCase names (section 3.1). To move one,
bind `defaults.<name>` to another key; to replace one, bind its key to your own command; to change its
palette text, list your own command with the same alias in `commands`.

| Action | Parameters | Base-layer command (key) | In the palette |
|---|---|---|---|
| **Text and markdown** | | | |
| `action.InsertText({ text })` | `text: string`, required |  |  |
| `action.MarkdownBold` |  | `bold` (`Cmd B`) | yes |
| `action.MarkdownItalic` |  | `italic` (`Cmd I`) | yes |
| `action.MarkdownStrikethrough` |  | `strike` (`Cmd Shift E`) | yes |
| `action.MarkdownCodeBlock` / `({ lang })` | `lang?: string` | `code` (`Cmd Option B`, no lang) | yes |
| `action.MarkdownH1` / `H2` / `H3` |  | `h1` / `h2` / `h3` (`Cmd Option 1` / `2` / `3`) | yes |
| **Lists** | | | |
| `action.IndentListItem` |  | `indent` (`Tab`) |  |
| `action.UnindentListItem` |  | `unindent` (`Shift Tab`) |  |
| `action.SplitListItem` |  | `split-list-item` (`Enter`) |  |
| `action.ExpandTaskMarker` |  | `task-marker` (`Space`) |  |
| **Structural editing** | | | |
| `action.ExpandSelection` |  | `expand-selection` (`Option O`) |  |
| `action.ShrinkSelection` |  | `shrink-selection` (`Option I`) |  |
| `action.MoveLinesUp` |  | `move-lines-up` (`Option K`) |  |
| `action.MoveLinesDown` |  | `move-lines-down` (`Option J`) |  |
| **Navigation and palette** | | | |
| `action.StartWordJump` |  | `jump` (`Cmd J`) | yes |
| `action.ShowSlashPalette` |  | `slash-palette` (`/`) |  |
| `action.ShowPrompt` |  | `ai` (`Ctrl Enter`) | yes |
| **Notes and window** | | | |
| `action.SwitchToNote({ index })` | `index: NoteIndex` (`1 \| 2 \| 3 \| 4`, 1-based like the shelf labels), required | `shelf-1`-`shelf-4` (`Cmd 1`-`Cmd 4`) |  |
| `action.SwitchToSettings` |  | `settings` (`Cmd Comma`) |  |
| `action.OpenFileDialog` |  | `open-file` (`Cmd O`) |  |
| `action.CloseCurrentNote` |  | `close-file` (`Cmd W`) |  |
| `action.PinWindow` |  | `pin` (`Cmd P`) |  |
| `action.HideApp` |  | `hide` (`Escape`) |  |
| `action.ToggleDebugTools` |  | `debug-tools` (`Ctrl Option Shift D`) |  |
| **Global only** (the whole `run` of a `global` command) | | | |
| `action.ShowHideApp` |  | `show-hide` (`Cmd Option S`, global) |  |

Rules:

- An action with **required** parameters must be called (`action.SwitchToNote` alone is an error).
- An action with **optional** parameters can be used bare (`action.MarkdownCodeBlock` = no language).
- Where an action works (note editor focused, or anywhere in the app) is fixed per action, as today.
  It is not configurable in v1.
- A global action is only valid as the whole `run` of a command (not in an array, not returned by a
  function): `command("show-hide", { run: action.ShowHideApp })`.
- A command bound to several keys has one palette entry, which shows the **last** binding's key.

## 6. `command(alias, def)`

```ts
function command(alias: string, def: {   // alias: the palette name; also names the command in errors
  icon?: PhosphorIconName; // palette icon, via Icon.phosphor(...); default "user-circle-gear"
  description?: string;    // palette and hint text; default: the alias
  needs?: Capability[];    // inputs, resolved when the command runs (only with a function `run`)
  run: Run;
}): Command;               // Command<"global"> when run is a global action (action.ShowHideApp)

type Run =
  | Action                            // run this action
  | Action[]                          // run these, in order
  | ((...inputs) => Action | Action[] | null | undefined);  // sync function called at keypress
  // or, for a global command, exactly one GlobalAction
```

### 6.1 `run` forms

```js
// a value: no logic
export const jsBlock = command("js", {
  icon: Icon.phosphor("file-js"), description: "Insert javascript code block",
  run: action.MarkdownCodeBlock({ lang: "js" }),
});

// several actions in order
export const boldItalic = command("bi", { run: [action.MarkdownBold, action.MarkdownItalic] });

// a function: computed at keypress
export const insertDate = command("date", {
  icon: Icon.phosphor("calendar-heart"), description: "Insert current date",
  run: () => action.InsertText({ text: new Date().toDateString() }),
});
```

- A function `run` is called **synchronously** each time the command runs, with `this` undefined and
  the resolved `needs` as arguments, in order.
- It must return an action or an array of actions. **Strings are not accepted**: text is
  `action.InsertText({ text })`. `null`, `undefined` and an empty array do nothing (handy for
  conditional commands).
- Returning a Promise (an `async` function) is an error in v1; async commands are `flow` (section 10).
- Returning a user `command(...)` is an error in v1.

### 6.2 Arrays run in order

Each action in an array runs against the note as left by the previous one. For example,
`[action.InsertText({ text: "a" }), action.MarkdownBold]` inserts first, then toggles bold. The whole array
is **one undo step**.

### 6.3 `needs`: capabilities for a function `run`

- An array of **capabilities**: inert values from `"shelv"` that describe something Shelv provides
  when the command runs. Entry *i* is resolved and passed as argument *i* of `run`.
- Only capabilities are accepted. Constants do not belong in `needs`: use the value directly in `run`.
- v1 has one capability:

| Capability | Type | `run` receives |
|---|---|---|
| `selection` | `Capability<string>` | the selected text in the current note, `""` when nothing is selected |

```js
export const quote = command("quote", {
  icon: Icon.phosphor("quotes"),
  needs: [selection],
  run: text => action.InsertText({ text: `\"${text}\"{||}` }),
});
```

**Types correlate `needs` with `run`.** A capability carries the type of the value it resolves to, and
`command` maps the `needs` tuple to `run`'s parameter list. A parameter of the wrong type, an extra
parameter, or a constant in `needs` is a type error (checked with `tsc`):

```ts
declare const capabilityBrand: unique symbol;
/** An inert capability: nothing happens until Shelv resolves it for a running command. */
export interface Capability<T> { readonly [capabilityBrand]: T }

export const selection: Capability<string>;

type Resolve<N extends readonly Capability<unknown>[]> = {
  [K in keyof N]: N[K] extends Capability<infer T> ? T : never;
};

export function command<const N extends readonly Capability<unknown>[]>(alias: string, def: CommandMeta & {
  needs: N;
  run: (...inputs: Resolve<N>) => Action | readonly Action[] | null | undefined;
}): Command;

export function command(alias: string, def: CommandMeta & {
  needs?: undefined;
  run: Action | readonly Action[] | (() => Action | readonly Action[] | null | undefined);
}): Command;
```

With `needs: [selection]`, `run: (n: number) => ...` is a type error (`string` is not `number`), and so
are `run: (a, b) => ...` (only one input) and `needs: ["x"]`. Declaring *fewer* parameters is allowed
in TypeScript, so at runtime Shelv warns when `run` declares fewer parameters than `needs` has
entries, and checks that every entry is a capability.

- `needs` with a non-function `run` is an error.

### 6.4 `action.InsertText({ text })`

- Replaces the selection with `text`. With no selection, it inserts at the cursor.
- Cursor markers in `text`:
  - `{||}`: put the cursor here;
  - `{|}` ... `{|}`: select the text between the two markers.
- There is **no `{{selection}}` templating**. To use the selection, take it through `needs: [selection]`.

## 7. Icons

```js
icon: Icon.phosphor("calendar-heart")
```

- `Icon.phosphor(name)` takes a Phosphor icon name (kebab-case, as on <https://phosphoricons.com>) from
  the *light* set Shelv ships (egui-phosphor 0.14, 1530 icons).
- **Type-checked, not looked up in JS**: `system.ts` declares `name` as a union of all valid
  names, so a typo is a type error in an editor or `tsc`. At runtime, `Icon.phosphor` simply returns
  the string, and there is no icon table in JS.
- Shelv still checks the name when the settings load, because blocks are not type-checked in the app:
  an unknown name is a load-time error (`bindings[2].command.icon: unknown icon "calender"`).
- A plain string (`icon: "clock"`) behaves the same at runtime. Prefer `Icon.phosphor(...)`, which is
  what gets type-checked.

## 8. The base layer: `default-settings.ts`

- Shelv's default commands (with their descriptions), bindings and palette entries are defined in
  `src/scripting/default-settings.ts`, which Shelv embeds as is (types stripped at load, section 2.1).
  There are no other built-in defaults: the table in section 5 is that file.
- It is evaluated before the settings note. A default can be replaced (bind its key), moved (bind the
  same command from `"shelv/defaults"` to another key), removed (`{ key, command: null }`), or the whole
  layer dropped (`useDefaults: false`); section 2.
- It is also the module `"shelv/defaults"` (section 3.1).
- It also serves as the **reference**: everything in it is written with the public API, so anything Shelv
  does by default, a user can write too.

## 9. Errors

**Load time** (when the settings note is evaluated). The block shows ✗ and applies nothing. Kinds of error:

- JS errors (syntax, `ReferenceError`, a throw at the top level), with Boa's message;
- invalid settings values, with a path and a reason:
  - `bindings[2].key: "Cmd Shit B": "Shit" is not a key or modifier`
  - `bindings[0].command: expected a command, got a string`
  - `bindings[0].command: expected a command, got an action: wrap it, command("bold", { run: action.MarkdownBold })`
  - `bindings[5].key: required`
  - `global[0].command: expected a global command (run: action.ShowHideApp), got "bold"`
  - `bindings[6].command: "show-hide" is a global command, bind it in global`
  - `bindings[3].command.run: action.SwitchToNote needs { index }`
  - `bindings[4].command.needs[0]: expected a capability, got a string`
  - `bindings[1].command.needs: needs requires a function run`
  - `bindings[1].command.flow: flow is not supported yet`

**Run time** (when a command runs). Shelv shows a notification with the command's alias and the message. Nothing is changed in the note. Kinds of error:

- `run` throws;
- `run` returns something other than an action or an array of actions (a global action included);
- `run` returns a Promise.

Today such failures are silent; in v1 they are visible.

## 10. Not in v1 (names reserved)

These are part of the design and must stay possible.
Where a name is reserved, v1 rejects it with a load-time "not supported yet" error.

| Feature | Shape | v1 |
|---|---|---|
| **Async commands** | `flow: async (...inputs) => { const how = await ask.for("How?"); ... }`, with inert thenable requests | `flow` key reserved |
| **Named `needs`** | `needs: { text: selection.required, ai, ask }` (object form, permissions derived from it) | array only |
| **More capabilities** | `now`, `line`, `note`, `clipboard` (gated), `ai`, `ask` | not exported |
| **More actions** | `action.WrapSelection`, `action.Notify`, `action.Run(command)` (all in `action.*`; no separate `edit.*` namespace) | not exported |
| **Running user commands from `run`** | returning `command(...)` values (later: `action.Run(command)`) | error |

## 11. Open questions

None for v1. Placeholder: the UI for storing secrets (section 4.1).

Decided (formerly open): `SwitchToNote` takes `NoteIndex` (`1`-`4`), and the base layer binds Cmd 1-4;
`run` returning `null`/`undefined` is a no-op; the base layer is not viewable in the app (it is in the
source); secrets live in the macOS keychain; a command bound to several keys shows the **last** binding's
key in the palette; defaults move by identity, `command: null` unbinds and `useDefaults: false` drops the
base layer, with no exception for `Cmd Comma`.
