/// Shorten a string. It will either be to the first . or the first new line
#[must_use]
pub fn shorten_string(s: String) -> String {
    let mut new_str = s;
    if let Some(index) = new_str.find('.') {
        new_str.truncate(index + 1);
    }
    if let Some(index) = new_str.find('\n') {
        new_str.truncate(index);
    }

    // Strip markdown
    new_str = new_str.replace('*', "");
    new_str = new_str.replace('#', "");
    new_str = new_str.replace('[', "");
    new_str = new_str.replace(']', "");
    new_str = new_str.replace('(', "");
    new_str = new_str.replace(')', "");

    new_str
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shorten_string() {
        assert_eq!(
            shorten_string("This is a test. Second sentence".to_string()),
            "This is a test.".to_string()
        );
    }

    #[test]
    fn test_shorten_string_with_newline() {
        assert_eq!(shorten_string("This is a test\nSecond line".to_string()), "This is a test".to_string());
    }

    #[test]
    fn test_shorten_string_with_markdown() {
        assert_eq!(shorten_string("# This is a test.".to_string()), " This is a test.".to_string());
    }

    #[test]
    fn test_shorten_string_with_markdown_and_newline() {
        assert_eq!(
            shorten_string("# This is a test.\nSecond Line.".to_string()),
            " This is a test.".to_string()
        );
    }
}
