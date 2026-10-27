// TEMPORARY: the current Shelv settings note as one TypeScript module, for iterating on the API.
// Type-checked by `just typecheck-settings` (src/scripting/tsconfig.json); not built, not embedded.
//
// In the note, every fenced block is its own module with its own `export default`; here the blocks
// are `Settings` values, exported together as one array (merged in order, `ai` last-wins).

import {
	action,
	command,
	Icon,
	secret,
	selection,
	type Settings,
} from "./system";

// =================================================================================================
// ## Shortcuts
// =================================================================================================

// All changes inside this block will apply immediately!

// Format: { key: "[key1] [key2] ...", command: someCommand }, with someCommand = command(alias, { run })
// Alt keys: Shift (⇧), Cmd (⌘), Option (⌥), Ctrl (⌃), Enter (⏎)
// Alpha-numeric keys: A-Z or `0-9`

const showHide = command("show-hide", { run: action.ShowHideApp });

// Toggle Code Block: Creates or removes code block annotations
const codeBlock = command("code-block", {
	description: "Toggle Code Block",
	run: action.MarkdownCodeBlock,
});

const shortcuts = {
	global: [{ key: "Cmd Ctrl Option Shift S", command: showHide }],
	bindings: [{ key: "Cmd Shift B", command: codeBlock }],
} satisfies Settings;

// toggle javascript code block
export const jsBlock = command("js", {
	icon: Icon.phosphor("file-js"),
	description: "Insert javascript code block",
	run: action.MarkdownCodeBlock({ lang: "js" }),
});

const jsBlockBinding = {
	commands: [jsBlock],
	bindings: [{ key: "Cmd Shift J", command: jsBlock }],
} satisfies Settings;

// =================================================================================================
// ## Snippets
// =================================================================================================

const monthNames = [
	"jan",
	"feb",
	"mar",
	"apr",
	"may",
	"jun",
	"jul",
	"aug",
	"sep",
	"oct",
	"nov",
	"dec",
];

export function getCurrentDate(): string {
	const now = new Date();
	const year = now.getFullYear();
	const month = monthNames[now.getMonth()];
	// Ensures the day is 2 digits, adding leading zero if needed(2, '0');
	const day = String(now.getDate()).padStart(2, "0");
	return `${year}/${month}/${day}`;
}

// (⌘ Y): Insert result from: getCurrentDate
export const insertDate = command("date", {
	icon: Icon.phosphor("calendar-heart"),
	description: "Insert current date (YYYY/mon/DD)",
	run: () => action.InsertText({ text: getCurrentDate() }),
});

export function getCurrentTime(): string {
	const now = new Date();
	let hours = now.getHours();
	const minutes = String(now.getMinutes()).padStart(2, "0");
	const ampm = hours >= 12 ? "pm" : "am";

	// Convert to 12-hour format
	hours = hours % 12;
	hours = hours ? hours : 12; // handle midnight (0)

	return `${hours}:${minutes}${ampm}`;
}

getCurrentTime();

export const insertTime = command("time", {
	icon: Icon.phosphor("clock"),
	description: "Insert current time (hh:mm am/pm)",
	run: () => action.InsertText({ text: getCurrentTime() }),
});

// (Cmd K): Insert Markdown Link
export const insertLink = command("link", {
	icon: Icon.phosphor("link"),
	description: "Insert Markdown Link",
	needs: [selection],
	run: (text) => action.InsertText({ text: `[${text}]({||})` }),
});

export function wrapInBackticks(selectedText: string): string {
	if (!selectedText) {
		return "`{||}`";
	}
	return `\`${selectedText}\`{||}`;
}

export const backticks = command("backticks", {
	icon: Icon.phosphor("code"),
	description: "Wrap in backticks",
	needs: [selection],
	run: (text) => action.InsertText({ text: wrapInBackticks(text) }),
});

const snippets = {
	commands: [insertDate, insertTime, insertLink, backticks],
	bindings: [
		{ key: "Cmd Y", command: insertDate },
		{ key: "Cmd T", command: insertTime },
		{ key: "Cmd K", command: insertLink },
		{ key: "Cmd /", command: backticks },
	],
} satisfies Settings;

/// ------------------------------------------------------------------------------------------------
/// ------------------------------------------------------------------------------------------------
///
///   EXPLORATION: things KDL exposes today that the settings above do not use.
///   Everything below this banner is new; everything above maps the current note exactly.
///
/// ------------------------------------------------------------------------------------------------
/// ------------------------------------------------------------------------------------------------

// --- AI settings: today `ai { model "..." systemPrompt "..." token "..." useShelvSystemPrompt #true }`
// Without `ai` settings Shelv uses its proxy model "shelv-claude".
const aiSettings = {
	ai: {
		model: "gpt-4o-mini", // any genai model name, or "shelv-claude"
		systemPrompt: "Answer tersely. Prefer bullet lists.",
		token: secret("OPENAI_API_KEY"), // today: a plain string in the note
		useShelvSystemPrompt: true, // default true
	},
} satisfies Settings;

