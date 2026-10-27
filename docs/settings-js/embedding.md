# Embedding `system.ts` and bridging Rust into it (draft)

Status: draft for discussion. Everything marked **verified** was run in throwaway probe crates against
Boa 0.20 and Boa 0.22; both passed the same checks. Upgrading Boa: section 7.

Related: `plan.md` (Phase 3), `spec.md`.

## 1. Embedding `system.ts` (Phase 3)

Shelv embeds the TypeScript sources as is. Boa runs JavaScript only, so their types are stripped first
(`strip_types`, a thin wrapper over `swc_ts_fast_strip` in strip-only mode, plan.md Phase 3). Stripping
replaces types with whitespace, so line and column numbers stay those of the `.ts` file. The sources
never change at runtime, so each is stripped once per process:

```rust
static SYSTEM_JS: OnceLock<String> = OnceLock::new();
let system_js = SYSTEM_JS.get_or_init(|| strip_types(include_str!("system.ts")).expect("system.ts is erasable TS"));

let module = Module::parse(Source::from_bytes(system_js).with_path(Path::new("shelv:system")), None, cx)?;
loader.register_module(js_string!("shelv"), module.clone());
module.load_link_evaluate(cx); // then cx.run_jobs()

// the base layer is also a module the note imports: `import * as defaults from "shelv/defaults"`
static DEFAULTS_JS: OnceLock<String> = OnceLock::new();
let defaults_js = DEFAULTS_JS.get_or_init(|| strip_types(include_str!("default-settings.ts")).expect("erasable TS"));
let defaults = Module::parse(Source::from_bytes(defaults_js).with_path(Path::new("shelv:defaults")), None, cx)?;
loader.register_module(js_string!("shelv/defaults"), defaults.clone());
defaults.load_link_evaluate(cx); // its `default` export is the base layer's Settings
```

- **One instance per `Context`.** Every block that imports `"shelv"` gets the same module instance,
  so `action.MarkdownBold` is one frozen object shared by all blocks. **Verified**: the base layer
  (`default-settings`) evaluates against it and yields 28 bindings; Rust reads the tag from the
  function object `action.MarkdownCodeBlock` (`callable=true __shelv="builtin"`).
- **`"shelv/defaults"` is evaluated once per `Context` too**, so `defaults.bold` in the note is the same
  value (same `id`) that the base layer binds; that is what lets Rust move a default (spec section 2).
- **The path is the identity.** Parsing with the path `shelv:system` lets the module loader recognise
  imports coming *from* the system module (section 3.2). User blocks have no path.
- **Freshness.** Nothing generated is embedded, so an edit to `system.ts` takes effect on the next
  build. Only `system.d.ts` (editors, AI prompt) is generated and committed: rerun `just build-system`
  after changing the public API. A Rust test asserts both `.ts` files strip without errors.
- **Limits.** Set `RuntimeLimits` (loop iterations, recursion) on the settings `Context`, since `run`
  functions execute on keypress.
- **Cost (verified, release build, M-series Mac, warm runs of the binary, measured with the earlier
  generated JS, comments stripped; the stripped `.ts` keeps comments and blank type space, so re-measure):**
  `Context` build about 0.2 ms, `system.js` parse + link + evaluate about 0.8-1.1 ms, `default-settings.js` about 0.45 ms,
  so about 1.7 ms per full evaluation. Boa 0.20 and 0.22 are within about 10% of each other. The first
  run after a build is several times slower (cold process), and timings taken through `cargo run` are
  inflated, so measure the binary. This is small enough that per-keystroke re-evaluation is not urgent,
  but the planned in-process caching (one `Context`, the system module evaluated once) still removes most of it.

## 2. What Boa offers for Rust → JS (0.20; changes in 0.21/0.22 in section 7)

