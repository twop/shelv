# Welcome to **settings**

Yep, settings is just another note! (Note: A more comprehensive list of settings is coming soon)
Feel free to ask questions on [discord](https://discord.gg/sSGHwNKy)

- [ ] Set up your global hotkey for `ShowHideApp`
	* [ ] Try showing and hiding Shelv by pressing your new shortcut

## Global shortcuts

```settings
// <-- Tip: Remove the "//" part on the line below to enable
// global "Cmd Option S" {ShowHideApp;}

```

**Supported Shortcuts**
- `ShowHideApp`: Global shortcut that shows or hides Shelv

For configuring your own shortcuts, use the following expanded keywords:
- `⇧` -> `Shift`,
- `⌘` -> `Cmd`
- `⌥` -> `Option`,
- `⌃` -> `Ctrl`
- `⏎` -> `Enter`
- Alpha-numeric keys -> `A-Z` or `0-9`


## AI configuration

```settings
ai {
	model "claude-3-haiku-20240307"
    // Or "claude-3-5-sonnet-20240620"

	systemPrompt r#"
    	You are a helpful assistant in tech. Be very concise with your answers
    "#
}
```

**System Prompt**
- The `systemPrompt` sets the initial context for all AI requests 
	* We've found "be concise" to be very helpful, feel free to play around with it!

***Tip**: Haiku model is the cheapest for us to run, but you can also try sonnet (commented out in the settings block below). It's is slower and more expensive for us, but more accurate.*
