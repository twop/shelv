use smallvec::SmallVec;

use crate::byte_span::{ByteSpan, RangeRelation, UnOrderedByteSpan};

#[derive(Debug)]
pub enum TextChange {
    // Delete(ByteRange),
    Insert(ByteSpan, String),
    // Insert { insertion: String, byte_pos: usize },
}

impl TextChange {
    pub const CURSOR_EDGE: &'static str = "{|}";
    pub const CURSOR: &'static str = "{||}";

    pub fn try_extract_cursor(mut text: String) -> (String, Option<ByteSpan>) {
        // let mut text = text.to_string();
        if let Some(start) = text.find(TextChange::CURSOR) {
            text.replace_range(start..(start + TextChange::CURSOR.len()), "");
            (text, Some(ByteSpan::new(start, start)))
        } else {
            let Some(start) = text.find(TextChange::CURSOR_EDGE) else {
                return (text, None);
            };
            text.replace_range(start..(start + TextChange::CURSOR_EDGE.len()), "");
            let Some(end) = text.find(TextChange::CURSOR_EDGE) else {
                // undo the first removal
                text.insert_str(start, Self::CURSOR_EDGE);
                return (text, None);
            };
            text.replace_range(end..(end + TextChange::CURSOR_EDGE.len()), "");
            (text, Some(ByteSpan::new(start, end)))
        }
    }

    pub fn encode_cursor(text: &str, cursor: UnOrderedByteSpan) -> String {
        let mut text = text.to_string();
        let cursor = cursor.ordered();
        if cursor.is_empty() {
            text.insert_str(cursor.start, TextChange::CURSOR);
        } else {
            text.insert_str(cursor.start, TextChange::CURSOR_EDGE);
            text.insert_str(
                cursor.end + TextChange::CURSOR_EDGE.len(),
                TextChange::CURSOR_EDGE,
            );
        }
        text
    }
}

// ----  text change handler ----
#[derive(Debug)]
pub enum TextChangeError {
    OverlappingChanges,
}

