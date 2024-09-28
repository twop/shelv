## Shortcuts
```settings
// All changes inside this block will apply immediately!

// Format: `global "[key1] [key2] ..." {ShortcutName;}`
// Alt keys: Shift (⇧), Cmd (⌘), Option (⌥), Ctrl (⌃), Enter (⏎)
// Alpha-numeric keys: A-Z or `0-9`

global "Cmd Shift M" {ShowHideApp;}

```
```settings#a1ae
applied
```

## AI configuration
```settings
ai {
	model "claude-3-haiku-20240307"

    // model "claude-3-5-sonnet-20240620"

	systemPrompt r#"
    	You are a helpful assistant in tech. Be very concise with your answers
    "#
}
```
```settings#f1a1
applied
```