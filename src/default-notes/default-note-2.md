# Markdown Features

## Lists

***FYI**:  "text cursor" refers to the position your keyboard is focussd at. For some tasks, we've provided  helpful indicators using `«` or `»` characters that indicate where you should place the text cursor (either by clicking or using the arrow keys).*

Shelv supports the Markdown format for lists. 

- In Markdown, unordered lists can use `-`, `*`, or `+`
	* You can even use a different bullet for a sub-list
		+ But keep them consistent within the same level

Even though everything in Shelv is plain-text, you'll find all the handy shortcuts and features you're used to when working with lists.

- [ ] Add new rows to this list by pressing `enter` at the end »

- [ ] Split lists in the middle with`enter`» « You should now have two items!

- [ ] Pressing `enter` on an empty list item will delete it. It's a nifty way to finish your list!
	* Try pressing `enter` twice here »

- [ ] Press `tab` anywhere on this item to indent right
	* [ ]  and `shift + tab`to indent left

1. Numbered lists are also supported. Try adding another item with `enter` »
2. Now, indent your new row to the right (with `tab`). Notice the counter resets.
3. [ ] Done (numbered lists can also be checklists!)

In Markdown, it's important to note that checkboxes only work in lists. Shelv also provides a nifty shortcut for creating them:

- Add a checkbox to the start of a list item with `[]`
	*  « type `[] done!` here 

Or start a new checklist with `[]`
« type `[] done!` here


## Formatting

In Markdown, ***emphasize*** words or phrases with **bold** or *italic* by wrapping the text with:
- single-asterisks `*` (for *italic*) 
- double-asterisks `**` (for **bold**) 
- triple-asterisks `***` (for ***bold + italic***)
- double-tildes `~~` (for ~~strikethrough~~)

Shelv also supports some nifty features using the shortcuts `⌘ + b` for bold, and `⌘ + i` for italic.

Highlight the text »here«
- [ ] Press `⌘ + i` to italicize and then `⌘ + b` to also add bold

To switch from bold to italic, **»Place text cursor anywhere here«**
- [ ] Press `⌘ + b` to unbold, which auto-selects the text, and then `⌘ + i` to italicize.

## Links

- Shelv will automatically link URLs: https://shelv.app
	* [ ] Hover the link to see its clickable!

- Markdown also supports links like [this](https://shelv.app)
	* [ ] Hover over `this` and see it's clickable!

## Headers

Shelv supports 3 variations of markdown headers. Use a variable number of `#` characters at the start of a line to format the header, like this:

# Header 1
## Header 2
### Header 3

»Place text cursor anywhere on this line«
- [ ] Use the shortcut `⌘ + ⌥ + 1` to switch to header 1 (similar shortcut for header 2 and 3)
- [ ] Undo the header by pressing `⌘ + ⌥ + 1` again