## Prompt instructions

The user request is defined in "-- Request --" header below
- User request pattern uses XML-style tags:
  - <prompt>: Contains the user prompt
  - <selection>: Target text to be modified, can be empty if content needs to be generated and note replaced
  - the rest of the content is untagged
  - 

Example:

```md
<prompt>Remove redundancy from this text</prompt>
Context: The following sentence contains repetition.
<selection>The redundant wording keeps repeating the same words redundantly in a redundant way.</selection>
Next sentence follows here.
```

## -- Request --
<prompt>
{{prompt}}
</prompt>
{{before}}<selection>{{selection}}</selection>{{after}}



## Processing Instructions:
1. Focus modifications only on text within <selection> tags
2. Return ONLY your response formatted as html-like tags:
  - <reasoning> tag reasoning steps for the prompt
  - <selection_replacement> node that will replace <selection> content in the user prompt
  - <explanation> tag to provide explanation for the suggested replacement
3. Consider the full context when making modifications
4. Untagged response text is going to be ignored, and only tagged will be shown to the user

