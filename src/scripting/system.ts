/*! system.d.ts is GENERATED from system.ts by `just build-system`: edit system.ts only. */
// The "shelv" module: the API that settings blocks import (`import { action, command } from "shelv"`).
// Spec: docs/settings-js/spec.md.
//
// This file is the single source for the module: its types, its runtime and the icon names.
// Shelv embeds it as is and strips the types when it loads (Boa runs JS only), so only erasable
// TypeScript is allowed here (tsconfig `erasableSyntaxOnly`: no enum, namespace, parameter properties).
// `just build-system` generates ./system.d.ts (committed; the `/*!` line above is copied into it): the
// public typings, also embedded (icon union elided) in the AI system prompt.
// Anything exported here is public and lands in system.d.ts, unless its doc comment has the JSDoc
// "internal" tag (the build passes --stripInternal; do not write that tag in plain comments here).
//
// Layout:
//   - region `api`: the public types, including the types of every export.
//   - the runtime: pure constructors that build frozen data and have no effect on the app. Shelv reads
//     that data back in Rust. Values carry an internal `__shelv` tag so the decoder can recognise them;
//     the tag is not part of the public API.
//   - region `icons`: the generated `PhosphorIconName` union (rewritten by the icons sync test).

//#region api

// Brands (internal; values are created only by the constructors in this module)
declare const actionBrand: unique symbol;
declare const commandBrand: unique symbol;
declare const capabilityBrand: unique symbol;
declare const secretBrand: unique symbol;

// --- Built-in actions ------------------------------------------------------------------------

/**
 * A built-in action, ready to run: what a command's `run` does. Actions are not bindable on their own;
 * wrap them in a command: `command("bold", { run: action.MarkdownBold })`.
 */
export interface Action<Name extends string = string> {
	readonly [actionBrand]: Name;
	readonly global?: undefined;
}

/** An action that works system-wide: the whole `run` of a command bound in `global`. */
export interface GlobalAction<Name extends string = string> {
	readonly [actionBrand]: Name;
	readonly global: true;
}

/** Shelf number, 1-based like the shelf labels. */
export type NoteIndex = 1 | 2 | 3 | 4;

export interface Actions {
	// text and markdown
	/** Replaces the selection (or inserts at the cursor). `{||}` sets the cursor, `{|}..{|}` selects. */
	readonly InsertText: (params: { text: string }) => Action<"InsertText">;
	readonly MarkdownBold: Action<"MarkdownBold">;
	readonly MarkdownItalic: Action<"MarkdownItalic">;
	readonly MarkdownStrikethrough: Action<"MarkdownStrikethrough">;
	/** Bare: a code block without language. Called: `action.MarkdownCodeBlock({ lang: "js" })`. */
	readonly MarkdownCodeBlock: Action<"MarkdownCodeBlock"> &
		((params: { lang?: string }) => Action<"MarkdownCodeBlock">);
	readonly MarkdownH1: Action<"MarkdownH1">;
	readonly MarkdownH2: Action<"MarkdownH2">;
	readonly MarkdownH3: Action<"MarkdownH3">;

	// lists
	readonly IndentListItem: Action<"IndentListItem">;
	readonly UnindentListItem: Action<"UnindentListItem">;
	readonly SplitListItem: Action<"SplitListItem">;
	readonly ExpandTaskMarker: Action<"ExpandTaskMarker">;

	// structural editing
	readonly ExpandSelection: Action<"ExpandSelection">;
	readonly ShrinkSelection: Action<"ShrinkSelection">;
	readonly MoveLinesUp: Action<"MoveLinesUp">;
	readonly MoveLinesDown: Action<"MoveLinesDown">;

	// navigation and palette
	readonly StartWordJump: Action<"StartWordJump">;
	readonly ShowSlashPalette: Action<"ShowSlashPalette">;
	readonly ShowPrompt: Action<"ShowPrompt">;

	// notes and window
	readonly SwitchToNote: (params: {
		index: NoteIndex;
	}) => Action<"SwitchToNote">;
	readonly SwitchToSettings: Action<"SwitchToSettings">;
	readonly OpenFileDialog: Action<"OpenFileDialog">;
	readonly CloseCurrentNote: Action<"CloseCurrentNote">;
	readonly PinWindow: Action<"PinWindow">;
	readonly HideApp: Action<"HideApp">;
	readonly ToggleDebugTools: Action<"ToggleDebugTools">;

	// global only
	readonly ShowHideApp: GlobalAction<"ShowHideApp">;
}

// --- Capabilities (inputs for `needs`) -------------------------------------------------------

/** An inert capability: nothing happens until Shelv resolves it for a running command. */
export interface Capability<T> {
	readonly [capabilityBrand]: T;
}

// --- command(...) ----------------------------------------------------------------------------

/**
 * What bindings bind and the palette lists. `"global"`: its `run` is a `GlobalAction`, so it can only be
 * bound in `global`; every other command is an `"editor"` command.
 */
export interface Command<Scope extends "editor" | "global" = "editor"> {
	readonly [commandBrand]: Scope;
}

/** Shown in the slash palette (when the command is listed in `Settings.commands`) and in hints. */
export interface CommandMeta {
	/** Palette icon, `Icon.phosphor("name")`; default `"user-circle-gear"`. */
	icon?: PhosphorIconName;
	/** Palette and hint text; default: the alias. */
	description?: string;
}

/** `null` / `undefined` / `[]`: do nothing. */
export type RunResult = Action | readonly Action[] | null | undefined;

export type Resolve<N extends readonly Capability<unknown>[]> = {
	[K in keyof N]: N[K] extends Capability<infer T> ? T : never;
};

export interface CommandFn {
	/**
	 * `alias` names the command: its slash-palette name, and how errors refer to it.
	 * A function `run` with inputs: entry i of `needs` is resolved into argument i.
	 */
	<const N extends readonly Capability<unknown>[]>(
		alias: string,
		def: CommandMeta & { needs: N; run: (...inputs: Resolve<N>) => RunResult },
	): Command;
	/** `run` as an action, an array of actions, or a function without inputs. */
	(
		alias: string,
		def: CommandMeta & {
			needs?: undefined;
			run: Action | readonly Action[] | (() => RunResult);
		},
	): Command;
	/** A global action (`action.ShowHideApp`) as the whole `run`: a command for `global` bindings. */
	(
		alias: string,
		def: CommandMeta & { needs?: undefined; run: GlobalAction },
	): Command<"global">;
}

// --- Secrets and icons -----------------------------------------------------------------------

/** A reference to a secret in the macOS keychain (service "Shelv", account = name). */
export interface Secret<T extends string, Name extends string = string> {
	readonly [secretBrand]: { value: T; name: Name };
}

export interface IconNs {
	/** A Phosphor icon name (https://phosphoricons.com), light set shipped with Shelv. */
	phosphor<const N extends PhosphorIconName>(name: N): N;
}

// --- Settings (`export default` of a block) --------------------------------------------------

/**
 * `"Cmd Shift B"`: space-separated modifiers (`Cmd`, `Ctrl`, `Option`/`Alt`, `Shift`), then exactly
 * one key (egui key names: `A`, `1`, `Enter`, `Comma`, `/`, ...). Validated by Shelv when settings load.
 */
