# Code-blocks

- [ ] **Create a codeblock** by highlighting the text below and pressing `⌘ + ⌥ + b`

.--------------------------.
| ____  _          _       |
|/ ___|| |__   ___| |_   __|
|\___ \| '_ \ / _ \ \ \ / /|
| ___) | | | |  __/ |\ V / |
||____/|_| |_|\___|_| \_/  |
'--------------------------'


## AI codeblocks

- [ ] **Execute the AI prompt** by pressing `⌘ + ⏎` while inside the codeblock.

```ai
What is the meaning of life? Answer ONLY as a number. 
```
```ai#b7e6
42
```

- [ ] Add `Spell it out` to the prompt, and re-run by pressing `⌘ + ⏎`.
	* The output block will refresh in place!


## Codeblocks that compute

In Shelv, `js` codeblocks have special functionality - they compute their output right in your note! Any codeblocks that start with ```js will automatically execute whenever the content changes.

- [ ] **Execute the code below** by adding `js` after  the ```

```
// ^ add "js" above to make ```js

const hi = (name) => "hello " + name + "!"
hi("universe")

```

## Blocks are interconnected

AI blocks will have the rest of the note above it as context, enabling use cases like:
- `Check spelling and grammer`
- `Re-write without markdown`
- `Give a summary`

***Tip**: AI blocks in a note essentially form a conversation, and content between each block is additional context (just like a traditional chat!)*

```ai
Give me a concise summary of the above.
```

- [ ] **Try running the above AI block**

JS Code blocks also connected! We can use the `hi` function we wrote earlier in this note in a new code block. Try it out below!

- [ ] Write some JavaScript that uses our previously defined function `hi`, like this:
	*  `  hi("<your name here>")  ` 

```js

```

You should now see your name printed as an output block. 


# Almost done! Give us your feedback in [note 4](shelv://note4)