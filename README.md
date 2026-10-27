<h1 align="center">
    <b>
        <a href="https://shelv.app" style="vertical-align: middle;">Shelv</a>
    </b>
</h1>

<h3 align="center">
    {  Hackable, Local, AI-powered notes  }
</h3>

<p align="center">
    Shelv is scriptable, plain text notes app with integrated ai-features for macOS, written in Rust (btw (tm)).
</p>

<p align="center">
    <a href="https://shelv.app"><b>Website</b></a> •
    <a href="https://discord.gg/PJhJgugJ9U"><b>Discord</b></a> •
    <a href="https://apps.apple.com/us/app/shelv-notes/id6499478682"><b>Mac App Store</b></a>
</p>

![Shelv app showing AI-powered quick prompt feature in action](distribution/assets/for_readme/github-composition.png)


<p align="center">
  <a href="https://apps.apple.com/us/app/shelv-notes/id6499478682">
    <img src="site/assets/media/mac-app-store-badge.svg" alt="Download on the Mac App Store">
  </a>
</p>


### Make It Yours

Settings in Shelv is just a note, where you can create custom commands in JavaScript (or TypeScript), assign and tweak keyboard shortcuts, all with live reload.

**The origin story:** at the time I used [Bear app](https://bear.app/), which had 4 different formats of dates, but I wanted `YYYY/mmm/dd`, and I kept thinking: "if only I could _define_ what I want". Well, with Shelv you can.


---

## Features

- **Markdown Support**: Full CommonMark with extensions, including TODOs
- **Code Syntax Highlighting**: A lot of languages are supported
- **Live JavaScript Blocks**: Execute JS code directly in notes
- **Slash Menu**: Quick access to all commands and features
- **Keyboard minded**: Most actions are available via shortcuts
- **Quick Prompt**: You can ask AI about your text selection, with the note as the context (the models are also configurable!)

### Markdown stuff
1. lists,**bold**, *italic*, ~strikethrough~
	* [ ] todos, `monospace`

```rs
println!("Just syntax highlighted")
```

### Live JS code blocks

```js 1
console.log('I can be run!');
new Map().set("with", "live reload")
```

will produce this output if you press "Run" button that will appear on the left of JS code blocks.
```js 1 > #e904
I can be run!
Map { "with" → "live reload" }
```

---

## Default Shortcuts

These keybindings work out of the box. They are defined in JavaScript, like your own settings, in
[`src/scripting/default-settings.ts`](src/scripting/default-settings.ts); bind the same key in the settings
note to replace one, or move/remove a default (see [Shelv Settings](#shelv-settings)).

| Shortcut | Command | Slash palette |
|---|---|---|
| `⌥ ⌘ B` | Toggle Code Block | `/code` |
| `⌘ B` / `⌘ I` / `⇧ ⌘ E` | Toggle Bold / Italic / Strikethrough | `/bold`, `/italic`, `/strike` |
| `⌥ ⌘ 1` / `2` / `3` | Heading 1 / 2 / 3 | `/h1`, `/h2`, `/h3` |
| `⌃ Enter` | Show AI Prompt | `/ai` |
| `⌘ J` | Word Jump (jump to any word with 2-character labels) | `/jump` |
| `⌥ O` / `⌥ I` | Expand / Shrink Selection | |
| `⌥ K` / `⌥ J` | Move Lines Up / Down | |
| `Tab` / `⇧ Tab` / `Enter` | Indent / unindent / split list items | |
| `/` | Slash palette | |
| `⌘ 1`-`⌘ 4` | Shelf 1-4 | |
| `⌘ ,` | Open Settings | |
| `⌘ P` | Toggle Always on Top | |
| `⌘ O` / `⌘ W` | Open File / Close File | |
| `Escape` | Hide Window | |
| `⌥ ⌘ S` (system-wide) | Show / Hide Shelv | |

---

## Word Jump Navigation

**Introduced in 1.4.0**

Word Jump is a cool navigation feature that allows you to quickly jump to any word in your current note using 2-character keyboard sequences, 
so you don't have to touch your mouse or trackpad for large cursor moves.

Read more on the website [here](https://shelv.app/updates/1_4_0)

Inspired by [Vimium](https://vimium.github.io/) and [Helix](https://helix-editor.com/)

<p align="center">
      <img src="distribution/assets/for_readme/word_jump/word-jump-before.png" alt="Word Jump before activation" width="300"/>
      <span>Press `⌘ J`</span>
      <img src="distribution/assets/for_readme/word_jump/word-jump-after.png" alt="Word Jump after activation showing letter labels" width="300"/>
</p>

### How to Use

1. **Activate Word Jump**: Press `⌘ J` (or your custom keybinding), or by `/jump` slash command
2. **See Jump Labels**: Letter combinations appear next to words throughout your note
3. **Jump**: Type the 2-character sequence to instantly jump to that word
4. **Cancel**: Press `Escape` or any invalid sequence to exit Word Jump mode

---

## Mini Documentation

### Shelv Settings

Settings are defined in the settings note and can be accessed by clicking a gear button on the bottom bar (or `⌘ + ,`).

Every ```` ```js ```` or ```` ```ts ```` code block in the settings note is an ES module, evaluated top to
bottom on every edit. A block's `export default` adds settings; its other exports (functions, constants)
are available in the blocks below without importing. TypeScript blocks have their types stripped (not
checked). The API is typed in [`src/scripting/system.d.ts`](src/scripting/system.d.ts); how it fits together is in [`docs/settings-js/codegen.md`](docs/settings-js/codegen.md).

#### Example: a custom command

Wraps the selection in a Markdown link and puts the cursor inside, on `⌘ K` and as `/link` in the slash palette:

```js
import { action, command, selection, Icon } from "shelv";

export const insertLink = command("link", {
  icon: Icon.phosphor("link"), // https://phosphoricons.com
  description: "Insert Markdown Link",
  needs: [selection], // the selected text, "" when nothing is selected
  run: (text) => action.InsertText({ text: `[${text}]({||})` }),
});

export default {
  commands: [insertLink],                            // in the slash palette
  bindings: [{ key: "Cmd K", command: insertLink }], // on a shortcut
};
```

- `command(alias, { icon?, description?, needs?, run })`: `run` is an action (`action.MarkdownBold`), an
  array of actions run in order, or a function returning either.
- In inserted text, `{||}` puts the cursor there and `{|}this will be selected{|}` selects text.

#### Example: inserting the current day of the week

```js
export function getCurrentDayOfWeek() {
  const days = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
  return days[new Date().getDay()];
}
```

```js
import { action, command, Icon } from "shelv";

// getCurrentDayOfWeek comes from the block above
export const insertDay = command("day", {
  icon: Icon.phosphor("calendar"),
  description: "Insert current day of the week",
  run: () => action.InsertText({ text: getCurrentDayOfWeek() }),
});

export default { commands: [insertDay], bindings: [{ key: "Cmd D", command: insertDay }] };
```

#### Example: moving and removing default shortcuts

```js
import * as defaults from "shelv/defaults";

export default {
  bindings: [
    { key: "Cmd Shift B", command: defaults.bold }, // bold moves from ⌘ B to ⇧ ⌘ B
    { key: "Cmd P", command: null },                 // no shortcut for "Always on Top"
  ],
  global: [{ key: "Cmd Ctrl Option Shift S", command: defaults.showHide }],
  // useDefaults: false, // start from no defaults at all
};
```

### Supported AI Providers

Shelv supports multiple AI providers through the [rust-genai](https://github.com/jeremychone/rust-genai) library:

- **OpenAI**: Models start with "gpt" (e.g., "gpt-4o-mini", "gpt-4")
- **Anthropic**: Models start with "claude" (e.g., "claude-3-haiku-20240307", "claude-3-5-sonnet-20240620")  
- **Cohere**: Models start with "command" (e.g., "command-light")
- **Gemini**: Models start with "gemini" (e.g., "gemini-2.0-flash")
- **Groq**: Specific model names (e.g., "llama-3.1-8b-instant")
- **Ollama**: Local model names (e.g., "gemma:2b")
- **XAI/Grok**: Specific model names (e.g., "grok-beta")
- **DeepSeek**: Specific model names (e.g., "deepseek-chat")

### Settings Schema

```typescript
export default {
  useDefaults?: boolean,              // false: drop all of Shelv's defaults
  commands?: Command[],               // slash palette entries, in order
  bindings?: { key, command }[],      // in-app shortcuts; command: null removes the key
  global?: { key, command }[],        // system-wide; only commands running action.ShowHideApp
  ai?: {
    model: string,                    // see providers above; default: Shelv's rate limited model
    systemPrompt?: string,            // extra instructions
    token?: string | Secret,          // secret("name") reads the macOS keychain (service "Shelv")
    useShelvSystemPrompt?: boolean,   // default true
  },
};
```

- **Keys:** `"Cmd Shift B"`: modifiers `Cmd`, `Option`, `Shift`, `Ctrl`, then one key.
- **Actions** (`action.*`): `InsertText({ text })`, `MarkdownBold`, `MarkdownItalic`,
  `MarkdownStrikethrough`, `MarkdownCodeBlock` / `MarkdownCodeBlock({ lang })`, `MarkdownH1`-`H3`,
  `IndentListItem`, `UnindentListItem`, `SplitListItem`, `ExpandTaskMarker`, `ExpandSelection`,
  `ShrinkSelection`, `MoveLinesUp`, `MoveLinesDown`, `StartWordJump`, `ShowSlashPalette`, `ShowPrompt`,
  `SwitchToNote({ index: 1..4 })`, `SwitchToSettings`, `OpenFileDialog`, `CloseCurrentNote`,
  `PinWindow`, `HideApp`, `ToggleDebugTools`, and for `global` only: `ShowHideApp`.
- Typings for editors: [`src/scripting/system.d.ts`](src/scripting/system.d.ts) (map `"shelv"` to it in a tsconfig).
- Coming from the old KDL settings? See [`docs/settings-js/migration_to_js.md`](docs/settings-js/migration_to_js.md).

---

## Frequently Asked Questions

<details>
<summary>I'm sick of AI hype, is Shelv yet another AI-'something'?</summary>

I hope not, my opinions toward AI (or rather LLMs) are mixed. My current position can be roughly outlined as:
- AI is not a "higher-level abstraction" like programming languages over assembly. For a simple reason: it is not deterministic.
- Using AI may and likely will cause skill degradation if used as a solo replacement for typing code or writing prose (like "vibe coding").
- I think the best use of it (at the moment) is if you can expertly assess the output. So the UX I'm leaning towards will try to emphasize that idea.
- Luckily, working with text, adding small scripts, etc. qualifies as such.
- But moreover, I think it can be used as a discovery tool - try asking with a quick prompt, "What are the current keybindings?"

I hope I've convinced you to give Shelv a try.

</details>

<details>
<summary>Is Shelv coming to Mobile/Window/Web?</summary>

Yes, but with time (and perhaps your help). Shelv is written in Rust + [egui](https://egui.rs/), which has some cross-platform support.

</details>

<details>
<summary>How do you make money?</summary>

I don't. I worked on Shelv for over 2 years, and I had a dream to start company (still do), but as of now, it's a labor of love. In the future, there may be a way to purchase AI tokens. Currently, Shelv uses my personal token with a cap, but you can always configure your own provider (including [Ollama](https://ollama.com/).

</details>

<details>
<summary>Does Shelv have sync functionality?</summary>

Not yet, I'm a local first movement fan, and wanted to use [Automerge](https://github.com/automerge/automerge) since forever, ideally with e2e encryption with a pure Rust server. This is something that's being worked on right now, but e2e encrypted scalable sync ain't easy both technically and from a product perspective.

</details>

<details>
<summary>Is Shelv open source?</summary>

Yes and no, Shelv is licensed under the [Komorebi License](https://github.com/LGUG2Z/komorebi-license) 2.0.0, so more like "source available".

This limits you from compiling Shelv for work or repackaging it to a new app, but you can fork it for non-commercial personal use or with the intention of contributing changes back.
However you can (and hopefully always) just use the version from the app store, which grants **personal** commercial use (such as using it at work for productivity). 


Read more [below](#License)

</details>

<details>
<summary>Is it Native?</summary>

Native is a spectrum, Shelv is written in Rust using [egui](https://egui.rs/) as the gui toolkit, not Swift UI tech stack. Maybe the closest analogy would be [Flutter](https://flutter.dev/) that is painting every pixel. Are Flutter apps native? I think so.

</details>

<details>
<summary>Are my beloved vim motions supported?</summary>

I am a [Helix](https://helix-editor.com/) user myself, but markdown and text are a bit different from code. That said, I would love to support modal editing in the future. Perhaps some features can be added for "insert" mode (which is the only mode at the moment) that can enhance editing, for example: jump to a word, press any buttons with a label(vimium style), expand + shrink semantic selection etc. I need to work on Shelv full-time to justify adding vim or helix motions to egui TextEdit, vote with your money I guess, oh wait, I don't have a way to actually recieve money...

</details>

<details>
<summary>Are you collecting any analytics?</summary>

Not at the moment (besides crash reporting), but I'm not fundamentally opposed to collecting statistics, because it is hard to know if a feature is even used without some observability. I do think it can be done with privacy in mind (at least anonymizing and being mindful of where the data is stored). Probably in the future, however, when and if I add monetization, I'll likely start collecting emails associated with a purchase and/or install.

</details>

---

## Roadmap

**Done:**
- Initial launch on macOS: Sep 2025
  - Barebones editing with 4 notes
  - Optimized for quick capture
  - No Shelv API exposed to JS scripts

**Coming:**
- Multi-file + workspace support
  - Workspace folder with notes inside
  - Import from [Obsidian](https://obsidian.md/)
  - File tree + workspace viewer
- Agentic mode
  - Tools/MCP that allow to search/move/create/edit notes
  - UI for having agentic workflows, probably just a chat that is going to be just another file
  - Files that define custom workflows, similar to Claude Code
- Core editing features:
  - Semantic selection: expand and shrink cursor selection with markdown AST nodes
  - Jump to an *element*, jump to any word on the screen with a couple of keystrokes (similar to Vimium and Helix)
  - Search, Redo etc
- Support for pasting/rendering images
- Rich API exposed to JS + better scripting capabilities (like sharing code among notes)
- Sync 
  - I plan to use [Automerge](https://github.com/automerge/automerge) for personal syncing, which can be also used for collaboration
  - Dump to git, e.g. backup all the notes to git, potentially with AI-generated change summary
- Web version
  - Mobile (including web) version is TBD
- Collaboration
  - Share a note via link (co-editing on the web)
  - Share workspace, that is, co-ownership of a collection of folder+notes

---

## Building and Running

### Prerequisites
- Rust (latest stable version)
- macOS (for now only on macOS, I wish I would have enough money and time to make Windows, Linux, Web and mobile versions! But alas)

### Build Commands

```bash
# Run the application
cargo run

# Run tests
cargo test
```

Yep, that is it.

### Shelv is built with:
- **[Rust](https://github.com/rust-lang/rust)** for core application logic
- **[egui](https://github.com/emilk/egui) + [eframe](https://github.com/emilk/egui/tree/master/crates/eframe)** for cross-platform GUI
- **[Boa engine](https://github.com/boa-dev/boa)** for JavaScript runtime in code blocks
- **[pulldown-cmark](https://github.com/raphlinus/pulldown-cmark)** for Markdown parsing
- **[syntect](https://github.com/trishume/syntect)** for syntax highlighting
- **[global-hotkey](https://github.com/tauri-apps/global-hotkey)** for system-wide shortcuts
- **[tray-icon](https://github.com/tauri-apps/tray-icon)** for system tray functionality
- **[genai](https://github.com/jeremychone/rust-genai)** for LLM integration
- **[swc](https://github.com/swc-project/swc)** (`swc_ts_fast_strip`) for running TypeScript settings and code blocks

---

## Links

- **[Homepage](https://shelv.app)** - sources under `/site` folder
- **[Discord](https://discord.gg/PJhJgugJ9U)** - ask for help, give feedback, or just say "hi"!

[![Download on the Mac App Store](site/assets/media/mac-app-store-badge.svg)](https://apps.apple.com/us/app/shelv-notes/id6499478682)

---

## License

### Summary

- You can fork Shelv for the intended purpose of contributing features upstream or for non-commercial use only
- You can use the Mac App Store version for personal use at work or home freely
- In the future, there might be a team license available for purchase separately

*The overall structure is inspired by the [komorebi project](https://github.com/LGUG2Z/komorebi). Thank you [LGUG2Z](https://github.com/LGUG2Z) for being an inspiration!*

### Komorebi License

Shelv is licensed under the [Komorebi License](https://github.com/LGUG2Z/komorebi-license) 2.0.0. 

This license does not permit any kind of commercial use. If you are a business owner, the act of forking this repository, making changes to it and then using your fork in your business constitutes commercial use.

If you would like to discuss commercial licensing, feel free to reach out.

### Personal Commercial Use

Personal commercial use (such as using Shelv for personal productivity while at work) is automatically granted with the Mac App Store version of Shelv.

