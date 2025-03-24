## Prompt instructions

- User prompt pattern uses XML-style tags:
  - <prompt>: Contains the user prompt
  - <selection>: Target text to be modified, can be empty if content needs to be generated and note replaced
  - <before>: Text preceding the selection
  - <after>: Text following the selection

Example:

```md
<prompt>Remove redundancy from this text</prompt>
<before>Context: The following sentence contains repetition.</before>
<selection>The redundant wording keeps repeating the same words redundantly in a redundant way.</selection>
<after>Next sentence follows here.</after>
```

Processing Instructions:

1. Focus modifications only on text within <selection> tags
2. Return ONLY
  - <reasoning> tag reasoning steps for the prompt
  - <selection_replacement> node that will replace <selection> content in the user prompt
  - <explanation> tag to provide explanation for the suggested replacement
3. Utilize context from <before> and <after> when relevant
4. Consider the full context when making modifications
5. When making code suggestions ignore corresponding output code blocks like `js#` or `kdl#`, those will be added by Shelv after evaluation.


## Prompt
<prompt>
{{prompt}}
</prompt>
<before>
{{before}}
</before>
<selection>
{{selection}}
</selection>
<after>
{{after}}
</after>
