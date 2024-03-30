option #1:
note1.mdx

```ts
importShelv("gist.githumb.com/some-plugin...");
```

```#some-plugin

all
the
code
from
the
gist

```

Shelv - TODOs

````ts
{

    settings: {
        fontSize: 12
    },
    commands: [
        {
            type: "editor:command",
            name: "create js block",
            shortcut: "cmd + option + r",
            action: () => {
                Shelv.insert(Shelv.currentNote(), Shelv.cursor(),
                "\n```js\n{||}```\n");
            }
        }
    ]
}


````

````ts
Shelv.globalSettings.set("fontSize", 12);

// register a command based on keyboard shortcuts
Shelv.register({
  type: "editor:command",
  name: "create js block",
  shortcut: "cmd + option + r",
  action: () => {
    Shelv.insert(Shelv.currentNote(), Shelv.cursor(), "\n```js\n{||}```\n");
  },
});
````

```#85b7;
"'create js block' was registered"
```

```#85b7
ReferenceError: console is not defined
```

```js
export function createJsBlock() {}
```
