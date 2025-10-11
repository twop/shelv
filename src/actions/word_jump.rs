use crate::{app_state::WordJumpAddress, byte_span::UnOrderedByteSpan};
use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct JumpLabel([char; 2]);

impl JumpLabel {
    // TODO: use SmolStr instead of String for better performance
    pub fn to_string(&self) -> String {
        format!("{}{}", self.0[0], self.0[1])
    }
}

fn assign_label(symbols: &[char], seq_index: usize) -> Option<JumpLabel> {
    let total = symbols.len();
    if total * total - 1 < seq_index {
        // it means that we exhausted all the combinations
        return None;
    }

    let first = seq_index / total;
    let second = seq_index - first * total;

    Some(JumpLabel([symbols[first], symbols[second]]))
}

pub fn create_jump_points(
    text: &str,
    cursor: UnOrderedByteSpan,
    jump_symbols: &[char],
) -> Option<Vec<WordJumpAddress>> {
    let mut jumps: Vec<_> = iterate_over_words(text)
        .map(|(_, span)| WordJumpAddress {
            span,
            label: JumpLabel(['-', '-']),
        })
        .collect();

    if jumps.is_empty() {
        // TODO should it be None or Err?
        return None;
    }

    let total_words = jumps.len();

    let split_pos = jumps
        .iter()
        .enumerate()
        .find_map(|(i, jump)| (jump.span.end >= cursor.start).then(|| i))
        .unwrap_or(total_words - 1);

    let mut left = split_pos as isize - 1;
    let mut right = split_pos;

    for i in 0..jumps.len() {
        let should_go_right = i % 2 == 0;
        let reached_end_right = right >= total_words;
        let reached_end_left = left < 0;

        let Some(label) = assign_label(jump_symbols, i) else {
            // means that we exhausted the label amount
            break;
        };

        if (should_go_right || !should_go_right && reached_end_left) && !reached_end_right {
            jumps[right].label = label;
            right = right + 1;
        } else if (should_go_right && reached_end_right || !should_go_right) && !reached_end_left {
            jumps[left as usize].label = label;
            left = left - 1;
        } else {
            break;
        }
    }

    let left_boundary = (left + 1).max(0) as usize;
    let removed_from_right = jumps.drain(right..).count();
    let removed_from_left = jumps.drain(0..left_boundary).count();

    assert_eq!(removed_from_left, left_boundary as usize);
    assert_eq!(removed_from_right, total_words - right.min(total_words));

    Some(jumps)
}

// Using LazyLock for compile-once performance
static WORD_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    // \p{L} matches any Unicode letter, \p{N} matches any Unicode number
    // {2,} ensures at least 2 characters
    Regex::new(r"[\p{L}\p{N}]{2,}").unwrap()
});

/// Returns an iterator over word matches, each containing byte positions and string slice
pub fn iterate_over_words(text: &str) -> impl Iterator<Item = (&str, UnOrderedByteSpan)> {
    WORD_REGEX
        .find_iter(text)
        .map(|m| (m.as_str(), UnOrderedByteSpan::new(m.start(), m.end())))
}

#[cfg(test)]
mod label_assigning_tests {
    use crate::actions::word_jump::{JumpLabel, assign_label};

    #[test]
    fn test_simple_label_allocation() {
        let label_pallete = ['a', 'b'];

        let labels: Vec<_> = (0..5)
            .map(|index| assign_label(&label_pallete, index))
            .collect();

        assert_eq!(
            labels,
            vec![
                Some(JumpLabel(['a', 'a'])),
                Some(JumpLabel(['a', 'b'])),
                Some(JumpLabel(['b', 'a'])),
                Some(JumpLabel(['b', 'b'])),
                None
            ]
        );
    }
}
#[cfg(test)]
mod word_iteration_tests {
    use super::*;

    // Helper function for tests that just need the word strings
    fn extract_words(text: &str) -> Vec<&str> {
        iterate_over_words(text).map(|(s, _)| s).collect()
    }

    #[test]
    fn test_english_words() {
        let text = "Hello, world! This is a test; with various-separators.";
        assert_eq!(
            extract_words(text),
            vec![
                "Hello",
                "world",
                "This",
                "is",
                "test",
                "with",
                "various",
                "separators"
            ]
        );
    }

    #[test]
    fn test_single_character_exclusion() {
        let text = "a bb c dd e ff";
        assert_eq!(extract_words(text), vec!["bb", "dd", "ff"]);
    }

    #[test]
    fn test_numbers_two_digits_or_more() {
        let text = "1 22 3 456 7 8901";
        assert_eq!(extract_words(text), vec!["22", "456", "8901"]);
    }

    #[test]
    fn test_numbers_with_words() {
        let text = "test1, test22, 9test, 99test";
        assert_eq!(
            extract_words(text),
            vec!["test1", "test22", "9test", "99test"]
        );
    }

