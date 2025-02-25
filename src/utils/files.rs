#[cfg(feature = "ssr")]
use leptos::prelude::ServerFnError;

/// Get the file extension from a file name
///
/// # Example
/// ```
/// let file_name = "file.txt";
/// let extension = file_extension(file_name);
/// assert_eq!(extension, Some("txt"));
/// ```
pub fn file_extension(file_name: &str) -> Option<&str> {
    if !file_name.contains('.') {
        return None;
    }
    let mut parts = file_name.split('.');
    parts.next_back()
}

/// Get a valid filename for the file.
/// This function will return a filename that is unique if `rename` is true.
/// If `rename` is false, it will return the original filename.
///
/// # Errors
/// - If the file already exists and `file_exists_ok` is false, it will return an error.
#[cfg(feature = "ssr")]
pub fn valid_file_name(
    file_name: &str,
    rename: Option<String>,
    path: &str,
    overwrite_existing_file: bool,
) -> Result<String, ServerFnError> {
    let mut name = file_name.to_string();

    match rename {
        Some(rename_as) => {
            match file_extension(file_name) {
                Some(file_extension) => {
                    name = format!("{rename_as}.{file_extension}");
                }
                None => {
                    name.clone_from(&rename_as);
                }
            };
        }
        None => {
            name = file_name.to_string();
        }
    };
    name = format!("{}-{name}", chrono::Utc::now().timestamp());

    if !overwrite_existing_file {
        // Check if the file already exists
        // If it does, return an error
        let file_path = format!("{path}/{name}");
        if std::path::Path::new(&file_path).exists() {
            return Err(ServerFnError::new("File already exists.".to_string()));
        }
    }

    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_extension() {
        let file_name = "file.txt";
        let extension = file_extension(file_name);
        assert_eq!(extension, Some("txt"));
    }

    #[test]
    fn test_file_extension_no_extension() {
        let file_name = "file";
        let extension = file_extension(file_name);
        assert_eq!(extension, None);
    }

    #[test]
    fn test_file_extension_multiple_dots() {
        let file_name = "file.with.multiple.dots.txt";
        let extension = file_extension(file_name);
        assert_eq!(extension, Some("txt"));
    }
}