export type Shortcut = string;

export interface Binding {
	key: Shortcut;
	/** `null` frees the key: removes whatever an earlier block or the base layer bound to it. */
	command: Command | null;
}

export interface GlobalBinding {
	/** System-wide hotkey. */
	key: Shortcut;
	/** `null` frees the hotkey. */
	command: Command<"global"> | null;
}

export interface AiSettings {
	/** genai model name; `"shelv-claude"` is Shelv's proxy model (the default without `ai`). */
	model: "shelv-claude" | (string & {});
	/** Extra instructions, added after Shelv's prompt. */
	systemPrompt?: string;
	/** API token: a plain string, or `secret("name")`. */
	token?: string | Secret<string>;
	/** Prepend Shelv's own system prompt. Default `true`. */
	useShelvSystemPrompt?: boolean;
}

/**
 * A block's contribution. Blocks merge in order: lists append; a binding on a bound key replaces it;
 * binding a default command (from "shelv/defaults") moves it off its default keys; `ai` and
 * `useDefaults` last win.
 */
export interface Settings {
	/** `false`: the base layer contributes nothing (no default commands, bindings or global hotkey). */
	useDefaults?: boolean;
	/** Slash-palette entries, in palette order. A command can be listed here, bound to a key, or both. */
	commands?: readonly Command[];
	global?: readonly GlobalBinding[];
	bindings?: readonly Binding[];
	ai?: AiSettings;
}

//#endregion api

// ---------------------------------------------------------------------------------------------------
// Runtime
// ---------------------------------------------------------------------------------------------------

const TAG = "__shelv";

type Tag =
	| "builtin"
	| "builtin-constructor"
	| "command"
	| "capability"
	| "secret";
type Params = Record<string, unknown>;

const fail = (message: string): never => {
	throw new TypeError(message);
};

const isPlainObject = (value: unknown): value is Params =>
	typeof value === "object" &&
	value !== null &&
	Object.getPrototypeOf(value) === Object.prototype;

const describe = (value: unknown): string =>
	value === null || value === undefined
		? String(value)
		: Array.isArray(value)
			? "an array"
			: typeof value === "object"
				? "an object"
				: `a ${typeof value}`;

const tagOf = (value: unknown): unknown =>
	(typeof value === "object" || typeof value === "function") && value !== null
		? (value as { [TAG]?: unknown })[TAG]
		: undefined;

/** Rejects keys outside `allowed`, so typos fail instead of being ignored. */
const checkKeys = (
	where: string,
	object: Params,
	allowed: readonly string[],
): void => {
	for (const key of Object.keys(object)) {
		if (!allowed.includes(key)) {
			fail(
				`${where}: unknown field "${key}" (expected ${allowed.map((k) => `"${k}"`).join(", ")})`,
			);
		}
	}
};

// --- Built-in actions ------------------------------------------------------------------------

const builtinValue = (builtin: string, params: Params, extra: Params = {}) =>
	Object.freeze({
		[TAG]: "builtin" as Tag,
		builtin,
		params: Object.freeze(params),
		...extra,
	});

/** An action without parameters: a plain value. */
const plain = <N extends string>(builtin: N): Action<N> =>
	builtinValue(builtin, {}) as unknown as Action<N>;

/** An action that works system-wide. */
const globalOnly = <N extends string>(builtin: N): GlobalAction<N> =>
	builtinValue(builtin, {}, { global: true }) as unknown as GlobalAction<N>;

/**
 * An action with parameters: a function that creates an instance.
 * With `optional`, the function itself is also usable as the instance without parameters.
 */
const withParams = <N extends string>(
	builtin: N,
	optional: boolean,
	check: (params: Params) => void,
) => {
	const create = (params: unknown) => {
		if (!isPlainObject(params))
			return fail(
				`action.${builtin}: expected an object, got ${describe(params)}`,
			);
		check(params);
		return builtinValue(builtin, { ...params }) as unknown as Action<N>;
	};
	Object.defineProperty(create, TAG, {
		value: (optional ? "builtin" : "builtin-constructor") satisfies Tag,
	});
	Object.defineProperty(create, "builtin", { value: builtin });
	Object.defineProperty(create, "params", { value: Object.freeze({}) });
	return Object.freeze(create);
};

const NOTE_INDEXES: readonly unknown[] = [1, 2, 3, 4];

export const action: Actions = Object.freeze({
	// text and markdown
	InsertText: withParams("InsertText", false, (params) => {
		checkKeys("action.InsertText", params, ["text"]);
		if (typeof params.text !== "string") {
			fail(
				`action.InsertText: "text" must be a string, got ${describe(params.text)}`,
			);
		}
	}),
	MarkdownBold: plain("MarkdownBold"),
	MarkdownItalic: plain("MarkdownItalic"),
	MarkdownStrikethrough: plain("MarkdownStrikethrough"),
	MarkdownCodeBlock: withParams("MarkdownCodeBlock", true, (params) => {
		checkKeys("action.MarkdownCodeBlock", params, ["lang"]);
		if (params.lang !== undefined && typeof params.lang !== "string") {
			fail(
				`action.MarkdownCodeBlock: "lang" must be a string, got ${describe(params.lang)}`,
			);
		}
	}) as unknown as Actions["MarkdownCodeBlock"],
	MarkdownH1: plain("MarkdownH1"),
	MarkdownH2: plain("MarkdownH2"),
	MarkdownH3: plain("MarkdownH3"),

	// lists
	IndentListItem: plain("IndentListItem"),
	UnindentListItem: plain("UnindentListItem"),
	SplitListItem: plain("SplitListItem"),
	ExpandTaskMarker: plain("ExpandTaskMarker"),

	// structural editing
	ExpandSelection: plain("ExpandSelection"),
	ShrinkSelection: plain("ShrinkSelection"),
	MoveLinesUp: plain("MoveLinesUp"),
	MoveLinesDown: plain("MoveLinesDown"),

	// navigation and palette
	StartWordJump: plain("StartWordJump"),
	ShowSlashPalette: plain("ShowSlashPalette"),
	ShowPrompt: plain("ShowPrompt"),

	// notes and window
	SwitchToNote: withParams("SwitchToNote", false, (params) => {
		checkKeys("action.SwitchToNote", params, ["index"]);
		if (!NOTE_INDEXES.includes(params.index)) {
			fail(
				`action.SwitchToNote: "index" must be one of ${NOTE_INDEXES.join(", ")}, got ${String(params.index)}`,
			);
		}
	}),
	SwitchToSettings: plain("SwitchToSettings"),
	OpenFileDialog: plain("OpenFileDialog"),
	CloseCurrentNote: plain("CloseCurrentNote"),
	PinWindow: plain("PinWindow"),
	HideApp: plain("HideApp"),
	ToggleDebugTools: plain("ToggleDebugTools"),

	// global only
	ShowHideApp: globalOnly("ShowHideApp"),
});

const isBuiltin = (value: unknown): boolean => tagOf(value) === "builtin";
const isGlobalAction = (value: unknown): boolean =>
	isBuiltin(value) && (value as { global?: true }).global === true;

// --- Capabilities ----------------------------------------------------------------------------

