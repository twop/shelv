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
        .take_while(|(_, desc, _)| desc.byte_pos.start < cursor.end)
        .collect();

    let (block_span_index, block_desc, _) = llm_blocks
        .iter()
        .rev()
        .find(|(_, desc, kind)| desc.byte_pos.contains(cursor) && *kind == CodeBlockKind::Source)?;

    let (_, question_desc) = text_structure
        .iterate_immediate_children_of(*block_span_index)
        .find(|(_, desc)| desc.kind == SpanKind::Text)?;

    let question_body = &text[question_desc.byte_pos.range()];
    let source_hash = SourceHash::from(question_body);

    // Create a new code block for llm output
    let address = format!("llm#{}", source_hash.to_string());
    let output_block = format!("\n```{address}\n```\n");

    let text_change = TextChange::Replace(ByteSpan::point(block_desc.byte_pos.end), output_block);

    let target = app_state.selected_note;

    let mut conversation = Conversation { parts: Vec::new() };

    let mut prev_block_end = 0;
    for (_, desc, kind) in llm_blocks.iter() {
        if prev_block_end < desc.byte_pos.start {
            let markdown = text[prev_block_end..desc.byte_pos.start].trim();
            if !markdown.is_empty() {
                conversation
                    .parts
                    .push(ConversationPart::Markdown(markdown.to_string()));
            }
        }

        match kind {
            CodeBlockKind::Source => {
                let question = text[desc.byte_pos.range()].trim();
                conversation
                    .parts
                    .push(ConversationPart::Question(question.to_string()));
            }
            CodeBlockKind::Output(_) => {
                let answer = text[desc.byte_pos.range()].trim();
                conversation
                    .parts
                    .push(ConversationPart::Answer(answer.to_string()));
            }
        }

        prev_block_end = desc.byte_pos.end;
    }

    let llm_request = LLMRequest {
        conversation,
        output_code_block_address: address,
        note_id: target,
    };

    println!("{llm_request:#?}");

    let mut res = SmallVec::new();

    res.push(AppAction::ApplyTextChanges {
        target,
        changes: vec![text_change],
        should_trigger_eval: false,
    });
    res.push(AppAction::AskLLM(llm_request));

    return Some(res);
}
