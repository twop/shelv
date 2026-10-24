# Settings in JavaScript: implementation plan

Status: **plan, nothing implemented.** API: [`spec.md`](spec.md). User-facing porting guide:
[`migration_to_js.md`](migration_to_js.md). Design background: `tmp/settings-js-00` … `08`.

## Goal

Replace KDL in the settings note with JavaScript blocks and remove KDL from Shelv completely, without
adding features: every KDL setting has a JS equivalent, nothing more. The settings note stays a markdown
note edited in Shelv, with the same per-block ✓/✗ annotations.

## Decisions

| Topic | Decision |
|---|---|
| Where settings live | `settings.md`, ```` ```js ```` or ```` ```ts ```` blocks; ```` ```kdl ```` blocks are ignored silently |
| TypeScript | ```` ```ts ```` blocks in notes and in the settings note, and `system.ts` / `default-settings.ts` embedded as is: Shelv strips types at load (`swc_ts_fast_strip`, positions preserved, erasable syntax only), never type-checks; no generated JS in the repo |
| Block contribution | `export default { commands, global, bindings, ai }`; other exports shared with later blocks |
| Commands and the palette | `command(alias, { icon?, description?, needs?, run })`: the alias is the only positional argument (the rest is optional or form-dependent); the slash palette is the `commands` list, in order, independent of `bindings` (whose `key` is now required) |
| Defaults | defined only in JS: `src/scripting/default-settings.ts` (embedded, types stripped at load) is a base layer evaluated before the note; Rust's `default_keybinding()` and hardcoded palette defaults are removed |
| Actions vs commands | built-ins are **actions** (`action.HideApp`, PascalCase names as in Rust), used only inside a command's `run`; only `command(...)` values are bindable (`bindings`, `global`) or listed (`commands`). Descriptions live in JS (default: the alias); the base layer writes them out, so Rust's `human_description()` goes. `global` takes `Command<"global">` (run is `action.ShowHideApp`) |
| Overriding defaults | the base layer is also the module `"shelv/defaults"` (every default command exported). In the note: same key → later wins; binding a default command (by identity, not alias) moves it off its base-layer keys; `{ key, command: null }` frees a key; `useDefaults: false` drops the whole base layer, `Cmd Comma` included (no exceptions) |
| `run` | `Action`, `Action[]`, or a sync function returning either, or `null`/`undefined` (no-op); no string shorthand (`action.InsertText({ text })`) |
| `needs` | array of **capabilities** only (no constants); `selection` is the only one; typed so `needs` flows into `run`'s parameters |
| `{{selection}}` | removed; use `needs: [selection]` |
| Icons | `Icon.phosphor("calendar-heart")`: a typed name union in `system.ts`, identity at runtime, validated in Rust |
| Run-time errors | notification (today: silent) |
| `SwitchToNote` | `{ index: NoteIndex }`, `NoteIndex = 1 \| 2 \| 3 \| 4` (1-based); base layer binds Cmd 1-4 |
| Secrets | `secret("name")` in v1: `Secret<string, Name>` branded type, resolved in Rust at AI-request time from the **macOS keychain** (generic password, service `Shelv`, account = name) |
| Base layer | not viewable in the app (source only) |
| Upgrade | KDL blocks silently ignored; accepted |
| Reserved for later | `flow` (async), named `needs`, more capabilities, `edit.*`/`action.*` |
| Docs in scope | spec, plan, migration guide, README settings sections, AI system prompt (embeds the generated `system.d.ts`, icon union elided) |

## Phases

### Phase 1: written plan (this change)

- `docs/settings-js/spec.md`, `plan.md`, `migration_to_js.md`.
- Review checkpoint.

### Phase 2: JS surface

Write the JS-facing files first, so they can be reviewed and type-checked before any Rust exists.

