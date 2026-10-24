// Shelv's default settings: the base layer, evaluated before the settings note.
// Shelv embeds this file as is and strips its types when it loads (erasable TypeScript only).
//
// These are *all* of Shelv's default commands, bindings and slash-palette entries; nothing else is
// hardcoded, including the descriptions shown in the palette and in hints. It is written with the
// public API only, so it doubles as the reference: anything Shelv does by default, a settings block can
// do too.
//
// It is also the module "shelv/defaults": every command below is exported, so the settings note can
// reuse the same command (`import * as defaults from "shelv/defaults"`). Binding a default command in the
// note moves it off its default key, `{ key, command: null }` frees a key, and `useDefaults: false`
// drops this whole layer. Spec: docs/settings-js/spec.md (section 5 lists the same table, section 2 the
// merge rules).

import { action, command, Icon, type NoteIndex, type Settings } from "shelv";

// --- slash-palette commands ----------------------------------------------------------------------

export const ai = command("ai", {
	icon: Icon.phosphor("sparkle"),
	description: "Show AI Prompt",
	run: action.ShowPrompt,
});
export const code = command("code", {
	icon: Icon.phosphor("code-block"),
	description: "Toggle Code Block",
	run: action.MarkdownCodeBlock,
});
export const h1 = command("h1", {
	icon: Icon.phosphor("text-h-one"),
	description: "Heading 1",
	run: action.MarkdownH1,
});
export const h2 = command("h2", {
	icon: Icon.phosphor("text-h-two"),
	description: "Heading 2",
	run: action.MarkdownH2,
});
export const h3 = command("h3", {
	icon: Icon.phosphor("text-h-three"),
	description: "Heading 3",
	run: action.MarkdownH3,
});
export const bold = command("bold", {
	icon: Icon.phosphor("text-bolder"),
	description: "Toggle Bold",
	run: action.MarkdownBold,
});
export const italic = command("italic", {
	icon: Icon.phosphor("text-italic"),
	description: "Toggle Italic",
	run: action.MarkdownItalic,
});
export const strike = command("strike", {
	icon: Icon.phosphor("text-strikethrough"),
	description: "Toggle Strikethrough",
	run: action.MarkdownStrikethrough,
});
export const jump = command("jump", {
	icon: Icon.phosphor("cursor-click"),
	description: "Activate Word Jump",
	run: action.StartWordJump,
});

// --- key-only commands (not in the palette) ------------------------------------------------------

// lists
export const taskMarker = command("task-marker", {
	description: "Expand Task Marker",
	run: action.ExpandTaskMarker,
});
export const indent = command("indent", {
	description: "Increase List Item indentation",
	run: action.IndentListItem,
});
export const unindent = command("unindent", {
	description: "Decrease List Item indentation",
	run: action.UnindentListItem,
});
export const splitListItem = command("split-list-item", {
	description: "Split List item at cursor position",
	run: action.SplitListItem,
});

// structural editing
export const expandSelection = command("expand-selection", {
	description: "Expand Selection",
	run: action.ExpandSelection,
});
export const shrinkSelection = command("shrink-selection", {
	description: "Shrink Selection",
	run: action.ShrinkSelection,
});
export const moveLinesUp = command("move-lines-up", {
	description: "Move Lines Up",
	run: action.MoveLinesUp,
});
export const moveLinesDown = command("move-lines-down", {
	description: "Move Lines Down",
	run: action.MoveLinesDown,
});

// palette
export const slashPalette = command("slash-palette", {
	description: "Show slash command palette",
	run: action.ShowSlashPalette,
});

// notes and window
const shelf = (index: NoteIndex) =>
	command(`shelf-${index}`, {
		description: `Shelf ${index}`,
		run: action.SwitchToNote({ index }),
	});
export const shelf1 = shelf(1);
export const shelf2 = shelf(2);
export const shelf3 = shelf(3);
export const shelf4 = shelf(4);
export const settings = command("settings", {
	description: "Open Settings",
	run: action.SwitchToSettings,
});
export const openFile = command("open-file", {
	description: "Open File",
	run: action.OpenFileDialog,
});
export const closeFile = command("close-file", {
	description: "Close File",
	run: action.CloseCurrentNote,
});
export const pin = command("pin", {
	description: "Toggle Always on Top",
	run: action.PinWindow,
});
export const hide = command("hide", {
	description: "Hide Window",
	run: action.HideApp,
});

// dev
export const debugTools = command("debug-tools", {
	description: "Toggle Debug Tools",
	run: action.ToggleDebugTools,
});

// global
export const showHide = command("show-hide", {
	description: "Show / Hide Shelv",
	run: action.ShowHideApp,
});

export default {
	// the slash palette, in this order
	commands: [ai, code, h1, h2, h3, bold, italic, strike, jump],

	global: [
		// show / hide Shelv from anywhere
		{ key: "Cmd Option S", command: showHide },
	],

	bindings: [
		// shortcuts for the palette commands
		{ key: "Ctrl Enter", command: ai },
		{ key: "Cmd Option B", command: code },
		{ key: "Cmd Option 1", command: h1 },
		{ key: "Cmd Option 2", command: h2 },
		{ key: "Cmd Option 3", command: h3 },
		{ key: "Cmd B", command: bold },
		{ key: "Cmd I", command: italic },
		{ key: "Cmd Shift E", command: strike },
		{ key: "Cmd J", command: jump },

		// lists
		{ key: "Space", command: taskMarker },
		{ key: "Tab", command: indent },
		{ key: "Shift Tab", command: unindent },
		{ key: "Enter", command: splitListItem },

		// structural editing
		{ key: "Option O", command: expandSelection },
		{ key: "Option I", command: shrinkSelection },
		{ key: "Option K", command: moveLinesUp },
		{ key: "Option J", command: moveLinesDown },

		// palette
		{ key: "/", command: slashPalette },

		// notes and window
		{ key: "Cmd 1", command: shelf1 },
		{ key: "Cmd 2", command: shelf2 },
		{ key: "Cmd 3", command: shelf3 },
		{ key: "Cmd 4", command: shelf4 },
		{ key: "Cmd Comma", command: settings },
		{ key: "Cmd O", command: openFile },
		{ key: "Cmd W", command: closeFile },
		{ key: "Cmd P", command: pin },
		{ key: "Escape", command: hide },

		// dev
		{ key: "Ctrl Option Shift D", command: debugTools },
	],
} satisfies Settings;