pub fn apply_text_changes(
    text: &mut String,
    prev_cursor: Option<UnOrderedByteSpan>,
    changes: impl IntoIterator<Item = TextChange>,
) -> Result<Option<UnOrderedByteSpan>, TextChangeError> {
    #[derive(Debug, Clone)]

    struct Log {
        removed: ByteSpan,
        inserted_len: usize,
    }
    type Logs = SmallVec<[Log; 4]>;

    fn append(
        range: ByteSpan,
        to_insert: usize,
        logs: &[Log],
    ) -> Result<(Logs, ByteSpan), TextChangeError> {
        let mut res: Logs = logs.iter().map(Log::clone).collect();
        res.sort_by(|a, b| a.removed.end.cmp(&b.removed.end));

        let mut actual_range = range;

        let mut split_point: Option<usize> = None;

        // find a splitting point in the insertion logs
        for (i, log) in logs.iter().enumerate() {
            let log_entry_range = log.removed;
            match log_entry_range.relative_to(actual_range) {
                // check for overlaps
                RangeRelation::StartInside
                | RangeRelation::EndInside
                | RangeRelation::Inside
                | RangeRelation::Equal
                | RangeRelation::Contains => {
                    // it means that we have overlapping ranges for removal
                    // that is not allowed
                    return Err(TextChangeError::OverlappingChanges);
                }

                RangeRelation::Before => {
                    // that means that the removal happened earlier
                    // thus, we need to adjust starting position
                    let delta = log.inserted_len as isize - log.removed.range().len() as isize;
                    actual_range = ByteSpan::new(
                        (actual_range.start as isize + delta) as usize,
                        (actual_range.end as isize + delta) as usize,
                    );
                }

                RangeRelation::After => {
                    split_point = Some(i);
                }
            }
        }

        // if we need to insert somewhere in the middle we need to shift spans that come after
        if let Some(split_point) = split_point {
            // we need to move what comes after the split
            let delta: isize = to_insert as isize - actual_range.range().len() as isize;
            for log in res[split_point..].iter_mut() {
                log.removed = ByteSpan::new(
                    (log.removed.start as isize + delta) as usize,
                    (log.removed.end as isize + delta) as usize,
                );
            }
        }

        // finally insert the element at a proper position
        res.insert(
            split_point.unwrap_or(res.len()),
            Log {
                removed: actual_range.clone(),
                inserted_len: to_insert,
            },
        );

        Ok((res, actual_range))
    }

    let mut logs: Logs = Logs::new();

    let mut actual_changes: SmallVec<[TextChange; 4]> = SmallVec::new();

    let mut inserted_cursor: Option<ByteSpan> = None;

    for change in changes.into_iter() {
        match change {
            TextChange::Insert(range, with) => {
                let (with, extracted_cursor) = TextChange::try_extract_cursor(with);
                let to_insert = with.len();
                let (new_logs, target) = append(range, to_insert, &logs)?;
                logs = new_logs;
                if let Some(extracted_cursor) = extracted_cursor {
                    inserted_cursor = Some(ByteSpan::new(
                        target.start + extracted_cursor.start,
                        target.start + extracted_cursor.end,
                    ));
                }
                actual_changes.push(TextChange::Insert(target, with));
            }
        }
    }

    let adjusted_cursor = prev_cursor.map(|prev_cursor| match inserted_cursor {
        Some(cursor) => UnOrderedByteSpan::new(cursor.start, cursor.end),
        None => {
            // let mut cursor_start = prev_cursor.start;
            // let mut cursor_end = prev_cursor.end;
            let ordered = ByteSpan::new(prev_cursor.start, prev_cursor.end);
            let (cursor_start, cursor_end) = actual_changes.iter().fold(
                (ordered.start, ordered.end),
                |(cursor_start, cursor_end), change| match change {
                    TextChange::Insert(change_range, with) => {
                        let byte_delta: isize =
                            with.len() as isize - change_range.range().len() as isize;

                        match ByteSpan::new(cursor_start, cursor_end).relative_to(*change_range) {
                            RangeRelation::Before => {
                                // nothing to do here
                                (cursor_start, cursor_end)
                            }
                            RangeRelation::After => {
                                // cursor is ahead of range => move the cursor by change delta
                                (
                                    (cursor_start as isize + byte_delta) as usize,
                                    (cursor_end as isize + byte_delta) as usize,
                                )
                            }
                            RangeRelation::StartInside => {
                                // means that the left side of selection is inside the replacement
                                // example
                                // `ab{|}cd{|}e`
                                //   ^___^ => replace with "oops"
                                // `a{|}oopsd{|}e`
                                // => selecte the entire replacement, and continue to the prev end
                                (
                                    change_range.start,
                                    (prev_cursor.end as isize + byte_delta) as usize,
                                )
                            }
                            RangeRelation::EndInside => {
                                // means that the right side of selection is inside the replacement
                                // example
                                // `ab{|}cd{|}efj`
                                //        ^____^ => replace with "oops"
                                // `ab{|}coops{|}j`
                                // => selecte the entire replacement, and continue to the prev start
                                (cursor_start, change_range.start + with.len())
                            }
                            RangeRelation::Inside => {
                                // means that the cursor is inside the replacement range
                                // example
                                // `ab{||}cd`
                                //   ^____^ => replace with "oops"
                                // `a{|}oops{|}d`
                                // => selecte the entire replacement
                                (change_range.start, change_range.start + with.len())
                            }
                            RangeRelation::Contains => {
                                // means that the selection is larger than replacement
                                // example
                                // `a{|}bcde{|}f`
                                //       ^ ^ => replace with "oops"
                                // `a{|}boops{|}f`
                                // => selecte the entire replacement
                                (cursor_start, (cursor_end as isize + byte_delta) as usize)
                            }

                            RangeRelation::Equal => match cursor_start == cursor_end {
                                // means empty span, eg "{||}" is being replaced with "some text"
                                // so we CHOOSE to replace it with "some text{||}", e.g. replacement is before the cursor
                                // same as  RangeRelation::After
                                true => (
                                    (cursor_start as isize + byte_delta) as usize,
                                    (cursor_end as isize + byte_delta) as usize,
                                ),
                                // now if we are replacing the entire region of the selection we essentially just do the same as
                                // RangeRelation::Contains
                                false => {
                                    (cursor_start, (cursor_end as isize + byte_delta) as usize)
                                }
                            },
                        }
                    }
                },
            );

            // flip the direction if it was flipped before
            // note that we get ordered results due to algorithm using ByteSpan which assumed order
            let (cursor_start, cursor_end) = if prev_cursor.start > prev_cursor.end {
                (cursor_end, cursor_start)
            } else {
                (cursor_start, cursor_end)
            };

            UnOrderedByteSpan::new(cursor_start, cursor_end)
        }
    });

    // finally apply all the precomputed changes
    for change in actual_changes.into_iter() {
        match change {
            TextChange::Insert(byte_span, with) => {
                text.replace_range(byte_span.range(), &with);
            }
        }
    }

    Ok(adjusted_cursor)
}

