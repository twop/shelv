# Welcome to **settings**

Yep, it is just a note. We will publish a comprehensive list of settings later.

Meanwhile, feel free to ask questions on discord here {TODO: link}.
Or just give us feedback, we would **love** to hear from you.


```settings
// UNCOMMENT and tweak accordingly
// global "Cmd Option S" {ShowHideApp;}

llm {
  // this model is the cheapest for us to run
  // you can also try "claude-3-5-sonnet-20240620"
  // it is slower and more expensive for us, but more accurate
	model "claude-3-haiku-20240307"

	// optional, we found "be concise" very helpful, feel free to play with the system prompt
	systemPrompt r#"
    	You are a helpful assistant in tech. Be Very concise with your answers
    "#
}
```
