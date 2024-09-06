# Code-blocks

- [ ] **Create a codeblock** by highlighting the text below and pressing `cmd + option + B`

  /\ 
 /__\


## AI codeblocks

- [ ] **Execute the AI prompt** by pressing `cmd + enter` while inside the codeblock.

```llm
What is the meaning of life? Answer ONLY as a single number
```

- [ ] Change `number` to `word` for the promp, and re-run by pressing `cmd + enter`, and the output block will refresh!


## Codeblocks that compute

In Shelv, `js` codeblocks have special functionality - they compute their output right in your note! Any codeblocks that start with ```js will automatically execute whenever the content changes.

- [ ] **Execute the code below** by specifying `js` (to make ```js) as the language.

```
const hi(name) = () => "hello " + name + "!"
hi("universe")
```

## Blocks are interconnected

AI blocks will have the rest of the note above it as context, enabling use cases like:
- `Check spelling and grammer`
- `Re-write without markdown`
- `Give a summary`

Tip: AI blocks in a note essentially form a conversations, and content between each block is additional context (just like a traditional chat!)

```ai
Give me a concise summary of the above.
```

- [ ] **Try running the above AI block**

Code blocks can reference the variables and functions defined in earlier blocks!

- [ ] Reference our `hi` function from before `hi("awesome user of Shelv")` inside the block below

```js

```
