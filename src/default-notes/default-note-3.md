# Code-blocks

## Markdown Format

In Markdown, you can format a block of text as "code" as follows:

```
hjhjhj
```

You can also specify the language of the code like this (and some languages suport syntax highlighting!):

```ts
"hello" + "world";
```

- [ ] **Task**: Highight the function below and use the hotkey `cmd + option + B` to create a codeblock.

function main() {
return "hello" + "world"
}

## Codeblocks that compute

In Shelv, `js` codeblocks have special functionality - they compute their output right in your note! Any codeblocks that start with ```js will automatically execute whenever the content changes.

- [ ] **Task**: Append js to the block below (to make ```js), the code should execute!
- [ ] **Task**: Modify the formula inside the block, and the code output should automatically update.

```
1 + 2
```

## AI codeblocks

- Shelv also supports ```ai codeblocks that execute by pressing `cmd + enter`

  - _Note: AI codeblocks won't automatically execute when the content changes._

- [ ] **Task**: Place your keyboard cursor in the block below and press `cmd + enter`

```llm
Give me a concise summary of the above.
```

- [ ] **Task**: Try your own AI prompt. Examples: Check spelling and grammer, Re-write without markdown, Write a Javascript function that adds two numbers
