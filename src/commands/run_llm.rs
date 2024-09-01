use smallvec::SmallVec;

use crate::{
    app_actions::{AppAction, Conversation, ConversationPart, LLMRequest},
    byte_span::ByteSpan,
    command::{
        try_extract_text_command_context, CommandContext, EditorCommandOutput, TextCommandContext,
    },
    effects::text_change_effect::TextChange,
    scripting::{CodeBlockKind, SourceHash},
    text_structure::{SpanDesc, SpanIndex, SpanKind, SpanMeta},
};

pub fn run_llm_block(CommandContext { app_state }: CommandContext) -> Option<EditorCommandOutput> {
    const LLM_LANG: &str = "llm";
    let text_command_context = try_extract_text_command_context(app_state)?;

    let TextCommandContext {
        text_structure,
        text,
        byte_cursor: cursor,
    } = text_command_context;

    // Check if we are in an LLM code block
    let llm_blocks: SmallVec<[(SpanIndex, &SpanDesc, CodeBlockKind); 6]> = text_structure
        .filter_map_codeblocks(|lang| match lang {
            LLM_LANG => Some(CodeBlockKind::Source),

            output if output.starts_with(LLM_LANG) => {
                let hex_str = &output[LLM_LANG.len()..];
                Some(CodeBlockKind::Output(SourceHash::parse(hex_str)))
            }

            _ => None,
        })
        .collect();

    // find source llm code block
    let (index_in_array, (block_span_index, block_desc, _)) =
        llm_blocks
            .iter()
            .enumerate()
            .find(|(_pos_index, (_, desc, kind))| {
                desc.byte_pos.contains(cursor) && *kind == CodeBlockKind::Source
            })?;

    let (_, question_desc) = text_structure
        .iterate_immediate_children_of(*block_span_index)
        .find(|(_, desc)| desc.kind == SpanKind::Text)?;

    let question_body = &text[question_desc.byte_pos.range()];
    let source_hash = SourceHash::from(question_body);

    // Create a new code block for llm output
    let address = format!("llm#{}", source_hash.to_string());

    // check the next block
    let (replacemen_pos, output_block) = match llm_blocks.get(index_in_array + 1) {
        // if it is llm output just reuse that one
        Some((_index, desc, CodeBlockKind::Output(_))) => {
            (desc.byte_pos, format!("```{address}\n```"))
        }

        // if not then add a new code block right after, note extra new lines
        _ => (
            ByteSpan::point(block_desc.byte_pos.end),
            format!("\n```{address}\n```\n"),
        ),
    };

    let text_change = TextChange::Replace(replacemen_pos, output_block);

    let target = app_state.selected_note;

    let mut conversation = Conversation { parts: Vec::new() };

    let mut prev_block_end = 0;
    for (block_index, desc, kind) in llm_blocks.iter().take(index_in_array + 1) {
        if prev_block_end < desc.byte_pos.start {
            let markdown = text[prev_block_end..desc.byte_pos.start].trim();
            if !markdown.is_empty() {
                conversation
                    .parts
                    .push(ConversationPart::Markdown(markdown.to_string()));
            }
        }

        let inner_content_range = text_structure.get_span_inner_content(*block_index);
        let inner_content = text[inner_content_range.range()].trim();

        match kind {
            CodeBlockKind::Source => {
                conversation
                    .parts
                    .push(ConversationPart::Question(inner_content.to_string()));
            }
            CodeBlockKind::Output(_) => {
                conversation
                    .parts
                    .push(ConversationPart::Answer(inner_content.to_string()));
            }
        }

        prev_block_end = desc.byte_pos.end;
    }

    let (model, system_prompt) = app_state
        .llm_settings
        .as_ref()
        .map(|s| (s.model.clone(), s.system_prompt.clone()))
        .unwrap_or_else(|| ("claude-3-haiku-20240307".to_string(), None));

    let llm_request = LLMRequest {
        model,
        system_prompt,
        conversation,
        output_code_block_address: address,
        note_id: target,
    };

    println!("----\n{llm_request:#?}");

    let mut res = SmallVec::new();

    res.push(AppAction::ApplyTextChanges {
        target,
        changes: vec![text_change],
        should_trigger_eval: false,
    });
    res.push(AppAction::AskLLM(llm_request));

    return Some(res);
}
