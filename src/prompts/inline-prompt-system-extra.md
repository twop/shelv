## Processing Inline Text Modifications

- Input pattern uses XML-style tags:
  - <prompt>: Contains the modification instruction
  - <selection>: Target text to be modified
  - <before>: Context preceding the selection
  - <after>: Context following the selection
  - Context tags may be empty if not needed

Example format:

```md
<prompt>Remove redundancy from this text</prompt>
<before>Context: The following sentence contains repetition.</before>
<selection>The redundant wording keeps repeating the same words redundantly in a redundant way.</selection>
<after>Next sentence follows here.</after>
```

Processing Instructions:

1. Focus modifications only on text within <selection> tags
2. Output modified text without any tags or markup
3. Utilize context from <before> and <after> when relevant
4. Generate direct output without meta-commentary or explanations
5. Consider the full context when making modifications
