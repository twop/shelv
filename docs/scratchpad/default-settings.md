# Welcome to **settings**

In Shelv, settings is just another note. All configuration is done inside codeblocks marked as `settings` (you'll learn more about codeblocks later). **Any changes are automatically applied**.

- [ ] **Assign a custom global hotkey** by modifying the key bindings for `ShowHideApp` in the block below.
- [ ] **Test out your new hotkey** to hide Shelv, and again to bring Shelv back.

Once you are done, go back to onboarding by [clicking here!](shelv://note1)

```settings
global "Cmd Shift M" {ShowHideApp;}

```
```settings#44a6
applied
```

*Format: Alt keys are `Shift (⇧)`, `Cmd (⌘)`, `Option (⌥)`, `Ctrl (⌃)`, `Enter (⏎)`, and alpha-numeric keys are `A-Z` or `0-9`*










## Advanced configuration

```settings
ai {
	model "claude-3-haiku-20240307"
    // Or "claude-3-5-sonnet-20240620"

	systemPrompt r#"
    	You are a helpful assistant in tech. Be very concise with your answers
    "#
}
```
```settings#3108
applied
```

**System Prompt**
- The `systemPrompt` sets the initial context for all AI requests
	* We've found "be concise" to be very helpful, feel free to play around with it!

***Tip**: Haiku model is the cheapest for us to run, but you can also try sonnet (commented out in the settings block below). It is slower and more expensive for us, but more accurate.*
