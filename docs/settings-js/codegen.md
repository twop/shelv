# Settings API: actions, capabilities, icons

## How it works now

Settings blocks import `"shelv"`, which is `src/scripting/system.ts`. Shelv embeds that file, strips the
types when it loads, and runs it in Boa. The values it builds (actions, commands, capabilities) are plain
frozen objects with a `__shelv` tag; Rust reads them back in `src/scripting/settings_decode.rs`.

Actions exist twice, by hand:

- in `system.ts`: the type and doc comment in `interface Actions`, the value in `export const action`;
- in `src/command.rs`: a `CommandInstruction` variant with the same name and the same parameter names.

`CommandInstruction` derives `serde::Deserialize`, internally tagged by `builtin`, so variants without
parameters stay plain unit variants (`MarkdownBold`). `system.ts` builds
`{ builtin: "MarkdownCodeBlock", params: { lang: "js" } }`; `instruction_from_js` (`src/scripting/actions.rs`)
flattens it to `{ builtin: "MarkdownCodeBlock", lang: "js" }`, and serde turns that into
`MarkdownCodeBlock { lang: Some("js") }`. The test `system_ts_actions_match` in the same file
fails if the two lists differ, or if an action does not decode.

To add an action:

1. `system.ts`: add it to `interface Actions` (with a doc comment) and to `action`. Without parameters it
   is a plain value (`plain("X")`); with parameters use `withParams("X", optional, check)`, where
   `optional` means it can also be used bare, like `action.MarkdownCodeBlock`.
2. `command.rs`: add the `CommandInstruction` variant. The compiler then points at
   `execute_instruction` (`app_state.rs`, what it does) and `default_phase_and_conditions` (when it is
   allowed, e.g. only with the editor focused).
3. If the action must be called, give the test sample parameters in `sample_params` (`actions.rs`).
4. `just build-system` (regenerates `system.d.ts`), then `just test`.

To change an action's parameters, change both sides and the test's sample. Rust-only rules, such as
`SwitchToNote`'s 1-based index, go in a serde `deserialize_with` on the field. Global actions (like
`ShowHideApp`) are special-cased in `Decoder::action` and `install_settings`; a new one needs both.

Capabilities (`needs: [selection]`) work the same way: an export in `system.ts`, a variant of `Need` in
`settings_decode.rs`, the test `system_ts_capabilities_match`, and an arm in `resolve_user_command` that
produces the value.

Icon names are the one generated part: the `//#region icons` union in `system.ts` comes from
`egui_phosphor::light::ICONS`. `system_ts_icons_match` fails when it is stale; `just update-icons`
rewrites it.

Defaults live in `src/scripting/default-settings.ts`, written with the same API. It is also importable as
`"shelv/defaults"`, so a settings block can rebind a default command (`defaults.bold`) or unbind a key
(`command: null`).

## Alternatives

All of these would make Rust the single source for actions, so `system.ts` would not repeat them.

### ts-rs

`#[derive(TS)]` on `CommandInstruction` emits a TypeScript union of the variants and their parameter
types. `system.ts` would derive `action.*` from it with mapped types (bare, callable, or both, depending on
whether the parameters are optional).

- Pro: no hand-written action types; a parameter change in Rust shows up in `tsc`.
- Con: per-variant doc comments are dropped, so editors and the AI prompt lose the action descriptions.
- Con: types only. `system.ts` still needs a runtime list of the actions to build `action`, which would
  stay hand-written or need another generator.
- Con: it writes one file per type with imports, while `system.ts` must stay one file, so its output would
  be pasted into a region by a test.

### schemars

`#[derive(JsonSchema)]` produces a JSON Schema of `CommandInstruction` that follows its serde attributes
exactly (tag, defaults, unknown fields), with doc comments as descriptions. A test would turn the schema
into a `system.ts` region: the types with doc comments, plus a runtime table of names and parameters, from
which `action` is built in one loop.

- Pro: docs and runtime table both come from Rust; the schema matches what serde accepts.
- Con: we would own a schema-to-TypeScript generator (roughly 100 lines).
- Con: one more derive and dependency on a core enum.

### facet

`#[derive(Facet)]` gives typed reflection at runtime: variants, fields, their types and doc comments. A test
would walk that and write the same region as with schemars.

- Pro: the most direct access to names, types and docs.
- Con: it does not read serde attributes, so the wire format is described twice; enums need `#[repr(u8)]`.
- Con: pre-1.0 and changing quickly.

### A macro

An `actions!` macro would expand one list into both the Rust enum and a table that a test renders into
`system.ts`.

- Pro: no dependencies; full control over the output.
- Con: the IDE cannot see inside the macro (no completion, no go-to-definition), and every reader has to
  learn its syntax.

With 27 actions that change rarely, the current setup (two lists and a test) was cheaper than any of these.
If many more Rust types need to reach TypeScript, schemars is the one to try first.
