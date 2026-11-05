//! Provides utility functions for handling file names and extensions.

#[cfg(feature = "ssr")]
use leptos::prelude::ServerFnError;

/// The image type a file with this extension (any case) is expected to hold.
///
/// The uploads in this app are images only (see
/// [`UploadConfiguration::get_details`](crate::config::upload::UploadConfiguration::get_details)),
/// so the set is small and fixed.
///
/// # Example
/// ```
/// use white_label::utils::files::image_mime_for_extension;
/// assert_eq!(image_mime_for_extension("JPG"), Some("image/jpeg"));
/// assert_eq!(image_mime_for_extension("txt"), None);
/// ```
pub const fn image_mime_for_extension(extension: &str) -> Option<&'static str> {
    if extension.eq_ignore_ascii_case("png") {
        Some("image/png")
    } else if extension.eq_ignore_ascii_case("jpg") || extension.eq_ignore_ascii_case("jpeg") {
        Some("image/jpeg")
    } else if extension.eq_ignore_ascii_case("gif") {
        Some("image/gif")
    } else if extension.eq_ignore_ascii_case("webp") {
        Some("image/webp")
    } else {
        None
    }
}

/// The image type a file's first bytes identify, or `None` when they match no
/// supported image signature.
///
/// This is the check that cannot be talked out of: a client's own claim about a
/// file (its `Content-Type`, or an extension) is the client's opinion, while the
/// signature is what arrived.
///
/// # Example
/// ```
/// use white_label::utils::files::image_mime_in_bytes;
/// assert_eq!(
///     image_mime_in_bytes(b"\x89PNG\r\n\x1a\nfake png body"),
///     Some("image/png")
/// );
/// assert_eq!(image_mime_in_bytes(b"just some text"), None);
/// ```
pub fn image_mime_in_bytes(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']) {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    // A `.webp` is a `RIFF` container; the type sits at offset 8.
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return Some("image/webp");
    }
    None
}

/// Extensions that stand for an accepted mime type in a file picker's `accept`
/// list, or an empty slice when the type has no extension of its own.
fn extensions_for_mime(mime_type: &str) -> &'static [&'static str] {
    match mime_type {
        "image/png" => &["png"],
        "image/jpeg" => &["jpg", "jpeg"],
        "image/gif" => &["gif"],
        "image/webp" => &["webp"],
        _ => &[],
    }
}

/// Builds a file picker's `accept` list from the mime types a configuration
/// allows, as extension tokens (`.png,.PNG,…`).
///
/// Browsers accept both mime types and extensions here, but the file dialog in
/// a dioxus window only understands extensions (and the `image/*` shorthand):
/// a mime type there becomes an empty filter, which offers every file on disk.
/// A mime type with no extension of its own is passed through unchanged, so an
/// unusual configuration cannot silently lose a type.
///
/// Every extension is listed twice, lower- and upper-case. A dioxus window's
/// native filter (rfd's GTK backend) matches the filename extension verbatim
/// with a case-sensitive `*.ext` pattern, so a lowercase-only list hides
/// `PHOTO.JPG`; browsers ignore the case, so the extra tokens change nothing
/// there.
///
/// # Example
/// ```
/// use white_label::utils::files::accept_extensions;
/// let accepted = ["image/jpeg".to_string(), "image/png".to_string()];
/// assert_eq!(accept_extensions(&accepted), ".jpg,.JPG,.jpeg,.JPEG,.png,.PNG");
/// ```
#[must_use]
pub fn accept_extensions(mime_types: &[String]) -> String {
    let mut tokens: Vec<String> = Vec::new();
    for mime_type in mime_types {
        let mapped = extensions_for_mime(mime_type);
        if mapped.is_empty() {
            // An unknown type keeps its mime token rather than disappearing.
            if !tokens.contains(mime_type) {
                tokens.push(mime_type.clone());
            }
            continue;
        }
        // A picker that lists a type twice is confusing, and two accepted mimes
        // can share an extension (a configuration that allowed both
        // `image/jpeg` and `.jpe`, say).
        for extension in mapped {
            // Lower- and upper-case variants (see the note above); the exact
            // `contains` keeps both cases but drops a repeat of either.
            for token in [
                format!(".{extension}"),
                format!(".{}", extension.to_uppercase()),
            ] {
                if !tokens.contains(&token) {
                    tokens.push(token);
                }
            }
        }
    }
    tokens.join(",")
}

/// How many bytes of a stored file to read back for its signature. `webp` is
/// the furthest into a file any of the accepted types puts its marker (offset
/// 8..12), so this covers all of them.
pub const SIGNATURE_BYTES: usize = 12;