1. `src/scripting/system.ts`: the `"shelv"` module, as a single TypeScript file with three parts:
   - `//#region api`: the public v1 types, including the type of every export (`Actions`, `CommandFn`, ...);
   - the runtime: only pure constructors returning frozen, branded data (`action.*`, `command()`,
     `selection`, `Icon.phosphor`, `secret`), typed against the `api` region;
   - `//#region icons`: the generated `PhosphorIconName` union.
2. Generated from `system.ts` by `just build-system`, committed and rebuilt by hand after changing
   the public API (Shelv embeds `system.ts` itself and strips its types at load, Phase 3 step 2; until
   the end of Phase 2 a `system.js` was generated too):
   - `src/scripting/system.d.ts`: the public typings with doc comments (exports marked
     `/** @internal */` are stripped). For editors and `tsc` users of `"shelv"`, and the source for the
     AI prompt. The settings type check runs against it, which proves it is complete.
3. `src/scripting/default-settings.ts`: the base layer, embedded as is (types stripped at load). It
   reproduces every current default from
   `CommandInstruction::default_keybinding()` (`src/command.rs`), the default instruction list, and the
   default slash palette entries (`src/app_state.rs`), including the global show/hide hotkey
   (`Cmd Option S`, today only in `default-settings.md`).
4. A `tsc` check of `system.ts` and `default-settings.ts` (which resolves `"shelv"` to the generated
   `system.d.ts`): `just typecheck-settings`, configured by `src/scripting/tsconfig.json`, which editors
   pick up too.
5. Review checkpoint.

**Status: done.** Notes for Phase 3:
- Actions with parameters (`action.MarkdownCodeBlock`, `action.SwitchToNote`, `action.InsertText`) are
  *function objects* carrying the tag: `__shelv: "builtin"` when usable bare (optional parameters),
  `"builtin-constructor"` when parameters are required. The decoder must read the tag from functions
  too, and must treat only untagged functions as user `run` functions.
- Constructors validate their own arguments (unknown fields, `index` in 1-4, `needs` entries are
  capabilities, `flow` rejected) and throw `TypeError`s at the call site. The decoder re-validates the
  final value with paths (shortcuts, icon names, binding shape: `bindings` / `commands` take editor
  commands, `global` takes global commands, a bare action is rejected with a "wrap it" hint).
- Value shapes: `{ __shelv: "builtin", builtin, params }`, `{ __shelv: "command", scope, id, alias, icon,
  description, needs, run }` (`scope`: `"editor"` or `"global"`; `id`: unique per `Context`, the
  identity used to move a default command), `{ __shelv: "capability", capability }`, `{ __shelv: "secret", name }`;
  `Icon.phosphor` returns the plain name.
- The prompt embeds `system.d.ts` with the `PhosphorIconName` line replaced by
  `type PhosphorIconName = string; // a phosphoricons.com name` and comments optionally stripped;
  measure its size for the prompt budget.
- Smoke-tested in Node (not Boa yet): base layer yields 28 bindings and 1 global.

### Phase 3: Rust, evaluate JS settings (KDL still present but unused)

0. **Upgrade Boa 0.20 → 0.22** as its own change, before any settings work, with no behaviour change.
   Why: `#[boa_module]` / `#[boa_class]` for the host module and its handle classes, `TryFromJs` for
   structs, `Error.prototype.stack` with user line/column for error reporting. Bytecode caching is not a
   reason, since no release has it (`embedding.md` section 7). The probe port (`tmp/boa-probe-022`) is the
   reference.
   - `Cargo.toml`: `boa_engine`, `boa_parser`, `boa_runtime`, `boa_gc` to `0.22`. Consider
     `default-features = false` + `float16`, `xsum` to drop the now-default `temporal`; compare binary size.
   - `src/scripting/js_module_loader.rs`: `ModuleLoader` becomes `async fn load_imported_module(self:
     Rc<Self>, Referrer, ModuleRequest, &RefCell<&mut Context>) -> JsResult<Module>`; keep
     `register_module` / `get_module` as inherent methods; `request.specifier()` replaces the `JsString`.
   - `settings_eval.rs`, `note_eval.rs`, `commands/insert_text.rs`: `run_jobs()` and `Module::evaluate`
     now return `JsResult`; handle the errors instead of ignoring them.
   - `note_eval.rs`, `js_console_logger.rs`: `Console` / `Logger` / `ConsoleState` now come from
     `boa_wintertc` (re-exported by `boa_runtime`); adjust `Console::init_with_logger` if its signature changed.
   - `Context::insert_data` takes any `T: Any`; drop now-unneeded `Trace`/`JsData` derives on host state.
   - Check: `cargo check`, `cargo test`, then `cargo run`: note code blocks still evaluate and log, the
     settings note still applies (KDL path untouched).
