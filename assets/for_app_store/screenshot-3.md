```js
const monthNames = ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec'];

export function getCurrentDate() {
	const now = new Date();
	const year = now.getFullYear();
	const month = monthNames[now.getMonth()];
	// Ensures the day is 2 digits, adding leading zero if needed(2, '0');
	const day = String(now.getDate()).padStart(2, '0'); 
	return `${year}/${month}/${day}`;
}
```

```kdl
// (⌘ Y): Insert result from: getCurrentDate
bind "Cmd Y" icon="\u{E4E5}" alias="date" description="Insert current date (YYYY/mon/DD)" { 
	InsertText {
		callFunc "getCurrentDate"
	} 
}
```

---

part 1:
prompt:
add a command for inserting the current day

part 2:
show the diff

part 3:
isert date
show the command palette with the shortcut