| Mechanism | API | Notes |
|---|---|---|
| Native function | `NativeFunction::from_fn_ptr(fn)` | No captures: safest. State comes from `Context` data. **Verified.** |
| Native function with captures | `from_copy_closure_with_captures(f, captures)` | Captures must be `Trace`. Non-`Copy` closures need `unsafe from_closure` (GC cannot trace closure captures). |
| Async native function | `NativeFunction::from_async_fn` / `JsPromise::new_pending` | For `flow` later; see doc 07. |
| Host state | `Context::insert_data(T)` / `get_data::<T>()` | `T: NativeObject` (`Trace + Finalize + JsData`). Holds e.g. an `Rc<RefCell<Vec<Effect>>>` with `#[unsafe_ignore_trace]` (no GC pointers inside). **Verified.** |
| Synthetic module | `Module::synthetic(&names, SyntheticModuleInitializer, ..)` + `set_export` | A Rust-defined ES module; JS imports it like any module. **Verified.** |
| Native class | `impl Class` + `register_global_class::<T>()`, `ClassBuilder::method/accessor` | Instances carry Rust data (`downcast_ref::<T>()`). |
| Rust-created handles | `T::from_data(data, cx)` | Builds an instance without calling the JS constructor. **Verified.** |
| Hide the global class | `global_object().delete_property_or_throw("Note")` after registering | The class stays in the realm (`from_data`, `instanceof` keep working), but user code cannot reach it as a global. **Verified** (`typeof globalThis.Note === "undefined"`). |
| Non-constructible from JS | `data_constructor` returns `Err` | `new host.Note()` throws. **Verified.** |
| Plain objects | `ObjectInitializer::new(cx).property(..).function(..)` | For export bags. |
| Call JS from Rust | `JsObject::as_callable()?.call(this, args, cx)` | How `run(selection)` is called on keypress. **Verified.** |
| Convenience layer | 0.20: crate `boa_interop` (`IntoJsFunctionCopied`, `js_class!`, `ContextData<T>`, `#[derive(TryFromJs)]`). 0.21+: folded into `boa_engine::interop`, plus `#[boa_class]` and `#[boa_module]` | Not used today. With 0.22, `#[boa_module]` is a natural fit for `"shelv:host"` (section 7). |

No Boa release has a bytecode or snapshot cache (section 7), so every launch parses the stripped `system.ts`
again (about 1 ms).

## 3. Two APIs: public (`"shelv"`) and host (`"shelv:host"`)

The API that settings code sees and the API that Rust exposes are **deliberately different**:

```
 settings block ──import──▶ "shelv"       system.ts: public, ergonomic, validates, reports errors
                                │    ▲
                         calls  │    │ hooks (validateSettings, ...)
                                ▼    │
                            "shelv:host"  Rust: internal, minimal, normalized data, no user-facing errors
```

| | Public API (`system.ts` `api` region, emitted as `system.d.ts`) | Host API (`Host` interface, Rust) |
|---|---|---|
| Audience | people and AI writing settings | `system.ts` only |
| Shape | sugar: `run` as an action, an array, or a function; bare `action.MarkdownCodeBlock`; `Icon.phosphor` | one normalized form per concept: `run` is always `{ kind: "builtins", list }` or `{ kind: "fn", fn, needs }` |
| Validation | all user-facing checks, at the call site | assumes valid input; asserts it |
| Errors | user terms: `action.SwitchToNote: "index" must be one of 1, 2, 3, 4, got 0`, `bindings[2].key: unknown key "Cmd Shitf B" (did you mean "Shift"?)` | `internal: ...`, meaning a bug in `system.ts`, never shown as the user's mistake |
| Changes | follows the spec, versioned for users | follows Rust refactors (e.g. `CommandInstruction`), free to change |

Why separate them:
- **Better error reporting.** Errors are produced in JS, where the user's code is running: thrown at
  the user's call (so Boa's error points at that line), phrased in public names (`action.*`, `bindings[i]`),
  never in Rust names (`CommandInstruction::MarkdownCodeblock`). Rust contributes *knowledge* (is this
  a valid icon? does this shortcut parse? which name is closest?) through queries, and `system.ts`
  turns the answers into messages.
- **Each side evolves on its own.** New sugar in the public API (another `run` form, a renamed
  action kept as an alias) needs no Rust change; a Rust refactor needs no public API change.
- **A small host is cheap to shim** (section 5) and to keep in sync.

### 3.1 The host API

Two kinds of functions, both on normalized data.

**Queries** are pure and never throw for bad user input; they return an answer for `system.ts` to word:

```ts
interface Host {
  readonly version: 1;                    // system.ts checks it, so a mismatched build fails loudly
  checkIcon(name: string): { ok: true } | { ok: false; suggestion?: string };
  parseShortcut(key: string): { ok: true; normalized: string } | { ok: false; reason: string; suggestion?: string };

  /** Constructors: normalized, already validated input; return an opaque handle. */
  builtin(desc: { builtin: string; params: Record<string, unknown> }): CmdHandle;
  command(desc: {
    alias: string; icon?: string; description: string; // system.ts defaults description to the alias
    scope: "editor" | "global";                            // "global": run is a global action
    id: number;                                            // identity: moving a default command
    run: { kind: "builtins"; list: readonly CmdHandle[] } | { kind: "fn"; fn: Function; needs: readonly string[] };
  }): CommandHandle;

  /** Hooks that Rust calls back into system.ts (kept private: not exports of "shelv"). */
  setHooks(hooks: { validateSettings(value: unknown): NormalizedSettings; normalizeRunResult(value: unknown): readonly CmdHandle[] }): void;
}
```