#[cfg(test)]
mod tests {
    // --------- Text changes cursor tests --------
    use super::*;

    #[test]
    pub fn test_cursor_extraction_from_string() {
        let (text, cursor) = TextChange::try_extract_cursor("- a{||}b".to_string());
        assert_eq!(text, "- ab");
        assert_eq!(cursor, Some(ByteSpan::new(3, 3)));

        let (text, cursor) = TextChange::try_extract_cursor("- {|}a{|}b".to_string());
        assert_eq!(text, "- ab");
        assert_eq!(cursor, Some(ByteSpan::new(2, 3)));

        let (text, cursor) = TextChange::try_extract_cursor("- a{|}b".to_string());
        assert_eq!(text, "- a{|}b");
        assert_eq!(cursor, None);
    }

    // --------- Apply changes tests --------
    #[test]
    pub fn test_several_text_changes_in_order() {
        let mut text = "a b".to_string();

        let a_pos = text.find("a").unwrap();
        let b_pos = text.find("b").unwrap();

        let changes = [
            TextChange::Insert(ByteSpan::new(a_pos, a_pos + 1), "hello".into()),
            TextChange::Insert(ByteSpan::new(b_pos, b_pos + 1), "world".into()),
            TextChange::Insert(ByteSpan::new(b_pos + 1, b_pos + 1), "!".into()),
        ];

        apply_text_changes(&mut text, Some(UnOrderedByteSpan::new(0, 0)), changes).unwrap();
        assert_eq!(text, "hello world!");
    }

    #[test]
    pub fn test_several_text_changes_out_of_order() {
        let mut text = "a b".to_string();

        let a_pos = text.find("a").unwrap();
        let b_pos = text.find("b").unwrap();

        let changes = [
            TextChange::Insert(ByteSpan::new(b_pos + 1, b_pos + 1), "!".into()),
            TextChange::Insert(ByteSpan::new(b_pos, b_pos + 1), "world".into()),
            TextChange::Insert(ByteSpan::new(a_pos, a_pos + 1), "hello".into()),
        ];

        apply_text_changes(&mut text, Some(UnOrderedByteSpan::new(0, 0)), changes).unwrap();
        assert_eq!(text, "hello world!");
    }

    #[test]
    pub fn test_overlapping_text_changes_are_not_allowed() {
        let mut text = "a b".to_string();

        let a_pos = text.find("a").unwrap();
        let b_pos = text.find("b").unwrap();

        let changes = [
            // captures "a b"
            TextChange::Insert(ByteSpan::new(a_pos, b_pos + 1), "hello".into()),
            // captures "b"
            TextChange::Insert(ByteSpan::new(b_pos, b_pos + 1), "world".into()),
        ];

        let cursor = apply_text_changes(&mut text, Some(UnOrderedByteSpan::new(0, 0)), changes);
        assert!(matches!(cursor, Err(TextChangeError::OverlappingChanges)));
        assert_eq!(text, "a b");
    }