// --- palette-only command: KDL `bind alias="..." { ... }` without a key = listed in `commands` only
export const pinCmd = command("pin", {
	icon: Icon.phosphor("push-pin"),
	description: "Keep Shelv on top",
	run: action.PinWindow,
});

// --- string export used as text: KDL `callFunc "signature"` on `export const signature = "..."`
export const signature = "-- sent from Shelv{||}";
export const insertSignature = command("sig", {
	icon: Icon.phosphor("signature"),
	run: action.InsertText({ text: signature }),
});

// --- actions with parameters
export const toNote1 = command("n1", {
	icon: Icon.phosphor("note"),
	run: action.SwitchToNote({ index: 1 }),
});

// --- every other binding needs a command too (actions are not bindable on their own)
const bold = command("bold", { run: action.MarkdownBold });
const italic = command("italic", { run: action.MarkdownItalic });
const strike = command("strike", { run: action.MarkdownStrikethrough });
const h1 = command("h1", { run: action.MarkdownH1 });
const h2 = command("h2", { run: action.MarkdownH2 });
const h3 = command("h3", { run: action.MarkdownH3 });
const toNote2 = command("n2", { run: action.SwitchToNote({ index: 2 }) });
const settings = command("settings", { run: action.SwitchToSettings });
const closeNote = command("close", { run: action.CloseCurrentNote });
const openFile = command("open", { run: action.OpenFileDialog });
const hide = command("hide", { run: action.HideApp });
const prompt = command("ai", { run: action.ShowPrompt });
const jump = command("jump", { run: action.StartWordJump });
const expandSelection = command("expand", { run: action.ExpandSelection });
const shrinkSelection = command("shrink", { run: action.ShrinkSelection });
const moveUp = command("move-up", { run: action.MoveLinesUp });
const moveDown = command("move-down", { run: action.MoveLinesDown });
const debugTools = command("debug", { run: action.ToggleDebugTools });

const builtins = {
	commands: [pinCmd],
	bindings: [
		// text formatting
		{ key: "Cmd B", command: bold },
		{ key: "Cmd I", command: italic },
		{ key: "Cmd Shift X", command: strike },
		{ key: "Cmd Option 1", command: h1 },
		{ key: "Cmd Option 2", command: h2 },
		{ key: "Cmd Option 3", command: h3 },

		// notes and window
		{ key: "Cmd 1", command: toNote1 },
		{ key: "Cmd 2", command: toNote2 },
		{ key: "Cmd Comma", command: settings },
		{ key: "Cmd W", command: closeNote },
		{ key: "Cmd O", command: openFile },
		{ key: "Cmd Shift H", command: hide },

		// AI and navigation
		{ key: "Ctrl Enter", command: prompt },
		{ key: "Ctrl J", command: jump },

		// structural editing
		{ key: "Option Up", command: expandSelection },
		{ key: "Option Down", command: shrinkSelection },
		{ key: "Option Shift Up", command: moveUp },
		{ key: "Option Shift Down", command: moveDown },

		// dev
		{ key: "Cmd Option I", command: debugTools },

		{ key: "Cmd Shift S", command: insertSignature },
	],
} satisfies Settings;

/// ------------------------------------------------------------------------------------------------
///   BEYOND KDL: not expressible today at all (see settings-js-07).
/// ------------------------------------------------------------------------------------------------

// several steps in one command (KDL: "Currently only 1 command per binding is supported")
export const stamp = command("stamp", {
	icon: Icon.phosphor("clock"),
	description: "Insert date and time",
	run: () => [
		action.InsertText({ text: getCurrentDate() }),
		action.InsertText({ text: " " }),
		action.InsertText({ text: getCurrentTime() }),
	],
});

// Not in v1 (spec section 10), kept from the sketch for reference:
//
// // clock as an input instead of `new Date()` inside the function: testable (`now` capability)
// export const insertIsoDate = command("iso-date", {
// 	needs: [now],
// 	run: (ms) => action.InsertText({ text: new Date(ms).toISOString().slice(0, 10) }),
// });
//
// // AI flow with user input (`flow`, `ask`, `ai`, named `needs`)
// export const rewrite = command("rewrite", {
// 	icon: Icon.phosphor("magic-wand"),
// 	description: "Rewrite selection with AI",
// 	needs: { text: selection.required, how: ask.for("How should I rewrite it?"), ai },
// 	async flow({ text, how, ai }) {
// 		let draft = await ai.complete({ prompt: `${how}\n---\n${text}` });
// 		while ((await ask.choose(draft, ["Insert", "Again"])) === "Again") {
// 			draft = await ai.complete({ prompt: `Rewrite differently:\n${draft}` });
// 		}
// 		return action.InsertText({ text: draft });
// 	},
// });

const beyondKdl = {
	commands: [stamp],
	bindings: [{ key: "Cmd Shift T", command: stamp }],
} satisfies Settings;

export default [
	shortcuts,
	jsBlockBinding,
	snippets, // the current note
	aiSettings,
	builtins,
	beyondKdl, // exploration
] satisfies Settings[];