1. **Startup timing, tracked from the start** (not deferred to Phase 5), so every later step shows its
   cost:
   - Time each stage on the real path: type stripping (once per process), system parse, system link +
     evaluate, base layer parse,
     base layer link + evaluate, decode + apply, and the total. Log one line per settings evaluation
     (`log::debug!`, plus `eprintln!` behind the existing debug flag), for example
     `settings eval: system parse 0.71ms eval 0.38ms · base parse 0.21ms eval 0.27ms · note 3 blocks 1.9ms · apply 0.1ms`.
   - Keep the numbers in a `SettingsTimings` struct returned with the evaluation, so the debug tools panel
     can show the last one and tests can read it.
   - `#[test] #[ignore] settings_startup` (`cargo test --release -- --ignored settings_startup`): builds a
     `Context`, evaluates the system module and the base layer N times, prints min / median per stage. It fails
     only above a generous ceiling (e.g. 20 ms total), so it catches regressions (a much larger
     `system.ts`, an accidental re-parse) without being flaky.
   - Record the results in the "Results" table below after each step that changes them (the Boa upgrade,
     the separate host API, in-process caching).
2. **TypeScript stripping** (Boa parses JavaScript only; `boa_parser` has no TypeScript support).
   - Probe first in `tmp/ts-strip-probe`: `swc_ts_fast_strip` (strip-only mode, what Node's
     `--experimental-strip-types` uses) against `oxc` (parser + transformer + codegen) on `system.ts`
     (including the 1530-line icon union), `default-settings.ts` and a settings block. Measure strip
     time, added build time and binary size; record them in Results. Expected pick: `swc_ts_fast_strip`,
     because it replaces types with whitespace, so Boa's line and column (errors, `Error.stack`, the
     ✓/✗ annotations) need no source map. oxc re-prints the code and would need its source map.
   - `src/scripting/typescript.rs`: `strip_types(src: &str) -> Result<String, TsDiagnostic { line, col, message }>`.
     Non-erasable syntax (enum, namespace, parameter properties) is an error, matching
     `erasableSyntaxOnly` in `src/scripting/tsconfig.json`.
   - `system.ts` and `default-settings.ts` are embedded with `include_str!` and stripped once per
     process (`OnceLock<String>`); each `Context` parses the stripped text as before. A unit test asserts
     both strip cleanly. No generated JS is committed.
   - Settings note: `SETTINGS_SCRIPT_BLOCK_LANG` (`settings_eval.rs`) accepts `js` and `ts`; a `ts` block
     is stripped before `Module::parse`, and a strip error becomes that block's ✗ annotation.
   - Regular notes: `JSBlockLang::parse` (`note_eval.rs`) accepts `ts`, `ts <n>` and `ts <n> > #<hash>`;
     output blocks keep the language of their source block. Syntax highlighting already maps `ts`.
   - Shared exports between blocks: synthesize the `import { ... }` lines from the evaluated module's
     namespace (runtime exports), not from source text, since type-only exports vanish when stripped.
   - Tests: an error on line N of a `ts` block reports line N; `enum` is a clean error; a `ts` settings
     block applies; a `ts` note block runs and prints its output.
