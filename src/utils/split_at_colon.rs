/// Split a string at the first occurrence of a colon
/// Returns a tuple of the string before the colon and the string after the semi-colon
pub fn split_at_colon(s: &str) -> (String, String) {
    let mut split = s.splitn(2, ':');
    let first = split.next().unwrap_or_default().trim().to_string();
    let second = split.next().unwrap_or_default().trim().to_string();
    (first, second)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_string_with_colon() {
        assert_eq!(split_at_colon("A:B"), ("A".to_string(), "B".to_string()));
    }

    #[test]
    fn test_split_string_without_colon() {
        assert_eq!(split_at_colon("AB"), ("AB".to_string(), String::new()));
    }

    #[test]
    fn test_split_string_with_colon_and_spaces() {
        assert_eq!(split_at_colon("A : B"), ("A".to_string(), "B".to_string()));
    }

    #[test]
    fn test_split_string_with_colon_and_spaces_and_newlines() {
        assert_eq!(split_at_colon("A\n : \nB"), ("A".to_string(), "B".to_string()));
    }

    #[test]
    fn test_split_string_with_multiple_colons() {
        assert_eq!(split_at_colon("A:B:C"), ("A".to_string(), "B:C".to_string()));
    }
}