/// The first `limit` bytes of a file, which is all an image signature needs.
///
/// This is how an upload decides what a stored file really is: once it is on
/// disk, and before anything points at it.
///
/// # Errors
/// [`std::io::Error`] if the file cannot be opened or read.
#[cfg(feature = "ssr")]
pub fn first_bytes(path: &str, limit: usize) -> std::io::Result<Vec<u8>> {
    use std::io::Read as _;

    let mut file = std::fs::File::open(path)?;
    let mut signature = Vec::with_capacity(limit);
    let mut buffer = [0u8; 64];
    while signature.len() < limit {
        let wanted = limit.min(buffer.len());
        let read = file.read(&mut buffer[..wanted])?;
        if read == 0 {
            // A file shorter than `limit`: what it does hold is still enough to
            // tell an image from a non-image.
            break;
        }
        signature.extend_from_slice(&buffer[..read]);
    }
    Ok(signature)
}

/// Get the file extension from a file name
///
/// # Example
/// ```
/// use white_label::utils::files::file_extension;
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
        Some(rename_as) => match file_extension(file_name) {
            Some(file_extension) => {
                name = format!("{rename_as}.{file_extension}");
            }
            None => {
                name.clone_from(&rename_as);
            }
        },
        None => {
            name = file_name.to_string();
        }
    }
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
    fn the_first_bytes_come_back_whatever_the_file_holds() {
        let file = tempfile::NamedTempFile::new().expect("temp file");
        std::fs::write(
            file.path(),
            b"\x89PNG\r\n\x1a\na body longer than the signature",
        )
        .expect("write");
        let signature = first_bytes(&file.path().to_string_lossy(), 12).expect("first bytes");
        assert_eq!(signature.len(), 12, "as many bytes as were asked for");
        assert_eq!(
            image_mime_in_bytes(&signature),
            Some("image/png"),
            "enough to recognise the file"
        );

        // A file shorter than the signature length still tells its story.
        let short = tempfile::NamedTempFile::new().expect("temp file");
        std::fs::write(short.path(), b"GIF89a").expect("write");
        let short_signature =
            first_bytes(&short.path().to_string_lossy(), 12).expect("first bytes");
        assert_eq!(short_signature, b"GIF89a");

        let missing = first_bytes("/no/such/file/here", 12);
        assert!(
            matches!(missing, Err(ref error) if error.kind() == std::io::ErrorKind::NotFound),
            "a missing file is an io error, not a panic"
        );
    }

    #[test]
    fn test_file_extension() {
        let file_name = "file.txt";
        let extension = file_extension(file_name);
        assert_eq!(extension, Some("txt"));
    }

    #[test]
    fn image_types_come_from_the_extension() {
        assert_eq!(image_mime_for_extension("png"), Some("image/png"));
        assert_eq!(image_mime_for_extension("webp"), Some("image/webp"));
        assert_eq!(image_mime_for_extension("gif"), Some("image/gif"));
        // The window's dialog hands back whatever case the file on disk has.
        assert_eq!(image_mime_for_extension("JPG"), Some("image/jpeg"));
        assert_eq!(image_mime_for_extension("JPEG"), Some("image/jpeg"));
        assert_eq!(image_mime_for_extension("txt"), None);
        assert_eq!(image_mime_for_extension(""), None);
    }

    #[test]
    fn image_types_come_from_the_bytes() {
        assert_eq!(
            image_mime_in_bytes(b"\x89PNG\r\n\x1a\nbody"),
            Some("image/png")
        );
        assert_eq!(
            image_mime_in_bytes(b"\xff\xd8\xff\xe0body"),
            Some("image/jpeg")
        );
        assert_eq!(image_mime_in_bytes(b"GIF89abody"), Some("image/gif"));
        assert_eq!(image_mime_in_bytes(b"GIF87abody"), Some("image/gif"));
        assert_eq!(
            image_mime_in_bytes(b"RIFF,\0\0\0WEBPVP8 "),
            Some("image/webp")
        );
        // Not enough bytes to hold a signature, and no signature in them.
        assert_eq!(image_mime_in_bytes(b""), None);
        assert_eq!(image_mime_in_bytes(b"RIFF"), None);
        assert_eq!(image_mime_in_bytes(b"just some text"), None);
        // A `RIFF` container that is not a webp (a `.avi`, say) is not one of
        // the accepted types.
        assert_eq!(image_mime_in_bytes(b"RIFF,\0\0\0AVI LIST"), None);
    }

    #[test]
    fn accept_list_uses_extension_tokens() {
        let accepted = [
            "image/jpeg".to_string(),
            "image/png".to_string(),
            "image/gif".to_string(),
            "image/webp".to_string(),
        ];
        assert_eq!(
            accept_extensions(&accepted),
            ".jpg,.JPG,.jpeg,.JPEG,.png,.PNG,.gif,.GIF,.webp,.WEBP"
        );
        // A type this helper does not know keeps its mime token rather than
        // quietly disappearing from the picker.
        let odd = ["application/pdf".to_string(), "image/png".to_string()];
        assert_eq!(accept_extensions(&odd), "application/pdf,.png,.PNG");
        assert_eq!(accept_extensions(&[]), "");
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
