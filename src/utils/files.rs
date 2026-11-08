//! Provides utility functions for handling file names and extensions.

#[cfg(feature = "server")]
use crate::error::AppError;

/// The image type a file with this extension (any case) is expected to hold.
///
/// The image uploads in this app are few and fixed (see
/// [`UploadConfiguration::get_details`](crate::config::upload::UploadConfiguration::get_details));
/// audio files have their own helper,
/// [`audio_mime_for_extension`], and [`upload_mime_for_extension`] covers both.
///
/// # Example
/// ```
/// use white_label::utils::files::image_mime_for_extension;
/// assert_eq!(image_mime_for_extension("JPG"), Some("image/jpeg"));
/// assert_eq!(image_mime_for_extension("txt"), None);
/// ```
#[must_use]
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
#[must_use]
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

/// The audio type a file with this extension (any case) is expected to hold.
///
/// These are the formats a track accepts
/// ([`UploadConfiguration::TrackAudio`](crate::config::upload::UploadConfiguration::TrackAudio)):
/// the four every target browser plays natively.
///
/// # Example
/// ```
/// use white_label::utils::files::audio_mime_for_extension;
/// assert_eq!(audio_mime_for_extension("FLAC"), Some("audio/flac"));
/// assert_eq!(audio_mime_for_extension("png"), None);
/// ```
#[must_use]
pub const fn audio_mime_for_extension(extension: &str) -> Option<&'static str> {
    if extension.eq_ignore_ascii_case("mp3") {
        Some("audio/mpeg")
    } else if extension.eq_ignore_ascii_case("wav") {
        Some("audio/wav")
    } else if extension.eq_ignore_ascii_case("flac") {
        Some("audio/flac")
    } else if extension.eq_ignore_ascii_case("ogg") {
        Some("audio/ogg")
    } else {
        None
    }
}

/// The type a file's extension stands for, image or audio.
///
/// The client builds and both upload handlers work this out whenever the type a
/// client declared is not one the configuration accepts (see
/// [`claimed_upload_mime`]).
///
/// # Example
/// ```
/// use white_label::utils::files::upload_mime_for_extension;
/// assert_eq!(upload_mime_for_extension("webp"), Some("image/webp"));
/// assert_eq!(upload_mime_for_extension("flac"), Some("audio/flac"));
/// assert_eq!(upload_mime_for_extension("txt"), None);
/// ```
#[must_use]
pub fn upload_mime_for_extension(extension: &str) -> Option<&'static str> {
    image_mime_for_extension(extension).or_else(|| audio_mime_for_extension(extension))
}

/// The type an upload claims to be: the declared type when `accepted` holds it,
/// otherwise the type the file's extension stands for.
///
/// Failing both, the declared type itself is returned, so a rejection can name
/// what the client sent.
///
/// A browser declares the file's own type. A dioxus window declares one dioxus
/// invented from a table of *web asset* types that holds no images and no audio
/// (a `.png` and a `.flac` both come back as `text/html; charset=utf-8`), and a
/// browser's own table may hold a legacy spelling the configuration does not
/// list (`audio/x-flac`). Only the bytes decide what is
/// stored ([`image_mime_in_bytes`] and [`audio_mime_in_bytes`]); this choice only
/// decides whether a body is worth streaming at all, and what the rejection
/// message says.
///
/// # Example
/// ```
/// use white_label::utils::files::claimed_upload_mime;
/// let accepted = ["image/png".to_string()];
/// assert_eq!(claimed_upload_mime("image/png", "cover.png", &accepted), "image/png");
/// assert_eq!(claimed_upload_mime("text/html; charset=utf-8", "cover.PNG", &accepted), "image/png");
/// assert_eq!(claimed_upload_mime("text/plain", "notes.txt", &accepted), "text/plain");
/// ```
#[must_use]
pub fn claimed_upload_mime<'a>(declared: &'a str, file_name: &str, accepted: &[String]) -> &'a str {
    if accepted.iter().any(|allowed| allowed == declared) {
        return declared;
    }
    upload_mime_for_extension(file_extension(file_name).unwrap_or_default()).unwrap_or(declared)
}