    #[test]
    fn test_only_numbers() {
        let text = "123,456;789";
        assert_eq!(extract_words(text), vec!["123", "456", "789"]);
    }

    #[test]
    fn test_japanese_words() {
        let text = "こんにちは、世界！テスト;です。";
        assert_eq!(
            extract_words(text),
            vec!["こんにちは", "世界", "テスト", "です"]
        );
    }

    #[test]
    fn test_russian_words() {
        let text = "Привет, мир! Это тест;проверка.";
        assert_eq!(
            extract_words(text),
            vec!["Привет", "мир", "Это", "тест", "проверка"]
        );
    }

    #[test]
    fn test_arabic_words() {
        let text = "مرحبا، العالم! هذا اختبار;للكلمات.";
        assert_eq!(
            extract_words(text),
            vec!["مرحبا", "العالم", "هذا", "اختبار", "للكلمات"]
        );
    }

    #[test]
    fn test_mixed_languages() {
        let text = "Hello世界, test123; café-résumé!";
        assert_eq!(
            extract_words(text),
            vec!["Hello世界", "test123", "café", "résumé"]
        );
    }

    #[test]
    fn test_various_separators() {
        let text = "word1🎉word2@#$%test;;another|||final";
        assert_eq!(
            extract_words(text),
            vec!["word1", "word2", "test", "another", "final"]
        );
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(extract_words(""), Vec::<&str>::new());
    }

    #[test]
    fn test_only_separators() {
        let text = ",,,;;;...!!!";
        assert_eq!(extract_words(text), Vec::<&str>::new());
    }

    #[test]
    fn test_only_single_characters() {
        let text = "a,b;c.d!e";
        assert_eq!(extract_words(text), Vec::<&str>::new());
    }
}

#[cfg(test)]
mod create_jump_points_tests {
    use super::*;

    // Helper function to extract both words and labels for easier debugging
    fn extract_words_and_labels<'t>(
        text: &'t str,
        jumps: &[WordJumpAddress],
    ) -> Vec<(&'t str, JumpLabel)> {
        jumps
            .iter()
            .map(|j| (&text[j.span.start..j.span.end], j.label))
            .collect()
    }

    #[test]
    fn test_two_words_cursor_at_end() {
        let text = "hello world";
        let jumps =
            create_jump_points(text, UnOrderedByteSpan::point(text.len()), &['a', 'b']).unwrap();

        assert_eq!(
            extract_words_and_labels(text, &jumps),
            vec![
                ("hello", JumpLabel(['a', 'b'])),
                ("world", JumpLabel(['a', 'a'])),
            ]
        );
    }

    #[test]
    fn test_two_words_cursor_at_start() {
        let text = "hello world";
        // At the start
        let jumps = create_jump_points(text, UnOrderedByteSpan::point(0), &['a', 'b']).unwrap();

        assert_eq!(
            extract_words_and_labels(text, &jumps),
            vec![
                ("hello", JumpLabel(['a', 'a'])),
                ("world", JumpLabel(['a', 'b'])),
            ]
        );
    }

    #[test]
    fn test_exhausted_jump_symbols() {
        let text = "zero one two three four five";
        let cursor = UnOrderedByteSpan::point(13); // Inside "three"
        let jumps = create_jump_points(text, cursor, &['a', 'b']).unwrap();

        // With only 2 symbols (a,b), we can only create 4 labels: aa, ab, ba, bb
        assert_eq!(
            extract_words_and_labels(text, &jumps),
            vec![
                // "zero" doesn't fit
                ("one", JumpLabel(['b', 'b'])),
                ("two", JumpLabel(['a', 'b'])),
                ("three", JumpLabel(['a', 'a'])),
                ("four", JumpLabel(['b', 'a'])),
                // "five" doesn't fit
            ]
        );
    }

    #[test]
    fn test_two_words_cursor_spanning_both() {
        let text = "hello world";
        // From middle of "hello" to middle of "world"
        let cursor = UnOrderedByteSpan::new(3, 8);
        let jumps = create_jump_points(text, cursor, &['a', 'b']).unwrap();

        // Should behave the same as if cursor was at the start
        assert_eq!(
            extract_words_and_labels(text, &jumps),
            vec![
                ("hello", JumpLabel(['a', 'a'])),
                ("world", JumpLabel(['a', 'b'])),
            ]
        );
    }

    #[test]
    fn test_one_word_cursor_in_middle() {
        let text = "hello";
        let cursor = UnOrderedByteSpan::point(2); // In middle of "hello"
        let jumps = create_jump_points(text, cursor, &['a', 'b']).unwrap();

        assert_eq!(jumps.len(), 1);
        assert_eq!(
            extract_words_and_labels(text, &jumps),
            vec![("hello", JumpLabel(['a', 'a'])),]
        );

        // Verify the word
        let word_text = &text[jumps[0].span.start..jumps[0].span.end];
        assert_eq!(word_text, "hello");
    }
}
