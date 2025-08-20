# Shelv Website Content Template

## Hackable, Local,  AI-powered notes
Shelv is scriptable, plain text notes app with integrated ai-features for macOS, written in Rust (btw (tm)). 

### screenshot
Prompt, code block, Markdown, TBD the exact content

---

## Hackable, Local, AI-Powered Notes
Shelv is a scriptable, plain text notes app with integrated AI features for macOS, written in Rust (by the way (tm)). 

### Screenshot
Prompt, code block, Markdown, [with] TBD the exact content

---

## Hack It, Make It Yours
Settings in Shelv is just a note, where you can create custom commands with [KDL](https://kdl.dev/) and JavaScript, assign and tweak keyboard shortcuts, all with live reload.

The origin story: at the time I used [Bear app](https://bear.app/), which has 4 versions of date, but I wanted it in YYYY/mmm/dd format, and I keep thinking: "if only I can just define what I want". Well, with Shelv you can.

#### screenshot/video
Prompt with Add a shortcut for "day" command and using it, first with slash command then with shortcut
**Type**: Animated GIF
**Content**: Demo showing:
1. Quick prompt to create a "day" insert feature
2. Triggering the new feature via keyboard shortcut
3. Using the same feature via slash menu
**Alt Text**: "Creating and using a custom 'day' command via shortcuts and slash menu"
**TODO**: Record this demo GIF

---

## Markdown essentials and more
>> notes: bold should be paragraph size, but description should be footnote fontsize, starting on new line

- **Markdown Support**: Full CommonMark with extensions, including TODOs
- **Code Syntax Highlighting**: A lot of languages are supported
- **Live JavaScript Blocks**: Execute JS code directly in notes
- **Slash Menu**: Quick access to all commands and features
- **Keyboard optimized**:  Everything is available via shortcuts


#### GIF/Screenshot
**Type**: Animated GIF  
**Content**: Demo showing:
1. Creating a live JavaScript block via slash menu
2. Writing and executing JavaScript code
3. Quick prompt to convert bullet list to numbered list
**Alt Text**: "Creating live JavaScript code and converting list formats with AI"
**TODO**: Record this demo GIF


### Frequently Asked Questions

- Is Shelv coming to Mobile/Window/Web
	* Yes, but with time. Shelv is written in Rust + [egui](https://egui.rs/), so it is possible to port it as is on all these platforms
- How do you make money?
	* I don't. I worked on Shelv for over 2 years, and I had a dream to start company(still do), but as of now, it is a labor of love, because I couldn't find a good business model, if you have ideas please let me know. Tentatively I plan to add ability just to buy tokens, but that seems lame. I plan to cap to $20/month the claude account assosiated with the app, but you can choose your providers for AI features, includind [Ollama](https://ollama.com/).
- Do you have sync?
	* Not yet, I'm a local first movement fan, and wanted to use [Automerge](https://github.com/automerge/automerge) forever, but I want to implement e2e encryption with Rust sever, which is being worked on right now, and it is darn had to do an e2e encrypted scalable sync technically and from product point of view.
- Is Shelv open source?
	* Yes and no, it has a licence inspired by  [ PolyForm Strict 1.0.0 license](https://polyformproject.org/licenses/strict/1.0.0). Which means that you cannot use Shelv compiled from source for work or repackage it to a new app. However that applies to the "build from" source option, you can (and hopefully will) just use the version from the app store.
- Is it Native?
	* Native is a spectrum, shelv is written in Rust using [egui](https://egui.rs/) as the gui toolkit, which in turn is using wgpu, not Swift UI tech stack. Maybe the closest analogy would be [Flutter](https://flutter.dev/) that is painting every pixel. Are Flutter apps native? I think so.
- Are my beloved vim motions supported?
	* I am a [Helix](https://helix-editor.com/) user myself, but markdown and text are a bit different from code, that said, I would love to support modal editing in the future. I do think that some features can be added for just "insert" mode (which is the only mode at the moment) that can enhance editing, for example: jump to a word, press any buttons with a label(vimium style), expand + shrink semantic selection etc. I need to work on Shelv full-time to justify adding vim or helix motions to egui TextEdit, vote with you money I guess, oh wait, I don't have a way to actually recieve money...
- Are you collecting any analytics?
	* Not at the moment (besides crash reporting), I'm not fundamentally opposed to collecting statistics, because it is hard to know if a feature is even used without some observability, but I do think it can be done with privacy in mind (at least anonymizing and being mindful of where the data is stored). Probably in the future, however, when and if I add monetization, I'll likely start collecting emails associated with a purchase and/or install

### Roadmap
- Done:
	* Initial launch on macOS: Aug 2025
		* Barebones editing with 4 notes
		* Optimized for quick capture
		* No API exposed to JS scripts
- Coming:
	* Multi-file + workspace support
		* Workspace folder with notes inside
		* Import from [Obsidian](https://obsidian.md/)
		* File tree + workspace viewer
	* Agentic mode
		* Tools/MCP that allow to search/move/create/edit notes
		* UI for having agentic workflows, probably just a chat that is going to be just another file
		* Files that define custom workflows, similar to Claude Code
	* Core editing features:
		* Semantic selection: expand and shrink cursor selection with markdown AST nodes
		* Jump to an *element*, jump to any word on the screen with a couple of keystrokes (similar to Vimium and Helix)
		* Search, Redo etc
	* Support for pasting/rendering images
	* Rich API exposed to JS + better scripting capabilities (like sharing code among notes)
	* Sync 
		* I plan to use [Automerge](https://github.com/automerge/automerge) for personal syncing, which can be also used for collaboration
		* Dump to git, e.g. backup all the notes to git, potentially with AI-generated change summary
	* Web version
		* Mobile (including web) version is TBD
	* Collaboration
		* Share a note via link (co-editing on the web)
		* Share workspace, that is, co-ownership of a collection of folder+notes

### Footer
>> keep as is

---
>> Some editing notes

### Tone of Voice (for AI editing)
- **Technical but written in a fun way**
- **Authentic and open**: Avoid marketing fluff, be direct