/// The audio type a file's first bytes identify, or `None` when they match no
/// supported audio signature.
///
/// The same rule as [`image_mime_in_bytes`]: a client's claim is an opinion,
/// the signature is what arrived.
///
/// Two signatures are wider than the format's name suggests. An mp3 often
/// begins with an `ID3v2` tag rather than a frame, so both count as
/// `audio/mpeg`; a file that carries an ID3 tag *and* is really another format
/// (some FLAC encoders allow it) is labelled `audio/mpeg` here and still
/// stored, because the bytes are an accepted audio type — the browser's own
/// sniffing decides whether it plays.
///
/// # Example
/// ```
/// use white_label::utils::files::audio_mime_in_bytes;
/// assert_eq!(
///     audio_mime_in_bytes(b"fLaC\0\0\0\x18fake flac body"),
///     Some("audio/flac")
/// );
/// assert_eq!(audio_mime_in_bytes(b"just some text"), None);
/// ```
#[must_use]
pub fn audio_mime_in_bytes(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"fLaC") {
        return Some("audio/flac");
    }
    if bytes.starts_with(b"OggS") {
        return Some("audio/ogg");
    }
    // A `.wav` is a `RIFF` container; the type sits at offset 8, exactly where a
    // `.webp` puts its own marker.
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") {
        return Some("audio/wav");
    }
    if bytes.starts_with(b"ID3") {
        return Some("audio/mpeg");
    }
    // An mp3 frame starts with an 11-bit sync mark: 0xFF then the top three
    // bits of the next byte.
    if let (Some(&0xff), Some(second)) = (bytes.first(), bytes.get(1))
        && second & 0xe0 == 0xe0
    {
        return Some("audio/mpeg");
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
        "audio/mpeg" => &["mp3"],
        "audio/wav" => &["wav"],
        "audio/flac" => &["flac"],
        "audio/ogg" => &["ogg"],
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

/// How many bytes of a stored file to read back for its signature.
///
/// `webp` and `wav` are the furthest into a file any of the accepted types puts
/// its marker (offset 8..12 inside a `RIFF` container), so this covers all of
/// them.
pub const SIGNATURE_BYTES: usize = 12;

/// The first `limit` bytes of a file, which is all an image signature needs.
///
/// This is how an upload decides what a stored file really is: once it is on
/// disk, and before anything points at it.
///
/// # Errors
/// [`std::io::Error`] if the file cannot be opened or read.
#[cfg(feature = "server")]
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
#[must_use]
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
#[cfg(feature = "server")]
pub fn valid_file_name(
    file_name: &str,
    rename: Option<String>,
    path: &str,
    overwrite_existing_file: bool,
) -> Result<String, AppError> {
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
            return Err(AppError::BadRequest("File already exists.".to_string()));
        }
    }

    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::upload::UploadConfiguration;

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
    fn audio_types_come_from_the_extension() {
        assert_eq!(audio_mime_for_extension("mp3"), Some("audio/mpeg"));
        assert_eq!(audio_mime_for_extension("wav"), Some("audio/wav"));
        assert_eq!(audio_mime_for_extension("flac"), Some("audio/flac"));
        assert_eq!(audio_mime_for_extension("ogg"), Some("audio/ogg"));
        // As with images, the window's dialog keeps the case the file has.
        assert_eq!(audio_mime_for_extension("FLAC"), Some("audio/flac"));
        assert_eq!(audio_mime_for_extension("PNG"), None);
        assert_eq!(audio_mime_for_extension(""), None);
    }

    #[test]
    fn one_helper_covers_both_families_of_upload() {
        assert_eq!(upload_mime_for_extension("webp"), Some("image/webp"));
        assert_eq!(upload_mime_for_extension("flac"), Some("audio/flac"));
        assert_eq!(upload_mime_for_extension("txt"), None);
    }

    #[test]
    fn a_declared_type_the_configuration_rejects_falls_back_to_the_extension() {
        let accepted = UploadConfiguration::TrackAudio.get_details().mime_types;
        // A browser's own type is accepted as it stands.
        assert_eq!(
            claimed_upload_mime("audio/flac", "song.flac", &accepted),
            "audio/flac"
        );
        // A legacy spelling the configuration does not list: the extension wins.
        assert_eq!(
            claimed_upload_mime("audio/x-flac", "song.FLAC", &accepted),
            "audio/flac"
        );
        // The window build's invented type, same as for an image upload.
        assert_eq!(
            claimed_upload_mime("text/html; charset=utf-8", "song.mp3", &accepted),
            "audio/mpeg"
        );
        // Nothing to fall back to, so the claim itself gets rejected.
        assert_eq!(
            claimed_upload_mime("text/plain", "notes.txt", &accepted),
            "text/plain"
        );

        // The image slots use the same fallback, which is where the window
        // build's invented type was first seen.
        let images = UploadConfiguration::Avatar.get_details().mime_types;
        assert_eq!(
            claimed_upload_mime("image/gif", "animation.gif", &images),
            "image/gif"
        );
        assert_eq!(
            claimed_upload_mime("text/html; charset=utf-8", "cover.PNG", &images),
            "image/png"
        );
        assert_eq!(
            claimed_upload_mime("application/octet-stream", "cover.webp", &images),
            "image/webp"
        );
        // A claim the configuration accepts stands even when the extension
        // would say otherwise; the signature decides what is stored.
        assert_eq!(
            claimed_upload_mime("image/jpeg", "cover.png", &images),
            "image/jpeg"
        );
    }

    #[test]
    fn audio_types_come_from_the_bytes() {
        assert_eq!(
            audio_mime_in_bytes(b"fLaC\0\0\0\x18body"),
            Some("audio/flac")
        );
        assert_eq!(audio_mime_in_bytes(b"OggS\0\x02body"), Some("audio/ogg"));
        assert_eq!(
            audio_mime_in_bytes(b"RIFF,\0\0\0WAVEfmt "),
            Some("audio/wav")
        );
        // An mp3 that begins with its ID3v2 tag, and one that begins with a
        // frame (0xFF plus the 11-bit sync mark).
        assert_eq!(
            audio_mime_in_bytes(b"ID3\x04\0\0\0body"),
            Some("audio/mpeg")
        );
        assert_eq!(
            audio_mime_in_bytes(b"\xff\xfb\x90\x00body"),
            Some("audio/mpeg")
        );
        assert_eq!(audio_mime_in_bytes(b"\xff\xf3body"), Some("audio/mpeg"));

        // Not audio: a `RIFF` that is not a wave, a sync mark that is not one,
        // and bytes that are only text.
        assert_eq!(audio_mime_in_bytes(b"RIFF,\0\0\0AVI LIST"), None);
        assert_eq!(audio_mime_in_bytes(b"\xff\x20body"), None);
        assert_eq!(audio_mime_in_bytes(b"\x89PNG\r\n\x1a\nbody"), None);
        assert_eq!(audio_mime_in_bytes(b""), None);
        assert_eq!(audio_mime_in_bytes(b"Ogg"), None);
        assert_eq!(audio_mime_in_bytes(b"just some text"), None);
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
    fn accept_lists_audio_extensions_too() {
        let accepted = UploadConfiguration::TrackAudio.get_details().mime_types;
        assert_eq!(
            accept_extensions(&accepted),
            ".mp3,.MP3,.wav,.WAV,.flac,.FLAC,.ogg,.OGG"
        );
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