    // --- automatic cursor adjacements based on text changes ---

    #[test]
    pub fn test_cursor_adjacement_cursor_inside_replacement() {
        // `ab{||}cd`
        //   ^____^ => replace with "oops"
        // `a{|}oops{|}d`
        let (mut text, cursor) = TextChange::try_extract_cursor("ab{||}cd".to_string());

        let start = text.find("b").unwrap();
        let end = text.find("d").unwrap();

        let changes = [
            TextChange::Insert(ByteSpan::new(start, end), "oops".into()),
            // delete "a", to test out cursor adjecement that are out of range
            TextChange::Insert(ByteSpan::new(0, 1), "".into()),
        ];

        let cursor =
            apply_text_changes(&mut text, Some(cursor.unwrap().unordered()), changes).unwrap();
        assert_eq!(
            TextChange::encode_cursor(&text, cursor.unwrap()),
            "{|}oops{|}d"
        );
    }

    #[test]
    pub fn test_cursor_adjacement_selection_contains_replacement() {
        // means that the selection is larger than replacement
        // example
        // `a{|}bcde{|}f`
        //       ^ ^ => replace with "oops"
        // `a{|}boops{|}f`
        // => selecte the entire replacement
        let (mut text, cursor) = TextChange::try_extract_cursor("a{|}bcde{|}f".to_string());

        let changes = [
            TextChange::Insert(
                ByteSpan::new(text.find("c").unwrap(), text.find("f").unwrap()),
                "oops".into(),
            ),
            // delete "a", to test out cursor adjecement that are out of range
            TextChange::Insert(ByteSpan::new(0, 1), "".into()),
        ];

        let cursor =
            apply_text_changes(&mut text, Some(cursor.unwrap().unordered()), changes).unwrap();
        assert_eq!(
            TextChange::encode_cursor(&text, cursor.unwrap()),
            "{|}boops{|}f"
        );
    }

    #[test]
    pub fn test_cursor_adjacement_selection_start_inside_replacement() {
        // means that the left side of selection is inside the replacement
        // example
        // `ab{|}cd{|}e`
        //   ^___^ => replace with "oops"
        // `a{|}oopsd{|}e`
        // => selecte the entire replacement, and continue to the prev end
        let (mut text, cursor) = TextChange::try_extract_cursor("ab{|}cd{|}e".to_string());

        let changes = [
            TextChange::Insert(
                ByteSpan::new(text.find("b").unwrap(), text.find("d").unwrap()),
                "oops".into(),
            ),
            TextChange::Insert(ByteSpan::new(text.len(), text.len()), "!".into()),
        ];

        let cursor =
            apply_text_changes(&mut text, Some(cursor.unwrap().unordered()), changes).unwrap();
        assert_eq!(
            TextChange::encode_cursor(&text, cursor.unwrap()),
            "a{|}oopsd{|}e!"
        );
    }

    #[test]
    pub fn test_cursor_adjacement_selection_end_inside_replacement() {
        // means that the right side of selection is inside the replacement
        // example
        // `ab{|}cd{|}efj`
        //        ^____^ => replace with "oops"
        // `ab{|}coops{|}j`
        // => selecte the entire replacement, and continue to the prev start
        let (mut text, cursor) = TextChange::try_extract_cursor("ab{|}cd{|}efj".to_string());

        let changes = [
            TextChange::Insert(
                ByteSpan::new(text.find("d").unwrap(), text.find("j").unwrap()),
                "oops".into(),
            ),
            TextChange::Insert(ByteSpan::new(0, 1), "!!".into()),
        ];

        let cursor =
            apply_text_changes(&mut text, Some(cursor.unwrap().unordered()), changes).unwrap();
        assert_eq!(
            TextChange::encode_cursor(&text, cursor.unwrap()),
            "!!b{|}coops{|}j"
        );
    }
}