const capability = <T>(name: string): Capability<T> =>
	Object.freeze({
		[TAG]: "capability" as Tag,
		capability: name,
	}) as unknown as Capability<T>;

/** The selected text in the current note, `""` when nothing is selected. */
export const selection: Capability<string> = capability("selection");

const isCapability = (value: unknown): boolean =>
	isPlainObject(value) && value[TAG] === "capability";

/** A user function, as opposed to an action with parameters (which is a tagged function object). */
const isUserFunction = (value: unknown): value is Function =>
	typeof value === "function" && !(TAG in value);

// --- command(...) ----------------------------------------------------------------------------

const COMMAND_KEYS = ["icon", "description", "needs", "run", "flow"];

/** Identity of a command for Shelv (rebinding a default moves it); aliases are not unique. */
let nextCommandId = 0;

const builtinName = (value: unknown): string =>
	(value as { builtin: string }).builtin;

const checkRunValue = (where: string, run: unknown): void => {
	if (Array.isArray(run)) {
		run.forEach((entry, i) => {
			if (isGlobalAction(entry)) {
				fail(
					`${where}[${i}]: action.${builtinName(entry)} can only be the whole run of a command (for global bindings)`,
				);
			}
			if (!isBuiltin(entry))
				fail(`${where}[${i}]: expected an action, got ${describe(entry)}`);
		});
	} else if (!isBuiltin(run)) {
		if (tagOf(run) === "builtin-constructor") {
			const name = builtinName(run);
			fail(
				`${where}: action.${name} has required parameters, call it: action.${name}({ ... })`,
			);
		}
		if (tagOf(run) === "command")
			fail(
				`${where}: expected an action, got a command (commands cannot run commands in v1)`,
			);
		fail(
			`${where}: expected an action, an array of actions, or a function, got ${describe(run)}`,
		);
	}
};

// The overloads in CommandFn pick the scope from `run`; the implementation returns either.
export const command = ((
	alias: unknown,
	def?: unknown,
): Command<"editor" | "global"> => {
	if (isPlainObject(alias) && "alias" in alias) {
		return fail(
			`command(): the alias is now the first argument: command("${String(alias.alias)}", { ... })`,
		);
	}
	if (typeof alias !== "string")
		return fail(
			`command(): expected an alias string first, got ${describe(alias)}`,
		);
	if (alias === "") fail("command(): the alias must not be empty");
	const where = `command("${alias}")`;
	if (!isPlainObject(def))
		return fail(`${where}: expected an object second, got ${describe(def)}`);
	checkKeys(where, def, COMMAND_KEYS);

	const { icon, description, needs, run } = def;

	if ("flow" in def) fail(`${where}: flow is not supported yet`);
	if (!("run" in def)) fail(`${where}: run is required`);

	for (const [field, value] of [
		["icon", icon],
		["description", description],
	] as const) {
		if (value !== undefined && typeof value !== "string") {
			fail(`${where}: "${field}" must be a string, got ${describe(value)}`);
		}
	}

	if (isUserFunction(run)) {
		if (needs !== undefined) {
			if (!Array.isArray(needs))
				fail(`${where}: "needs" must be an array, got ${describe(needs)}`);
			(needs as unknown[]).forEach((entry, i) => {
				if (!isCapability(entry)) {
					fail(
						`${where}: needs[${i}]: expected a capability (such as selection), got ${describe(entry)}`,
					);
				}
			});
		}
	} else {
		if (needs !== undefined) fail(`${where}: needs requires a function run`);
		checkRunValue(`${where}: run`, run);
	}

	return Object.freeze({
		[TAG]: "command" as Tag,
		// "global": bindable only in `global` (checked when the settings load)
		scope: isGlobalAction(run) ? "global" : "editor",
		id: nextCommandId++,
		alias,
		icon,
		description,
		needs: Object.freeze([...((needs as unknown[] | undefined) ?? [])]),
		run: Array.isArray(run) ? Object.freeze([...run]) : run,
	}) as unknown as Command<"editor" | "global">;
}) as CommandFn;

// --- Secrets and icons -----------------------------------------------------------------------

/** A reference to a secret in the macOS keychain (service "Shelv", account `name`). */
export function secret<const Name extends string>(
	name: Name,
): Secret<string, Name> {
	if (typeof name !== "string")
		fail(`secret(): expected a name, got ${describe(name)}`);
	if (name === "") fail("secret(): the name must not be empty");
	return Object.freeze({ [TAG]: "secret" as Tag, name }) as unknown as Secret<
		string,
		Name
	>;
}

export const Icon: IconNs = Object.freeze({
	/** Identity at runtime: the name is type-checked here and validated by Shelv when settings load. */
	phosphor<const N extends PhosphorIconName>(name: N): N {
		if (typeof name !== "string")
			fail(`Icon.phosphor(): expected an icon name, got ${describe(name)}`);
		return name;
	},
});