- **Constructors** decode once in Rust (into a `CommandInstruction`, or a `UserFn` stored in
  `Scripts.user_fns`) and return a native handle (`Class::from_data`, verified above). They only build
  values and register nothing, so a block's contribution is still decided by its `export default`
  alone (merge order, "a failing block contributes nothing", caching all behave as specified).
- **Hooks** cover the parts that are not a call made by the user: the `export default` value and the
  value returned by a `run` function. Rust calls `validateSettings(default)` per block, and
  `normalizeRunResult(result)` on keypress, so these errors are worded by the same JS code
  (`bindings[2].command: expected a command, got a string`). Rust then only `downcast_ref`s handles.
  The hooks are handed over with `host.setHooks(...)` when `system.ts` evaluates, so they stay out of
  the public `"shelv"` exports.
- Icon names and shortcuts are now checked **at the call** (`Icon.phosphor("calender")` throws on
  the user's line, with "did you mean calendar?"), not later by a decoder.

Public sugar maps onto the normalized form in `system.ts`, for example:

```ts
export const command: CommandFn = (alias: unknown, def?: unknown): Command => {
  // ...user-facing validation...
  const normalized = isUserFunction(run)
    ? { kind: "fn", fn: run, needs: needs.map(capabilityName) }
    : { kind: "builtins", list: Array.isArray(run) ? run.map(toHandle) : [toHandle(run)] };
  return host.command({ alias, icon, description, run: normalized }) as unknown as Command;
};
```

### 3.2 Only `system.ts` may import the host

The module loader receives the importing module (`Referrer::Module(m)`). Allow `"shelv:host"` only
when `m.path()` starts with `shelv:system`:

```rust
let from_system = matches!(&referrer, Referrer::Module(m) if m.path().is_some_and(|p| p.starts_with("shelv:system")));
if name == "shelv:host" && !from_system { /* TypeError: module 'shelv:host' is internal */ }
```

**Verified**: a user block's `import { log } from "shelv:host"` is rejected, while the system module
imports it fine.

### 3.3 Typing the host inside `system.ts` (still one file)

`Host` lives in `system.ts`, *outside* the `api` region, exported for the shim but marked
`/** @internal */`, so `--stripInternal` keeps it out of `system.d.ts` and the AI prompt:

```ts
// @ts-ignore: provided by Shelv at runtime (Rust synthetic module), typed by `Host`
import * as rawHost from "shelv:host";
const host: Host = rawHost;
if (host.version !== 1) throw new Error(`internal: shelv:host version ${host.version}, system.ts expects 1`);
```

Checked with `tsc` (scratch test):
- `declare module "shelv:host" { ... }` inside `system.ts` **does not work**: in a module file it is an
  augmentation, so it fails with `TS2664: Invalid module name in augmentation`.
- `@ts-ignore` on the import plus an assignment to `Host` **works**: calls through `host` are fully
  typed and wrong uses are errors. The emitted JS is a plain `import * as rawHost from "shelv:host"`.
- Fallback: a hidden global (`declare const __shelvHost: Host`), captured by `system.ts` and then
  deleted from the global object. It also works, but it mutates globals and makes isolation depend on
  evaluation order. Prefer the module.

Drift between Rust and `Host`:
- **Names and version**: the `version` check, plus a Rust test that evaluates `system.ts` against the
  real host and exercises every host function.
- **Argument shapes**: not checked across the boundary, which is one more reason to keep the host
  small and normalized.

## 4. Compared with v1 today (tagged data + Rust decoder)

| | Tagged data + decoder (`system.ts` now) | Separate host API (proposal) |
|---|---|---|
| Who validates | `system.ts` partly, then a Rust decoder re-validates with paths | `system.ts` only, with Rust answering queries |
| Who words errors | both (two styles, two places) | `system.ts` only; Rust errors are `internal:` bugs |
| When icons/shortcuts fail | after evaluation, by path | at the user's call, by line, with suggestions |
| Rust ↔ JS contract | the `__shelv` tag shapes (implicit) | the `Host` interface (explicit, typed, versioned) |
| Rust decoder | walks the whole tree | `downcast_ref`s handles from `validateSettings` |
| Runs outside Shelv | yes, the data is plain JS | yes, with the shim (section 5) |

## 5. Shims: running settings in unit tests and outside Shelv

Because the host is small, a JavaScript implementation of `Host` is small too. It lets `system.ts`,
`default-settings.ts` and any settings block run in Node (and in Boa without the Rust host):

```ts
// src/scripting/host-shim.ts: test-only, type-checked against the same interface
import type { Host } from "./system.ts";

export const version: Host["version"] = 1;
export const checkIcon: Host["checkIcon"] = () => ({ ok: true }); // or check against an icon list
export const parseShortcut: Host["parseShortcut"] = (key) => ({ ok: true, normalized: key });
export const builtin: Host["builtin"] = (desc) => Object.freeze({ shim: "builtin", ...desc });
export const command: Host["command"] = (desc) => Object.freeze({ shim: "command", ...desc });
// ...
```

- **Typing**: the shim is written against `Host`, so `tsc` reports a missing or wrong shim function
  as soon as `Host` changes. Rust, the shim and `system.ts` all meet at one interface.
- **Fidelity**: queries in the shim can be loose (accept any shortcut) or strict (a JS port of the
  rules). The icon list exists only as a type, so a strict `checkIcon` would need the names at runtime
  (e.g. generated next to the union). Loose is fine for testing settings logic; Rust tests cover the
  real queries.
- **Node**: resolve `"shelv:host"` to the shim with a module resolve hook (`module.register`) or a
  package `imports` entry; `"shelv"` resolves to `system.ts`, run with `--experimental-strip-types`
  (Node strips types only outside `node_modules`, so point the entry at the source file). Then a plain test can import a settings
  block and call a `run` function with a fake selection:
  ```js
  import { backticks } from "./my-settings.js";
  assert.deepEqual(inspect(backticks).run("x"), [{ builtin: "InsertText", params: { text: "`x`{||}" } }]);
  ```
  `inspect(handle)` would be a documented export that both host implementations support (the real
  handle is opaque); see open question 2.
- **Boa**: in `cargo test`, the same shim (types stripped like `system.ts`) can be registered as
  `"shelv:host"` to test `system.ts` in isolation, while the real host is tested separately against it.
- **File count**: one test-only file. Keeping it inside `system.ts` would need `system.ts` to become a
  factory (`createShelv(host)`), since static imports cannot be conditional. Separate file recommended.

## 6. Later: async (`flow`) over the same bridge

Doc 07's design fits here unchanged: requests such as `ask.for(...)` are inert thenables built by
`system.ts`; for a request it cannot resolve in JS, `system.ts` calls a host builtin returning a
`JsPromise::new_pending` whose resolvers Rust keeps and settles from the app loop, then `run_jobs`.
The shim resolves the same requests with canned answers for tests.

## 7. Boa versions: what 0.21 and 0.22 change for us

Shelv uses 0.20 (2024-12). Since then: 0.21 (2025-10), 0.21.1 (2026-03, patch), **0.22 (2026-08-28,
latest)**. Checked against the release notes and the 0.22 sources.

**Bytecode caching: still none.**
- 0.22 has no API to serialize compiled code: `Module::parse` and `Script::parse` take only source, and
  `CodeBlock` has no serde. The `deser` feature still only covers the AST, interner and bigints.
- The "bytecode" and "cache" items in the 0.22 notes are *runtime* work: bytecompiler/VM
  optimizations and an inline cache that went from monomorphic to polymorphic. Neither persists
  across launches.
- Startup snapshots: issue [boa-dev/boa#1372](https://github.com/boa-dev/boa/issues/1372) (2021, open).
  It is about snapshotting the whole `Context` heap at startup, as V8 does, not caching module bytecode.
  The discussion says it needs a different GC first. Its only implementation,
  [PR #3041 "Implement Startup Snapshot"](https://github.com/boa-dev/boa/pull/3041) (2023-06, +2061
  lines in 41 files, mostly `boa_engine/src/snapshot/{serializer,deserializer}.rs`), was a WIP with its
  TODO list unchecked (file format, full ser/de, checksum, versioning). It was closed unmerged on
  2025-12-18 as "very outdated with the current codebase".
- [PR #4921 "Add bytecode snapshot tests using `insta`"](https://github.com/boa-dev/boa/pull/4921)
  (merged 2026-03-08) is `insta` snapshot *testing* of bytecompiler output, for reviewing compiler
  changes. It is not a cache.
- Nothing open or recent touches serialization (GitHub search on snapshot / serialize / bytecode /
  cache, 2026-10-21).
- So the plan stays the same: in-process caching only. At about 1 ms for the system module, caching across
  launches would not be worth much anyway.

**What is useful for us:**
- **`Error.prototype.stack` (0.22).** Verified on 0.22: an error thrown inside
  `system.ts` for a user call carries the user's location, using the source path we set:
  ```
  TypeError: action.SwitchToNote: "index" must be one of 1, 2, 3, 4, got 0
      at fail (shelv:system:4:11)
      at <main> (shelv:system:67:17)
      at create (shelv:system:25:14)
      at <main> (settings.md#block-3:2:34)
  ```
  This works the same whether JS catches the error (`e.stack`) or Rust gets it uncaught, and is
  exactly what section 3 needs. Show the first frame that is not `shelv:system`, mapped to the note.
  Subtract the lines of the imports we synthesize ahead of each block, or append those imports after
  the block instead. `shelv:system` frames carry `system.ts` line numbers (stripping keeps positions),
  which helps when debugging Shelv, but hide them from users.
- **`#[boa_module]` / `#[boa_class]` (0.21+), verified.** A Rust
  `mod` becomes the ES module `"shelv:host"`:
  ```rust
  #[boa_module]
  #[boa(rename_all = "camelCase")]
  pub mod host {
      const VERSION: u32 = 1;                                  // export const version
      fn check_icon(name: JsString) -> bool { .. }              // export function checkIcon
      fn parse_shortcut(key: JsString) -> JsResult<JsString> { .. } // a JsResult Err throws in JS
      type CmdHandle = super::CmdHandle;                         // export class CmdHandle (#[boa_class])
  }
  ```
  - Arguments and return values convert through `TryFromJs` / `TryIntoJs` (also derivable for structs,
    so `{ builtin, params }` decodes into a Rust struct).
  - Classes exported this way are **module exports, not globals** (`typeof globalThis.CmdHandle ===
    "undefined"`), so the probe's "register then delete the global" step is unnecessary.
  - `#[boa_class]` turns `&self` methods into methods and `#[boa(getter)]` into accessors.
  - Caveats: the generated `boa_module(realm, cx) -> Module` is `pub(super)`, so wrap it in a `pub fn`.
    Every `fn` in the mod becomes an export, so helpers live outside it. `&str` is not an argument
    type; use `JsString` or `String`. Async fns are not supported (for `flow`, use
    `NativeFunction::from_async_fn` by hand).
- **0.21 also added** span nodes and error backtraces, a register-based VM, NaN-boxed `JsValue`
  (less memory) and a revamped `JobQueue`. The new `JobQueue` matters for `flow` later (doc 07).

**Migration cost for Shelv** (0.20 → 0.22, the probe port is the evidence):
- `ModuleLoader` is now `async fn load_imported_module(self: Rc<Self>, Referrer, ModuleRequest,
  &RefCell<&mut Context>) -> JsResult<Module>`. `register_module` / `get_module` are gone from the
  trait (keep them on our loader), and `ModuleRequest::specifier()` replaces the `JsString`.
  `Referrer::path()` is new, which simplifies the section 3.2 check. Affects
  `src/scripting/js_module_loader.rs`.
- `Context::run_jobs()` now returns `JsResult<()>`, and `Module::evaluate` returns
  `JsResult<JsPromise>`. Affects `settings_eval.rs`.
- `insert_data` / `get_data` take any `T: Any`, so host state no longer needs `Trace`/`JsData`.
- `boa_interop` stops at 0.20; its contents moved into `boa_engine::interop`.
- `boa_runtime` 0.22 now re-exports `Console`, `Logger` and `ConsoleState` from `boa_wintertc`, so
  `note_eval.rs` (`Console::init_with_logger`) and `js_console_logger.rs` should port with small
  changes (not compiled yet).
- `temporal` is now a default feature (more code). Turn it off with `default-features = false` and
  `float16`/`xsum` re-enabled, if binary size matters.
- In the probe, all other APIs (`Class`, `from_data`, `NativeFunction`, synthetic modules) compiled
  unchanged.

Recommendation: upgrade to 0.22 as a separate first step of Phase 3 (before any settings work). It is a
small, contained port. It gives `Error.prototype.stack` for error reporting and `#[boa_module]` for
the host, and it keeps us off a version that is 2 years behind.

## 8. Open questions

1. Adopt the separate host API (sections 3-4) for Phase 3, or keep tagged data + decoder for v1 and
   move later? It removes the Rust decoder's tag walking and moves all user-facing errors into
   `system.ts`, but makes the shim a prerequisite for running settings outside Shelv.
2. How do tests look inside a handle: a documented `inspect(handle)` export, or shim-only fields?
3. `boa_interop` as a dependency (typed native fns, `TryFromJs` derive), or hand-written `get`s while
   the host has fewer than about 5 functions?
4. Upgrade Boa to 0.22 first (section 7)?
5. Should note code blocks (`note_eval.rs`) later import `"shelv"` too? Then the host module needs a
   per-context capability set (settings vs notes).