3. Serve the stripped `system.ts` as module `"shelv"` from the settings `InMemoryModuleLoader`
   (`src/scripting/settings_eval.rs`, `js_module_loader.rs`), and the evaluated base layer
   (`default-settings.ts`) as `"shelv/defaults"`: one instance per `Context`, so the note's
   `defaults.bold` is the very value the base layer binds. Set `RuntimeLimits` (loop and recursion).
4. Exclude `default` from the synthesized cross-block imports. Today `import { default } from ...` would
   be a syntax error.
5. Decoder `src/scripting/settings_decode.rs`: read each block's `default` export into
   `SettingsContribution { commands, global, bindings, ai }`.
   - Recognise branded values.
   - Report path-aware errors.
   - Keep `run` functions in a per-evaluation table (`Scripts.user_fns`).
   - Validate icons with `PhosphorIcon::from_string` and shortcuts with `parse_keyboard_shortcut`. Move
     the latter to `src/shortcuts.rs`, together with `format_mac_shortcut_*`.
6. Command model:
   - `action.InsertText` maps to the existing `CommandInstruction::InsertText`.
   - Function `run`s become a new `CommandInstruction::RunUserFn(UserFnId)`. At keypress it resolves
     `needs`, calls the function, decodes the result into built-in instructions, and runs them in order
     through the existing dispatch.
   - Errors become `AppAction::ShowNotification`.
   - `ai.token` as `secret(name)`: keep the name in `LlmSettings`, resolve it in `app_io.rs` when building
     the request from the macOS keychain (`security-framework` or `keyring` crate); a missing secret
     or denied access is a notification.
   - Remove `find_exports` from the command path, the `{{selection}}` substitution, and the dead
     `fn run()` in `src/commands/insert_text.rs`.
7. Apply:
   - Clear the command list, then apply the base layer (skipped when the last `useDefaults` is
     `false`), then the note's blocks in order: `commands` go to the palette (`add_slash_command`, in
     order), `bindings` / `global` to keys.
   - Per note binding: drop the binding on the same key (`add_editor_cmd` does this today); if its
     command `id` is a base-layer command not yet moved, first drop that command's base-layer bindings
     and remember the `id`; `command: null` only drops the binding on its key.
   - Each bound command keeps its alias and description (default: the alias). They feed the palette, the
     empty-note hints (`PROMOTED_COMMANDS`, `src/app_ui.rs`: look up the command whose `run` is that
     action), the AI keybindings doc and run-time notifications.
   - Release global hotkeys before re-registering them (`cleanup_all_global_hotkeys`, which exists but is
     never called today).
   - Reset `llm_settings` before applying.
   - Annotate blocks with `Contributes: … · Shares: …` or the error.
8. Delete `default_keybinding()`, the hardcoded palette list and `CommandInstruction::human_description()`
   (descriptions come from JS; the debug `println!`s in `src/main.rs` use `{:?}`). `CommandList::reset_to_defaults()`
   becomes "apply base layer". Per-instruction phase and focus conditions stay in Rust.

### Phase 4: remove KDL

- **Cargo dependencies:** `kdl`, `knus` (git fork) with its commented alternatives, `tree-sitter`,
  `tree-sitter-highlight`, `tree-sitter-kdl`, `miette`, `strip-ansi-escapes`. None of these is used for
  anything but KDL (the other code blocks use syntect).
- **Code:**
  - every `knus` derive and `ForwardToChild` (`src/command.rs`);
  - the KDL types and tests in `src/settings_parsing.rs`;
  - `src/knus_test.rs`;
  - `src/commands/kdl_lang.rs`, plus `EnterInsideKDL` / `BracketAutoclosingInsideKDL` and their match arms;
  - the KDL highlighting branch in `src/text_structure.rs`;
  - `SETTINGS_BLOCK_LANG`;
  - `eval_kdl_in_settings_note`.
