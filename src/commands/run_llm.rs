use std::ops::Deref;

use smallvec::SmallVec;

use crate::{
    app_actions::{AppAction, Conversation, ConversationPart, LLMBlockRequest},
    app_state::AppState,
    byte_span::ByteSpan,
    command::{
        try_extract_text_command_context, CommandContext, EditorCommandOutput, TextCommandContext,
    },
    effects::text_change_effect::TextChange,
    persistent_state::NoteFile,
    scripting::note_eval_context::SourceHash,
    text_structure::{CodeBlockMeta, SpanIndex, SpanKind, SpanMeta, TextStructure},
};

#[derive(Clone, Copy)]
pub enum CodeBlockAddress {
    NoteSelection,
    TargetBlock(NoteFile, SpanIndex),
}

pub(crate) const LLM_LANG: &str = "ai";

pub fn prepare_to_run_llm_block(
    app_state: &AppState,
    address: CodeBlockAddress,
) -> Option<EditorCommandOutput> {
    const LLM_LANG_OLD: &str = "llm";

    enum OwnedOrRef<'r> {
        Ref(&'r TextStructure),
        Owned(TextStructure),
    }

    impl<'r> Deref for OwnedOrRef<'r> {
        type Target = TextStructure;

        fn deref(&self) -> &Self::Target {
            match self {
                OwnedOrRef::Ref(ts) => ts,
                OwnedOrRef::Owned(ts) => ts,
            }
        }
    }

    let (text_structure, cursor, text) = match address {
        CodeBlockAddress::NoteSelection => {
            let text_command_context = try_extract_text_command_context(app_state)?;

            let TextCommandContext {
                text_structure,
                text,
                byte_cursor: cursor,
            } = text_command_context;

            (OwnedOrRef::Ref(text_structure), cursor, text)
        }
        CodeBlockAddress::TargetBlock(note_file, span_index) => {
            let note = app_state.notes.get(&note_file).unwrap();

            let text_structure = TextStructure::new(&note.text);

            let cursor = match text_structure.get_span_with_meta(span_index) {
                Some((desc, SpanMeta::CodeBlock(CodeBlockMeta { lang, .. })))
                    if lang == LLM_LANG =>
                {
                    Some(desc.byte_pos)
                }
                _ => None,
            }?;

            (
                OwnedOrRef::Owned(text_structure),
                cursor,
                note.text.as_str(),
            )
        }
    };

    // Check if we are in an LLM code block
    let llm_blocks: SmallVec<[_; 6]> = text_structure
        .filter_map_codeblocks(|lang| match lang {
            LLM_LANG | LLM_LANG_OLD => Some((true, None)), // Source block

            output if output.starts_with(LLM_LANG) => {
                let hex_str = &output[LLM_LANG.len()..];
                Some((false, SourceHash::parse(hex_str))) // Output block with hash
            }

            _ => None,
        })
        .collect();

    // find source llm code block
    let (index_in_array, (block_span_index, block_desc, _, block_type)) = llm_blocks
        .iter()
        .enumerate()
        .find(|(_pos_index, (_, desc, _, (is_source, _)))| {
            desc.byte_pos.contains(cursor) && *is_source
        })?;

    let (_, question_desc) = text_structure
        .iterate_immediate_children_of(*block_span_index)
        .find(|(_, desc)| desc.kind == SpanKind::Text)?;

    let question_body = &text[question_desc.byte_pos.range()];
    let source_hash = SourceHash::from(question_body);

    // Create a new code block for llm output
    let address = format!("{LLM_LANG}#{}", source_hash.to_string());

    // check the next block
    let (replacemen_pos, output_block) = match llm_blocks.get(index_in_array + 1) {
        // if it is llm output just reuse that one
        Some((_index, desc, _, (is_source, _))) if !is_source => {
            (desc.byte_pos, format!("```{address}\n```"))
        }

        // if not then add a new code block right after, note extra new lines
        _ => (
            ByteSpan::point(block_desc.byte_pos.end),
            format!("\n```{address}\n```\n"),
        ),
    };

    let text_change = TextChange::Insert(replacemen_pos, output_block);

    let target = app_state.selected_note;

    let mut conversation = Conversation { parts: Vec::new() };

    let mut prev_block_end = 0;
    for (block_index, desc, _, kind) in llm_blocks.iter().take(index_in_array + 1) {
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
            (true, _) => {
                // Source block
                conversation
                    .parts
                    .push(ConversationPart::Question(inner_content.to_string()));
            }
            (false, _) => {
                // Output block
                conversation
                    .parts
                    .push(ConversationPart::Answer(inner_content.to_string()));
            }
        }

        prev_block_end = desc.byte_pos.end;
    }

    let llm_request = LLMBlockRequest {
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
