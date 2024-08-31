use smallvec::SmallVec;

use crate::{
    app_actions::{AppAction, LLMRequest},
    byte_span::ByteSpan,
    command::{
        try_extract_text_command_context, CommandContext, EditorCommandOutput, TextCommandContext,
    },
    effects::text_change_effect::TextChange,
    scripting::SourceHash,
    text_structure::{SpanKind, SpanMeta},
};

pub fn run_llm_block(CommandContext { app_state }: CommandContext) -> Option<EditorCommandOutput> {
    let text_command_context = try_extract_text_command_context(app_state)?;

    let TextCommandContext {
        text_structure,
        text,
        byte_cursor: cursor,
    } = text_command_context;

    // Check if we are in an LLM code block
    let (block_span_index, block_desc, meta) =
        text_structure.find_surrounding_span_with_meta(SpanKind::CodeBlock, cursor.clone())?;

    let SpanMeta::CodeBlock { lang } = meta else {
        return None;
    };

    if lang != "llm" {
        return None;
    }

    let (_, question_desc) = text_structure
        .iterate_immediate_children_of(block_span_index)
        .find(|(_, desc)| desc.kind == SpanKind::Text)?;

    let question_body = &text[question_desc.byte_pos.range()];
    let source_hash = SourceHash::from(question_body);

    // Create a new code block for llm output
    let address = format!("llm#{}", source_hash.to_string());
    let output_block = format!("\n```{address}\n```\n");

    let text_change = TextChange::Replace(ByteSpan::point(block_desc.byte_pos.end), output_block);

    let target = app_state.selected_note;

    let llm_request = LLMRequest {
        context: text[..block_desc.byte_pos.start].to_string(),
        question: question_body.to_string(),
        output_code_block_address: address,
        note_id: target,
    };

    let mut res = SmallVec::new();

    res.push(AppAction::ApplyTextChanges {
        target,
        changes: vec![text_change],
        should_trigger_eval: false,
    });
    res.push(AppAction::AskLLM(llm_request));

    return Some(res);
}