- **AI system prompt:** `serialize_to_kdl` / `create_ai_keybindings_documentation` emit JS. Rewrite the
  KDL parts of `src/prompts/shelv-system-prompt.md`. Embed `system.d.ts` (icon union elided) through a
  `{{shelv_api}}` placeholder.
- **Content:**
  - `src/default-notes/default-settings.md` becomes a short example note, since defaults now live in the
    base layer;
  - update the README settings sections;
  - leave CHANGELOG, site update logs and App Store screenshots as they are (historical).

### Phase 5: performance

Measure first, record the numbers in this file, then optimise.

| # | Measurement | Path |
|---|---|---|
| 0 | type stripping of `system.ts` + `default-settings.ts` | once per process |
| 1 | cold: `Context` build + system module parse, link and evaluate | every startup |
| 2 | cold: base layer evaluate + decode + apply | every startup |
| 3 | warm: re-evaluation after a one-character edit in a settings block | every keystroke in the settings note |
| 4 | the full real-world note (`migration_to_js.md` worked example) | startup for an existing user |

- **How:** the timing log line and `settings_startup` test added in Phase 3, step 1.
- **Caching across launches is not available**, as of Boa 0.22 (latest, checked 2026-10): no bytecode or
  snapshot serialization, `Module::parse` only accepts source, the `deser` feature only makes the AST
  serializable, and startup snapshots (boa-dev/boa#1372) are unimplemented. Probe timings (release,
  warm): about 1.7 ms for `Context` + system module + base layer, on 0.20 and 0.22 alike
  (`embedding.md` section 7).
- **In-process caching** (the main expected win: today every keystroke in the settings note builds a fresh
  `Context`):
  - keep one settings `Context` alive;
  - evaluate the system module and the base layer once (their stripped text is already cached per process);
  - re-parse only changed blocks and the blocks after them, keyed by a hash of the full block source
    including the synthesized imports;
  - fall back to a fresh `Context` after an error, so stale state cannot leak.

### Results

Baseline from the probes (not Shelv itself): `tmp/boa-probe` (0.20), `tmp/boa-probe-022` (0.22) and
`tmp/boa-probe-main` (git `main` at `9871775`, 2026-10-17, 53 commits after v0.22). Release build,
M-series Mac, the then-generated `system.js` (6.7 KB, comments stripped), binaries run directly and interleaved, 10 runs each; median shown,
min within 0.03 ms. "Cold" is the first run after a build (0.22).

| Stage | 0.20 | 0.22 | main | 0.22 cold |
|---|---|---|---|---|
| `Context` build | 0.21 ms | 0.25 ms | 0.25 ms | 2.4 ms |
| `system.js` parse | 0.62 ms | 0.73 ms | 0.75 ms | 3.6 ms |
| `system.js` link + evaluate | 0.31 ms | 0.38 ms | 0.40 ms | 2.7 ms |
| `default-settings.js` parse | 0.16 ms | 0.21 ms | 0.20 ms | 0.65 ms |
| `default-settings.js` link + evaluate | 0.29 ms | 0.28 ms | 0.30 ms | 1.1 ms |
| **total** | **1.59 ms** | **1.85 ms** | **1.88 ms** | **~10 ms** |

- Parsing is about two thirds of the system module's cost, so its size matters more than its runtime
  work. These numbers are for the generated JS (6.7 KB). Shelv now parses the stripped `system.ts`
  (about 45 KB: comments kept, and the icon union becomes whitespace), so re-measure in Phase 3 step 2.
  If parsing grows noticeably, strip comments too, or move the icon union to its own file that is
  never embedded (it is types only).
- 0.22 and `main` are the same within noise, and both about 16% (0.26 ms) slower than 0.20 on this
  workload, mostly in parsing and `Context` build. The upgrade is for the APIs, not for speed; at under
  2 ms the difference does not matter. `main` builds the probe unchanged, so no API changes since 0.22
  affect us.
- Cold numbers are what a user sees at app launch; they are dominated by first-touch costs and vary
  between runs.

