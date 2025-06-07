//! Turn a string into a slug
//!
//! This module provides a function to slugify a string, which means
//! - Transliterate unicode text to ASCII
//! - Remove double spaces from the text
//! - Replace spaces with hyphens
//! # Example
//! ```
//! use white_label::utils::slugify;
//! let slug = slugify("The Quick Brown Fox");
//! assert_eq!(slug, "the-quick-brown-fox");
//! ```
//! # Note
//! This function is useful for creating slugs for URLs, filenames, etc.

use deunicode::deunicode;

/// Slugify a text
///
/// This function takes a text and returns a slugified version of it.
/// - It transliterates unicode text to ASCII
/// - It removes double spaces from the text
/// - It replaces spaces with hyphens
#[must_use]
pub fn slugify(text: &str) -> String {
    // Transliterates unicode text to ASCII
    let mut slug = deunicode(text);
    let binding = slug.to_ascii_lowercase();
    slug = binding;

    // Remove all punctuation from the text
    let binding = slug
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>();
    slug = binding;

    // Remove double spaces from the text
    let binding = clean_whitespace(&slug);
    slug = binding;

    // Replace new lines with hyphens
    let binding = slug.replace('\n', "-");
    slug = binding;

    // Replace spaces with hyphens
    slug.to_lowercase().replace(' ', "-")
}

/// Trim whitespace from a string without using regex
#[must_use]
pub fn clean_whitespace(s: &str) -> String {
    let mut new_str = s.trim().to_owned();
    let mut prev = ' '; // The initial value doesn't really matter
    new_str.retain(|ch| {
        let result = ch != ' ' || prev != ' ';
        prev = ch;
        result
    });
    new_str
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify_with_stop_words() {
        assert_eq!(slugify("The Quick Brown Fox"), "the-quick-brown-fox");
    }

    #[test]
    fn test_slugify_with_extra_spaces() {
        assert_eq!(slugify("The  Quick   Brown    Fox"), "the-quick-brown-fox");
    }

    #[test]
    fn test_slugify_with_uppercase() {
        assert_eq!(slugify("The Quick Brown Fox"), "the-quick-brown-fox");
    }

    #[test]
    fn test_slugify_with_unicode() {
        assert_eq!(
            slugify("Ἰοὺ ἰού· τὰ πάντʼ ἂν ἐξήκοι σαφῆ."),
            "iou-iou-ta-pant-an-exekoi-saphe"
        );
    }

    #[test]
    fn test_slugify_with_punctuation() {
        assert_eq!(slugify("The Quick, Brown Fox"), "the-quick-brown-fox");
    }

    #[test]
    fn test_slugify_with_emoji() {
        assert_eq!(slugify("⏩ 🦊"), "fast-forward-fox-face");
    }

    #[test]
    fn test_trim_whitespace_with_double_spaces() {
        assert_eq!(slugify("The  Quick  Brown  Fox"), "the-quick-brown-fox");
    }

    #[test]
    fn test_trim_whitespace_with_triple_spaces() {
        assert_eq!(slugify("The   Quick   Brown   Fox"), "the-quick-brown-fox");
    }

    #[test]
    fn test_trim_whitespace_with_quadruple_spaces() {
        assert_eq!(
            slugify("The    Quick    Brown    Fox"),
            "the-quick-brown-fox"
        );
    }

    #[test]
    fn test_trim_whitespace_with_leading_spaces() {
        assert_eq!(slugify("   The Quick Brown Fox"), "the-quick-brown-fox");
    }

    #[test]
    fn test_trim_whitespace_with_trailing_spaces() {
        assert_eq!(slugify("The Quick Brown Fox   "), "the-quick-brown-fox");
    }

    #[test]
    fn test_remove_trailing_hyphen() {
        assert_eq!(slugify("The Quick Brown Fox-!-"), "the-quick-brown-fox");
    }

    #[test]
    fn test_remove_leading_hyphen() {
        assert_eq!(slugify("-!-The Quick Brown Fox"), "the-quick-brown-fox");
    }

    #[test]
    fn test_new_line_character() {
        assert_eq!(slugify("The\nQuick\nBrown\nFox\n"), "the-quick-brown-fox");
    }
}