//#region icons
// GENERATED from egui-phosphor 0.14.0 (src/variants/codepoints.rs). Do not edit by hand.
// Kept in sync by the icons test (docs/settings-js/plan.md, Phase 3).
// 1530 Phosphor icon names, as accepted by `Icon.phosphor(...)` and `PhosphorIcon::from_string`.
export type PhosphorIconName =
	| "acorn"
	| "address-book"
	| "address-book-tabs"
	| "air-traffic-control"
	| "airplane"
	| "airplane-in-flight"
	| "airplane-landing"
	| "airplane-takeoff"
	| "airplane-taxiing"
	| "airplane-tilt"
	| "airplay"
	| "alarm"
	| "alien"
	| "align-bottom"
	| "align-bottom-simple"
	| "align-center-horizontal"
	| "align-center-horizontal-simple"
	| "align-center-vertical"
	| "align-center-vertical-simple"
	| "align-left"
	| "align-left-simple"
	| "align-right"
	| "align-right-simple"
	| "align-top"
	| "align-top-simple"
	| "amazon-logo"
	| "ambulance"
	| "anchor"
	| "anchor-simple"
	| "android-logo"
	| "angle"
	| "angular-logo"
	| "aperture"
	| "app-store-logo"
	| "app-window"
	| "apple-logo"
	| "apple-podcasts-logo"
	| "approximate-equals"
	| "archive"
	| "armchair"
	| "arrow-arc-left"
	| "arrow-arc-right"
	| "arrow-bend-double-up-left"
	| "arrow-bend-double-up-right"
	| "arrow-bend-down-left"
	| "arrow-bend-down-right"
	| "arrow-bend-left-down"
	| "arrow-bend-left-up"
	| "arrow-bend-right-down"
	| "arrow-bend-right-up"
	| "arrow-bend-up-left"
	| "arrow-bend-up-right"
	| "arrow-circle-down"
	| "arrow-circle-down-left"
	| "arrow-circle-down-right"
	| "arrow-circle-left"
	| "arrow-circle-right"
	| "arrow-circle-up"
	| "arrow-circle-up-left"
	| "arrow-circle-up-right"
	| "arrow-clockwise"
	| "arrow-counter-clockwise"
	| "arrow-down"
	| "arrow-down-left"
	| "arrow-down-right"
	| "arrow-elbow-down-left"
	| "arrow-elbow-down-right"
	| "arrow-elbow-left"
	| "arrow-elbow-left-down"
	| "arrow-elbow-left-up"
	| "arrow-elbow-right"
	| "arrow-elbow-right-down"
	| "arrow-elbow-right-up"
	| "arrow-elbow-up-left"
	| "arrow-elbow-up-right"
	| "arrow-fat-down"
	| "arrow-fat-left"
	| "arrow-fat-line-down"
	| "arrow-fat-line-left"
	| "arrow-fat-line-right"
	| "arrow-fat-line-up"
	| "arrow-fat-lines-down"
	| "arrow-fat-lines-left"
	| "arrow-fat-lines-right"
	| "arrow-fat-lines-up"
	| "arrow-fat-right"
	| "arrow-fat-up"
	| "arrow-left"
	| "arrow-line-down"
	| "arrow-line-down-left"
	| "arrow-line-down-right"
	| "arrow-line-left"
	| "arrow-line-right"
	| "arrow-line-up"
	| "arrow-line-up-left"
	| "arrow-line-up-right"
	| "arrow-right"
	| "arrow-square-down"
	| "arrow-square-down-left"
	| "arrow-square-down-right"
	| "arrow-square-in"
	| "arrow-square-left"
	| "arrow-square-out"
	| "arrow-square-right"
	| "arrow-square-up"
	| "arrow-square-up-left"
	| "arrow-square-up-right"
	| "arrow-u-down-left"
	| "arrow-u-down-right"
	| "arrow-u-left-down"
	| "arrow-u-left-up"
	| "arrow-u-right-down"
	| "arrow-u-right-up"
	| "arrow-u-up-left"
	| "arrow-u-up-right"
	| "arrow-up"
	| "arrow-up-left"
	| "arrow-up-right"
	| "arrows-clockwise"
	| "arrows-counter-clockwise"
	| "arrows-down-up"
	| "arrows-horizontal"
	| "arrows-in"
	| "arrows-in-cardinal"
	| "arrows-in-line-horizontal"
	| "arrows-in-line-vertical"
	| "arrows-in-simple"
	| "arrows-left-right"
	| "arrows-merge"
	| "arrows-out"
	| "arrows-out-cardinal"
	| "arrows-out-line-horizontal"
	| "arrows-out-line-vertical"
	| "arrows-out-simple"
	| "arrows-split"
	| "arrows-vertical"
	| "article"
	| "article-medium"
	| "article-ny-times"
	| "asclepius"
	| "caduceus"
	| "asterisk"
	| "asterisk-simple"
	| "at"
	| "atom"
	| "avocado"
	| "axe"
	| "baby"
	| "baby-carriage"
	| "backpack"
	| "backspace"
	| "bag"
	| "bag-simple"
	| "balloon"
	| "bandaids"
	| "bank"
	| "barbell"
	| "barcode"
	| "barn"
	| "barricade"
	| "baseball"
	| "baseball-cap"
	| "baseball-helmet"
	| "basket"
	| "basketball"
	| "bathtub"
	| "battery-charging"
	| "battery-charging-vertical"
	| "battery-empty"
	| "battery-full"
	| "battery-high"
	| "battery-low"
	| "battery-medium"
	| "battery-plus"
	| "battery-plus-vertical"
	| "battery-vertical-empty"
	| "battery-vertical-full"
	| "battery-vertical-high"
	| "battery-vertical-low"
	| "battery-vertical-medium"
	| "battery-warning"
	| "battery-warning-vertical"
	| "beach-ball"
	| "beanie"
	| "bed"
	| "beer-bottle"
	| "beer-stein"
	| "behance-logo"
	| "bell"
	| "bell-ringing"
	| "bell-simple"
	| "bell-simple-ringing"
	| "bell-simple-slash"
	| "bell-simple-z"
	| "bell-slash"
	| "bell-z"
	| "belt"
	| "bezier-curve"
	| "bicycle"
	| "binary"
	| "binoculars"
	| "biohazard"
	| "bird"
	| "blueprint"
	| "bluetooth"
	| "bluetooth-connected"
	| "bluetooth-slash"
	| "bluetooth-x"
	| "boat"
	| "bomb"
	| "bone"
	| "book"
	| "book-bookmark"
	| "book-open"
	| "book-open-text"
	| "book-open-user"
	| "bookmark"
	| "bookmark-simple"
	| "bookmarks"
	| "bookmarks-simple"
	| "books"
	| "boot"
	| "boules"
	| "bounding-box"
	| "bowl-food"
	| "bowl-steam"
	| "bowling-ball"
	| "box-arrow-down"
	| "archive-box"
	| "box-arrow-up"
	| "boxing-glove"
	| "brackets-angle"
	| "brackets-curly"
	| "brackets-round"
	| "brackets-square"
	| "brain"
	| "brandy"
	| "bread"
	| "bridge"
	| "briefcase"
	| "briefcase-metal"
	| "broadcast"
	| "broom"
	| "browser"
	| "browsers"
	| "bug"
	| "bug-beetle"
	| "bug-droid"
	| "building"
	| "building-apartment"
	| "building-office"
	| "buildings"
	| "bulldozer"
	| "bus"
	| "butterfly"
	| "cable-car"
	| "cactus"
	| "cake"
	| "calculator"
	| "calendar"
	| "calendar-blank"
	| "calendar-check"
	| "calendar-dot"
	| "calendar-dots"
	| "calendar-heart"
	| "calendar-minus"
	| "calendar-plus"
	| "calendar-slash"
	| "calendar-star"
	| "calendar-x"
	| "call-bell"
	| "camera"
	| "camera-plus"
	| "camera-rotate"
	| "camera-slash"
	| "campfire"
	| "car"
	| "car-battery"
	| "car-profile"
	| "car-simple"
	| "cardholder"
	| "cards"
	| "cards-three"
	| "caret-circle-double-down"
	| "caret-circle-double-left"
	| "caret-circle-double-right"
	| "caret-circle-double-up"
	| "caret-circle-down"
	| "caret-circle-left"
	| "caret-circle-right"
	| "caret-circle-up"
	| "caret-circle-up-down"
	| "caret-double-down"
	| "caret-double-left"
	| "caret-double-right"
	| "caret-double-up"
	| "caret-down"
	| "caret-left"
	| "caret-line-down"
	| "caret-line-left"
	| "caret-line-right"
	| "caret-line-up"
	| "caret-right"
	| "caret-up"
	| "caret-up-down"
	| "carrot"
	| "cash-register"
	| "cassette-tape"
	| "castle-turret"
	| "cat"
	| "cell-signal-full"
	| "cell-signal-high"
	| "cell-signal-low"
	| "cell-signal-medium"
	| "cell-signal-none"
	| "cell-signal-slash"
	| "cell-signal-x"
	| "cell-tower"
	| "certificate"
	| "chair"
	| "chalkboard"
	| "chalkboard-simple"
	| "chalkboard-teacher"
	| "champagne"
	| "charging-station"
	| "chart-bar"
	| "chart-bar-horizontal"
	| "chart-donut"
	| "chart-line"
	| "chart-line-down"
	| "chart-line-up"
	| "chart-pie"
	| "chart-pie-slice"
	| "chart-polar"
	| "chart-scatter"
	| "chat"
	| "chat-centered"
	| "chat-centered-dots"
	| "chat-centered-slash"
	| "chat-centered-text"
	| "chat-circle"
	| "chat-circle-dots"
	| "chat-circle-slash"
	| "chat-circle-text"
	| "chat-dots"
	| "chat-slash"
	| "chat-teardrop"
	| "chat-teardrop-dots"
	| "chat-teardrop-slash"
	| "chat-teardrop-text"
	| "chat-text"
	| "chats"
	| "chats-circle"
	| "chats-teardrop"
	| "check"
	| "check-circle"
	| "check-fat"
	| "check-square"
	| "check-square-offset"
	| "checkerboard"
	| "checks"
	| "cheers"
	| "cheese"
	| "chef-hat"
	| "cherries"
	| "church"
	| "cigarette"
	| "cigarette-slash"
	| "circle"
	| "circle-dashed"
	| "circle-half"
	| "circle-half-tilt"
	| "circle-notch"
	| "circles-four"
	| "circles-three"
	| "circles-three-plus"
	| "circuitry"
	| "city"
	| "clipboard"
	| "clipboard-text"
	| "clock"
	| "clock-afternoon"
	| "clock-clockwise"
	| "clock-countdown"
	| "clock-counter-clockwise"
	| "clock-user"
	| "closed-captioning"
	| "cloud"
	| "cloud-arrow-down"
	| "cloud-arrow-up"
	| "cloud-check"
	| "cloud-fog"
	| "cloud-lightning"
	| "cloud-moon"
	| "cloud-rain"
	| "cloud-slash"
	| "cloud-snow"
	| "cloud-sun"
	| "cloud-warning"
	| "cloud-x"
	| "clover"
	| "club"
	| "coat-hanger"
	| "coda-logo"
	| "code"
	| "code-block"
	| "code-simple"
	| "codepen-logo"
	| "codesandbox-logo"
	| "coffee"
	| "coffee-bean"
	| "coin"
	| "coin-vertical"
	| "coins"
	| "columns"
	| "columns-plus-left"
	| "columns-plus-right"
	| "command"
	| "compass"
	| "compass-rose"
	| "compass-tool"
	| "computer-tower"
	| "confetti"
	| "contactless-payment"
	| "control"
	| "cookie"
	| "cooking-pot"
	| "copy"
	| "copy-simple"
	| "copyleft"
	| "copyright"
	| "corners-in"
	| "corners-out"
	| "couch"
	| "court-basketball"
	| "cow"
	| "cowboy-hat"
	| "cpu"
	| "crane"
	| "crane-tower"
	| "credit-card"
	| "cricket"
	| "crop"
	| "cross"
	| "crosshair"
	| "crosshair-simple"
	| "crown"
	| "crown-cross"
	| "crown-simple"
	| "cube"
	| "cube-focus"
	| "cube-transparent"
	| "currency-btc"
	| "currency-circle-dollar"
	| "currency-cny"
	| "currency-dollar"
	| "currency-dollar-simple"
	| "currency-eth"
	| "currency-eur"
	| "currency-gbp"
	| "currency-inr"
	| "currency-jpy"
	| "currency-krw"
	| "currency-kzt"
	| "currency-ngn"
	| "currency-rub"
	| "cursor"
	| "cursor-click"
	| "cursor-text"
	| "cylinder"
	| "database"
	| "desk"
	| "desktop"
	| "desktop-tower"
	| "detective"
	| "dev-to-logo"
	| "device-mobile"
	| "device-mobile-camera"
	| "device-mobile-slash"
	| "device-mobile-speaker"
	| "device-rotate"
	| "device-tablet"
	| "device-tablet-camera"
	| "device-tablet-speaker"
	| "devices"
	| "diamond"
	| "diamonds-four"
	| "dice-five"
	| "dice-four"
	| "dice-one"
	| "dice-six"
	| "dice-three"
	| "dice-two"
	| "disc"
	| "disco-ball"
	| "discord-logo"
	| "divide"
	| "dna"
	| "dog"
	| "door"
	| "door-open"
	| "dot"
	| "dot-outline"
	| "dots-nine"
	| "dots-six"
	| "dots-six-vertical"
	| "dots-three"
	| "dots-three-circle"
	| "dots-three-circle-vertical"
	| "dots-three-outline"
	| "dots-three-outline-vertical"
	| "dots-three-vertical"
	| "download"
	| "download-simple"
	| "dress"
	| "dresser"
	| "dribbble-logo"
	| "drone"
	| "drop"
	| "drop-half"
	| "drop-half-bottom"
	| "drop-simple"
	| "drop-slash"
	| "dropbox-logo"
	| "ear"
	| "ear-slash"
	| "egg"
	| "egg-crack"
	| "eject"
	| "eject-simple"
	| "elevator"
	| "empty"
	| "engine"
	| "envelope"
	| "envelope-open"
	| "envelope-simple"
	| "envelope-simple-open"
	| "equalizer"
	| "equals"
	| "eraser"
	| "escalator-down"
	| "escalator-up"
	| "exam"
	| "exclamation-mark"
	| "exclude"
	| "exclude-square"
	| "export"
	| "eye"
	| "eye-closed"
	| "eye-slash"
	| "eyedropper"
	| "eyedropper-sample"
	| "eyeglasses"
	| "eyes"
	| "face-mask"
	| "facebook-logo"
	| "factory"
	| "faders"
	| "faders-horizontal"
	| "fallout-shelter"
	| "fan"
	| "farm"
	| "fast-forward"
	| "fast-forward-circle"
	| "feather"
	| "fediverse-logo"
	| "figma-logo"
	| "file"
	| "file-archive"
	| "file-arrow-down"
	| "file-arrow-up"
	| "file-audio"
	| "file-c"
	| "file-c-sharp"
	| "file-cloud"
	| "file-code"
	| "file-cpp"
	| "file-css"
	| "file-csv"
	| "file-dashed"
	| "file-dotted"
	| "file-doc"
	| "file-html"
	| "file-image"
	| "file-ini"
	| "file-jpg"
	| "file-js"
	| "file-jsx"
	| "file-lock"
	| "file-magnifying-glass"
	| "file-search"
	| "file-md"
	| "file-minus"
	| "file-pdf"
	| "file-plus"
	| "file-png"
	| "file-ppt"
	| "file-py"
	| "file-rs"
	| "file-sql"
	| "file-svg"
	| "file-text"
	| "file-ts"
	| "file-tsx"
	| "file-txt"
	| "file-video"
	| "file-vue"
	| "file-x"
	| "file-xls"
	| "file-zip"
	| "files"
	| "film-reel"
	| "film-script"
	| "film-slate"
	| "film-strip"
	| "fingerprint"
	| "fingerprint-simple"
	| "finn-the-human"
	| "fire"
	| "fire-extinguisher"
	| "fire-simple"
	| "fire-truck"
	| "first-aid"
	| "first-aid-kit"
	| "fish"
	| "fish-simple"
	| "flag"
	| "flag-banner"
	| "flag-banner-fold"
	| "flag-checkered"
	| "flag-pennant"
	| "flame"
	| "flashlight"
	| "flask"
	| "flip-horizontal"
	| "flip-vertical"
	| "floppy-disk"
	| "floppy-disk-back"
	| "flow-arrow"
	| "flower"
	| "flower-lotus"
	| "flower-tulip"
	| "flying-saucer"
	| "folder"
	| "folder-notch"
	| "folder-dashed"
	| "folder-dotted"
	| "folder-lock"
	| "folder-minus"
	| "folder-notch-minus"
	| "folder-open"
	| "folder-notch-open"
	| "folder-plus"
	| "folder-notch-plus"
	| "folder-simple"
	| "folder-simple-dashed"
	| "folder-simple-dotted"
	| "folder-simple-lock"
	| "folder-simple-minus"
	| "folder-simple-plus"
	| "folder-simple-star"
	| "folder-simple-user"
	| "folder-star"
	| "folder-user"
	| "folders"
	| "football"
	| "football-helmet"
	| "footprints"
	| "fork-knife"
	| "four-k"
	| "frame-corners"
	| "framer-logo"
	| "function"
	| "funnel"
	| "funnel-simple"
	| "funnel-simple-x"
	| "funnel-x"
	| "game-controller"
	| "garage"
	| "gas-can"
	| "gas-pump"
	| "gauge"
	| "gavel"
	| "gear"
	| "gear-fine"
	| "gear-six"
	| "gender-female"
	| "gender-intersex"
	| "gender-male"
	| "gender-neuter"
	| "gender-nonbinary"
	| "gender-transgender"
	| "ghost"
	| "gif"
	| "gift"
	| "git-branch"
	| "git-commit"
	| "git-diff"
	| "git-fork"
	| "git-merge"
	| "git-pull-request"
	| "github-logo"
	| "gitlab-logo"
	| "gitlab-logo-simple"
	| "globe"
	| "globe-hemisphere-east"
	| "globe-hemisphere-west"
	| "globe-simple"
	| "globe-simple-x"
	| "globe-stand"
	| "globe-x"
	| "goggles"
	| "golf"
	| "goodreads-logo"
	| "google-cardboard-logo"
	| "google-chrome-logo"
	| "google-drive-logo"
	| "google-logo"
	| "google-photos-logo"
	| "google-play-logo"
	| "google-podcasts-logo"
	| "gps"
	| "gps-fix"
	| "gps-slash"
	| "gradient"
	| "graduation-cap"
	| "grains"
	| "grains-slash"
	| "graph"
	| "graphics-card"
	| "greater-than"
	| "greater-than-or-equal"
	| "grid-four"
	| "grid-nine"
	| "guitar"
	| "hair-dryer"
	| "hamburger"
	| "hammer"
	| "hand"
	| "hand-arrow-down"
	| "hand-arrow-up"
	| "hand-coins"
	| "hand-deposit"
	| "hand-eye"
	| "hand-fist"
	| "hand-grabbing"
	| "hand-heart"
	| "hand-palm"
	| "hand-peace"
	| "hand-pointing"
	| "hand-soap"
	| "hand-swipe-left"
	| "hand-swipe-right"
	| "hand-tap"
	| "hand-waving"
	| "hand-withdraw"
	| "handbag"
	| "handbag-simple"
	| "hands-clapping"
	| "hands-praying"
	| "handshake"
	| "hard-drive"
	| "hard-drives"
	| "hard-hat"
	| "hash"
	| "hash-straight"
	| "head-circuit"
	| "headlights"
	| "headphones"
	| "headset"
	| "heart"
	| "heart-break"
	| "heart-half"
	| "heart-straight"
	| "heart-straight-break"
	| "heartbeat"
	| "hexagon"
	| "high-definition"
	| "high-heel"
	| "highlighter"
	| "highlighter-circle"
	| "hockey"
	| "hoodie"
	| "horse"
	| "hospital"
	| "hourglass"
	| "hourglass-high"
	| "hourglass-low"
	| "hourglass-medium"
	| "hourglass-simple"
	| "hourglass-simple-high"
	| "hourglass-simple-low"
	| "hourglass-simple-medium"
	| "house"
	| "house-line"
	| "house-simple"
	| "hurricane"
	| "ice-cream"
	| "identification-badge"
	| "identification-card"
	| "image"
	| "image-broken"
	| "image-square"
	| "images"
	| "images-square"
	| "infinity"
	| "lemniscate"
	| "info"
	| "instagram-logo"
	| "intersect"
	| "intersect-square"
	| "intersect-three"
	| "intersection"
	| "invoice"
	| "island"
	| "jar"
	| "jar-label"
	| "jeep"
	| "joystick"
	| "kanban"
	| "key"
	| "key-return"
	| "keyboard"
	| "keyhole"
	| "knife"
	| "ladder"
	| "ladder-simple"
	| "lamp"
	| "lamp-pendant"
	| "laptop"
	| "lasso"
	| "lastfm-logo"
	| "layout"
	| "leaf"
	| "lectern"
	| "lego"
	| "lego-smiley"
	| "less-than"
	| "less-than-or-equal"
	| "letter-circle-h"
	| "letter-circle-p"
	| "letter-circle-v"
	| "lifebuoy"
	| "lightbulb"
	| "lightbulb-filament"
	| "lighthouse"
	| "lightning"
	| "lightning-a"
	| "lightning-slash"
	| "line-segment"
	| "line-segments"
	| "line-vertical"
	| "link"
	| "link-break"
	| "link-simple"
	| "link-simple-break"
	| "link-simple-horizontal"
	| "link-simple-horizontal-break"
	| "linkedin-logo"
	| "linktree-logo"
	| "linux-logo"
	| "list"
	| "list-bullets"
	| "list-checks"
	| "list-dashes"
	| "list-heart"
	| "list-magnifying-glass"
	| "list-numbers"
	| "list-plus"
	| "list-star"
	| "lock"
	| "lock-key"
	| "lock-key-open"
	| "lock-laminated"
	| "lock-laminated-open"
	| "lock-open"
	| "lock-simple"
	| "lock-simple-open"
	| "lockers"
	| "log"
	| "magic-wand"
	| "magnet"
	| "magnet-straight"
	| "magnifying-glass"
	| "magnifying-glass-minus"
	| "magnifying-glass-plus"
	| "mailbox"
	| "map-pin"
	| "map-pin-area"
	| "map-pin-line"
	| "map-pin-plus"
	| "map-pin-simple"
	| "map-pin-simple-area"
	| "map-pin-simple-line"
	| "map-trifold"
	| "markdown-logo"
	| "marker-circle"
	| "martini"
	| "mask-happy"
	| "mask-sad"
	| "mastodon-logo"
	| "math-operations"
	| "matrix-logo"
	| "medal"
	| "medal-military"
	| "medium-logo"
	| "megaphone"
	| "megaphone-simple"
	| "member-of"
	| "memory"
	| "messenger-logo"
	| "meta-logo"
	| "meteor"
	| "metronome"
	| "microphone"
	| "microphone-slash"
	| "microphone-stage"
	| "microscope"
	| "microsoft-excel-logo"
	| "microsoft-outlook-logo"
	| "microsoft-powerpoint-logo"
	| "microsoft-teams-logo"
	| "microsoft-word-logo"
	| "minus"
	| "minus-circle"
	| "minus-square"
	| "money"
	| "money-wavy"
	| "monitor"
	| "monitor-arrow-up"
	| "monitor-play"
	| "moon"
	| "moon-stars"
	| "moped"
	| "moped-front"
	| "mosque"
	| "motorcycle"
	| "mountains"
	| "mouse"
	| "mouse-left-click"
	| "mouse-middle-click"
	| "mouse-right-click"
	| "mouse-scroll"
	| "mouse-simple"
	| "music-note"
	| "music-note-simple"
	| "music-notes"
	| "music-notes-minus"
	| "music-notes-plus"
	| "music-notes-simple"
	| "navigation-arrow"
	| "needle"
	| "network"
	| "network-slash"
	| "network-x"
	| "newspaper"
	| "newspaper-clipping"
	| "not-equals"
	| "not-member-of"
	| "not-subset-of"
	| "not-superset-of"
	| "notches"
	| "note"
	| "note-blank"
	| "note-pencil"
	| "notebook"
	| "notepad"
	| "notification"
	| "notion-logo"
	| "nuclear-plant"
	| "number-circle-eight"
	| "number-circle-five"
	| "number-circle-four"
	| "number-circle-nine"
	| "number-circle-one"
	| "number-circle-seven"
	| "number-circle-six"
	| "number-circle-three"
	| "number-circle-two"
	| "number-circle-zero"
	| "number-eight"
	| "number-five"
	| "number-four"
	| "number-nine"
	| "number-one"
	| "number-seven"
	| "number-six"
	| "number-square-eight"
	| "number-square-five"
	| "number-square-four"
	| "number-square-nine"
	| "number-square-one"
	| "number-square-seven"
	| "number-square-six"
	| "number-square-three"
	| "number-square-two"
	| "number-square-zero"
	| "number-three"
	| "number-two"
	| "number-zero"
	| "numpad"
	| "nut"
	| "ny-times-logo"
	| "octagon"
	| "office-chair"
	| "onigiri"
	| "open-ai-logo"
	| "option"
	| "orange"
	| "orange-slice"
	| "oven"
	| "package"
	| "paint-brush"
	| "paint-brush-broad"
	| "paint-brush-household"
	| "paint-bucket"
	| "paint-roller"
	| "palette"
	| "panorama"
	| "pants"
	| "paper-plane"
	| "paper-plane-right"
	| "paper-plane-tilt"
	| "paperclip"
	| "paperclip-horizontal"
	| "parachute"
	| "paragraph"
	| "parallelogram"
	| "park"
	| "password"
	| "path"
	| "patreon-logo"
	| "pause"
	| "pause-circle"
	| "paw-print"
	| "paypal-logo"
	| "peace"
	| "pen"
	| "pen-nib"
	| "pen-nib-straight"
	| "pencil"
	| "pencil-circle"
	| "pencil-line"
	| "pencil-ruler"
	| "pencil-simple"
	| "pencil-simple-line"
	| "pencil-simple-slash"
	| "pencil-slash"
	| "pentagon"
	| "pentagram"
	| "pepper"
	| "percent"
	| "person"
	| "person-arms-spread"
	| "person-simple"
	| "person-simple-bike"
	| "person-simple-circle"
	| "person-simple-hike"
	| "person-simple-run"
	| "person-simple-ski"
	| "person-simple-snowboard"
	| "person-simple-swim"
	| "person-simple-tai-chi"
	| "person-simple-throw"
	| "person-simple-walk"
	| "perspective"
	| "phone"
	| "phone-call"
	| "phone-disconnect"
	| "phone-incoming"
	| "phone-list"
	| "phone-outgoing"
	| "phone-pause"
	| "phone-plus"
	| "phone-slash"
	| "phone-transfer"
	| "phone-x"
	| "phosphor-logo"
	| "pi"
	| "piano-keys"
	| "picnic-table"
	| "picture-in-picture"
	| "piggy-bank"
	| "pill"
	| "ping-pong"
	| "pint-glass"
	| "pinterest-logo"
	| "pinwheel"
	| "pipe"
	| "pipe-wrench"
	| "pix-logo"
	| "pizza"
	| "placeholder"
	| "planet"
	| "plant"
	| "play"
	| "play-circle"
	| "play-pause"
	| "playlist"
	| "plug"
	| "plug-charging"
	| "plugs"
	| "plugs-connected"
	| "plus"
	| "plus-circle"
	| "plus-minus"
	| "plus-square"
	| "poker-chip"
	| "police-car"
	| "polygon"
	| "popcorn"
	| "popsicle"
	| "potted-plant"
	| "power"
	| "prescription"
	| "presentation"
	| "presentation-chart"
	| "printer"
	| "prohibit"
	| "prohibit-inset"
	| "projector-screen"
	| "projector-screen-chart"
	| "pulse"
	| "activity"
	| "push-pin"
	| "push-pin-simple"
	| "push-pin-simple-slash"
	| "push-pin-slash"
	| "puzzle-piece"
	| "qr-code"
	| "question"
	| "question-mark"
	| "queue"
	| "quotes"
	| "rabbit"
	| "racquet"
	| "radical"
	| "radio"
	| "radio-button"
	| "radioactive"
	| "rainbow"
	| "rainbow-cloud"
	| "ranking"
	| "read-cv-logo"
	| "receipt"
	| "receipt-x"
	| "record"
	| "rectangle"
	| "rectangle-dashed"
	| "recycle"
	| "reddit-logo"
	| "repeat"
	| "repeat-once"
	| "replit-logo"
	| "resize"
	| "rewind"
	| "rewind-circle"
	| "road-horizon"
	| "robot"
	| "rocket"
	| "rocket-launch"
	| "rows"
	| "rows-plus-bottom"
	| "rows-plus-top"
	| "rss"
	| "rss-simple"
	| "rug"
	| "ruler"
	| "sailboat"
	| "scales"
	| "scan"
	| "scan-smiley"
	| "scissors"
	| "scooter"
	| "screencast"
	| "screwdriver"
	| "scribble"
	| "scribble-loop"
	| "scroll"
	| "seal"
	| "circle-wavy"
	| "seal-check"
	| "circle-wavy-check"
	| "seal-percent"
	| "seal-question"
	| "circle-wavy-question"
	| "seal-warning"
	| "circle-wavy-warning"
	| "seat"
	| "seatbelt"
	| "security-camera"
	| "selection"
	| "selection-all"
	| "selection-background"
	| "selection-foreground"
	| "selection-inverse"
	| "selection-plus"
	| "selection-slash"
	| "shapes"
	| "share"
	| "share-fat"
	| "share-network"
	| "shield"
	| "shield-check"
	| "shield-checkered"
	| "shield-chevron"
	| "shield-plus"
	| "shield-slash"
	| "shield-star"
	| "shield-warning"
	| "shipping-container"
	| "shirt-folded"
	| "shooting-star"
	| "shopping-bag"
	| "shopping-bag-open"
	| "shopping-cart"
	| "shopping-cart-simple"
	| "shovel"
	| "shower"
	| "shrimp"
	| "shuffle"
	| "shuffle-angular"
	| "shuffle-simple"
	| "sidebar"
	| "sidebar-simple"
	| "sigma"
	| "sign-in"
	| "sign-out"
	| "signature"
	| "signpost"
	| "sim-card"
	| "siren"
	| "sketch-logo"
	| "skip-back"
	| "skip-back-circle"
	| "skip-forward"
	| "skip-forward-circle"
	| "skull"
	| "skype-logo"
	| "slack-logo"
	| "sliders"
	| "sliders-horizontal"
	| "slideshow"
	| "smiley"
	| "smiley-angry"
	| "smiley-blank"
	| "smiley-meh"
	| "smiley-melting"
	| "smiley-nervous"
	| "smiley-sad"
	| "smiley-sticker"
	| "smiley-wink"
	| "smiley-x-eyes"
	| "snapchat-logo"
	| "sneaker"
	| "sneaker-move"
	| "snowflake"
	| "soccer-ball"
	| "sock"
	| "solar-panel"
	| "solar-roof"
	| "sort-ascending"
	| "sort-descending"
	| "soundcloud-logo"
	| "spade"
	| "sparkle"
	| "speaker-hifi"
	| "speaker-high"
	| "speaker-low"
	| "speaker-none"
	| "speaker-simple-high"
	| "speaker-simple-low"
	| "speaker-simple-none"
	| "speaker-simple-slash"
	| "speaker-simple-x"
	| "speaker-slash"
	| "speaker-x"
	| "speedometer"
	| "sphere"
	| "spinner"
	| "spinner-ball"
	| "spinner-gap"
	| "spiral"
	| "split-horizontal"
	| "split-vertical"
	| "spotify-logo"
	| "spray-bottle"
	| "square"
	| "square-half"
	| "square-half-bottom"
	| "square-logo"
	| "square-split-horizontal"
	| "square-split-vertical"
	| "squares-four"
	| "stack"
	| "stack-minus"
	| "stack-overflow-logo"
	| "stack-plus"
	| "stack-simple"
	| "stairs"
	| "stamp"
	| "standard-definition"
	| "star"
	| "star-and-crescent"
	| "star-four"
	| "star-half"
	| "star-of-david"
	| "steam-logo"
	| "steering-wheel"
	| "steps"
	| "stethoscope"
	| "sticker"
	| "stool"
	| "stop"
	| "stop-circle"
	| "storefront"
	| "strategy"
	| "stripe-logo"
	| "student"
	| "subset-of"
	| "subset-proper-of"
	| "subtitles"
	| "subtitles-slash"
	| "subtract"
	| "subtract-square"
	| "subway"
	| "suitcase"
	| "suitcase-rolling"
	| "suitcase-simple"
	| "sun"
	| "sun-dim"
	| "sun-horizon"
	| "sunglasses"
	| "superset-of"
	| "superset-proper-of"
	| "swap"
	| "swatches"
	| "swimming-pool"
	| "sword"
	| "synagogue"
	| "syringe"
	| "t-shirt"
	| "table"
	| "tabs"
	| "tag"
	| "tag-chevron"
	| "tag-simple"
	| "target"
	| "taxi"
	| "tea-bag"
	| "telegram-logo"
	| "television"
	| "television-simple"
	| "tennis-ball"
	| "tent"
	| "terminal"
	| "terminal-window"
	| "test-tube"
	| "text-a-underline"
	| "text-aa"
	| "text-align-center"
	| "text-align-justify"
	| "text-align-left"
	| "text-align-right"
	| "text-b"
	| "text-bolder"
	| "text-columns"
	| "text-h"
	| "text-h-five"
	| "text-h-four"
	| "text-h-one"
	| "text-h-six"
	| "text-h-three"
	| "text-h-two"
	| "text-indent"
	| "text-italic"
	| "text-outdent"
	| "text-strikethrough"
	| "text-subscript"
	| "text-superscript"
	| "text-t"
	| "text-t-slash"
	| "text-underline"
	| "textbox"
	| "thermometer"
	| "thermometer-cold"
	| "thermometer-hot"
	| "thermometer-simple"
	| "threads-logo"
	| "three-d"
	| "thumbs-down"
	| "thumbs-up"
	| "ticket"
	| "tidal-logo"
	| "tiktok-logo"
	| "tilde"
	| "timer"
	| "tip-jar"
	| "tipi"
	| "tire"
	| "toggle-left"
	| "toggle-right"
	| "toilet"
	| "toilet-paper"
	| "toolbox"
	| "tooth"
	| "tornado"
	| "tote"
	| "tote-simple"
	| "towel"
	| "tractor"
	| "trademark"
	| "trademark-registered"
	| "traffic-cone"
	| "traffic-sign"
	| "traffic-signal"
	| "train"
	| "train-regional"
	| "train-simple"
	| "tram"
	| "translate"
	| "trash"
	| "trash-simple"
	| "tray"
	| "tray-arrow-down"
	| "archive-tray"
	| "tray-arrow-up"
	| "treasure-chest"
	| "tree"
	| "tree-evergreen"
	| "tree-palm"
	| "tree-structure"
	| "tree-view"
	| "trend-down"
	| "trend-up"
	| "triangle"
	| "triangle-dashed"
	| "trolley"
	| "trolley-suitcase"
	| "trophy"
	| "truck"
	| "truck-trailer"
	| "tumblr-logo"
	| "twitch-logo"
	| "twitter-logo"
	| "umbrella"
	| "umbrella-simple"
	| "union"
	| "unite"
	| "unite-square"
	| "upload"
	| "upload-simple"
	| "usb"
	| "user"
	| "user-check"
	| "user-circle"
	| "user-circle-check"
	| "user-circle-dashed"
	| "user-circle-gear"
	| "user-circle-minus"
	| "user-circle-plus"
	| "user-focus"
	| "user-gear"
	| "user-list"
	| "user-minus"
	| "user-plus"
	| "user-rectangle"
	| "user-sound"
	| "user-square"
	| "user-switch"
	| "users"
	| "users-four"
	| "users-three"
	| "van"
	| "vault"
	| "vector-three"
	| "vector-two"
	| "vibrate"
	| "video"
	| "video-camera"
	| "video-camera-slash"
	| "video-conference"
	| "vignette"
	| "vinyl-record"
	| "virtual-reality"
	| "virus"
	| "visor"
	| "voicemail"
	| "volleyball"
	| "wall"
	| "wallet"
	| "warehouse"
	| "warning"
	| "warning-circle"
	| "warning-diamond"
	| "warning-octagon"
	| "washing-machine"
	| "watch"
	| "wave-sawtooth"
	| "wave-sine"
	| "wave-square"
	| "wave-triangle"
	| "waveform"
	| "waveform-slash"
	| "waves"
	| "webcam"
	| "webcam-slash"
	| "webhooks-logo"
	| "wechat-logo"
	| "whatsapp-logo"
	| "wheelchair"
	| "wheelchair-motion"
	| "wifi-high"
	| "wifi-low"
	| "wifi-medium"
	| "wifi-none"
	| "wifi-slash"
	| "wifi-x"
	| "wind"
	| "windmill"
	| "windows-logo"
	| "wine"
	| "wrench"
	| "x"
	| "x-circle"
	| "x-logo"
	| "x-square"
	| "yarn"
	| "yin-yang"
	| "youtube-logo";
//#endregion icons