In Shelv (fill in during Phase 3, step 1 onwards):

| # | Measurement | 0.20 | 0.22 | separate host API | after cache |
|---|---|---|---|---|---|
| 0 | type stripping (once per process) | | | | |
| 1 | `Context` + system module (parse / evaluate) | | | | |
| 2 | base layer (parse / evaluate / decode + apply) | | | | |
| 3 | keystroke re-evaluation | | | | |
| 4 | real-world note | | | | |

## Verification

- **`cargo test`:**
  - the base layer produces the same command list as today's Rust defaults (snapshot taken before
    deleting them);
  - decoder tests: every KDL construct's JS equivalent, and every error kind in `spec.md` section 9;
  - `system.ts` and `default-settings.ts` strip without errors and load;
  - the new example settings note evaluates on top of the base layer;
  - the worked example from `migration_to_js.md` evaluates with all of its bindings;
  - the `icons` region of `system.ts` is in sync with egui-phosphor (rewritten with `UPDATE_ICONS=1`);
  - the AI prompt stays under a size ceiling (about 20 KB; today 9.5 KB, `system.d.ts` without icons about 5 KB);
  - the keybinding-docs test is updated for JS output.
- **`tsc`:** over `system.ts` and `default-settings.ts` (`just typecheck-settings`).
- **`cargo check`:** passes with no `knus`, `kdl` or `tree-sitter` left in `Cargo.lock`.
- **Manual, `cargo run`:**
  - edit the settings note and check the annotation text;
  - Cmd / wraps the selection, and Cmd K inserts a link;
  - the palette shows aliases with icons;
  - changing the global hotkey releases the old one;
  - deleting `ai` falls back to the default model;
  - a leftover ```` ```kdl ```` block has no effect;
  - a throwing `run` shows a notification;
  - `{ key: "Cmd Shift B", command: defaults.bold }` leaves `Cmd B` unbound; `{ key: "Cmd P", command: null }`
    unbinds pin; `useDefaults: false` leaves only the note's bindings.

## Risks

| Risk | Mitigation |
|---|---|
| Upgrading users' KDL settings stop working **silently** (kdl blocks are ignored) | accepted; defaults keep working through the base layer |
| `run` arrays: each command must see the state left by the previous one, but dispatch may queue actions instead of applying them | check the dispatch in `app_state.rs`; apply each instruction's actions before running the next, and group them into one undo step |
| Today's `SwitchToNote` defaults depend on the shelf count (Cmd 0 for shelves beyond 4) | decided: `NoteIndex` 1-4 and Cmd 1-4 only; extra shelves have no default key |
| Per-keystroke evaluation cost grows with the system module and the base layer | measurements 1-3, then in-process caching |
| 16-bit `SourceHash` used as the module specifier can collide | widen it when touching the loader (Phase 3) |
| Prompt size grows with the API | size test; icons are never embedded |
| Sandboxed App Store build may not read keychain items created by the `security` CLI | test on the App Store build; placeholder: a Shelv UI that writes the item itself |

## Open questions

None blocking. Placeholder: a Shelv UI for storing secrets in the keychain (_to be designed_; likely
required for the App Store build, see Risks).

To decide before Phase 3: `embedding.md` (draft) proposes a host API (`"shelv:host"`, Rust) that is
separate from the public `"shelv"` API. The host takes normalized data, answers queries (icons,
shortcuts) and returns native handles. `system.ts` maps the public sugar onto it and produces every
user-facing error. This replaces tagged data plus a Rust decoder, and a JS shim of the same `Host`
interface runs settings in unit tests and outside Shelv.

Decided: `NoteIndex` 1-4 with Cmd 1-4 defaults; `null`/`undefined` from `run` is a no-op; the base layer
is not viewable in the app; silent KDL breakage is accepted; secrets in the macOS keychain; a command
bound to several keys shows the last binding's key in the palette; Phase 3 (JS evaluation) and Phase 4 (KDL removal) land as two changes.
