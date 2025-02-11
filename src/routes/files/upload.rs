use leptos::prelude::*;
use server_fn::codec::{MultipartData, MultipartFormData, StreamingText, TextStream};
#[cfg(feature = "ssr")]
use std::{fs::File, io::Write};

// In theory, you could create a single server function which
/// 1) received multipart form data
/// 2) returned a stream that contained updates on the progress
///
/// In reality, browsers do not actually support duplexing requests in this way. In other
/// words, every existing browser actually requires that the request stream be complete before
/// it begins processing the response stream.
///
/// Instead, we can create two separate server functions:
/// 1) one that receives multipart form data and begins processing the upload
/// 2) a second that returns a stream of updates on the progress
///
/// This requires us to store some global state of all the uploads. In a real app, you probably
/// shouldn't do exactly what I'm doing here in the demo. For example, this map just
/// distinguishes between files by filename, not by user.
#[cfg(feature = "ssr")]
pub mod progress {
    use async_broadcast::{broadcast, Receiver, Sender};
    use dashmap::DashMap;
    use futures::Stream;
    use once_cell::sync::Lazy;

    struct File {
        total: usize,
        tx: Sender<usize>,
        rx: Receiver<usize>,
    }

    static FILES: Lazy<DashMap<String, File>> = Lazy::new(DashMap::new);

    /// Add a chunk to the progress of a file upload.
    /// This function will update the total length of the file and send a message to the stream.
    /// The message will be the new total length of the file.
    pub async fn add_chunk(filename: &str, len: usize) {
        leptos::logging::log!("[{filename}]\tadding chunk {len}");
        let mut entry = FILES.entry(filename.to_string()).or_insert_with(|| {
            leptos::logging::log!("[{filename}]\tInserting channel");
            let (tx, rx) = broadcast(4096);
            File { total: 0, tx, rx }
        });
        entry.total += len;
        let new_total = entry.total;

        // We're about to do an async broadcast, so we don't want to hold a lock across it
        let tx = entry.tx.clone();
        drop(entry);

        // now we send the message and don't have to worry about it
        match tx.broadcast(new_total).await {
            Ok(_) => {}
            Err(e) => {
                leptos::logging::error!("[{filename}]\tCouldn't send a message over channel: {e}");
            }
        };
    }

    /// Get the progress of a file upload.
    ///
    /// This function will return a stream of the current length of the file.
    /// The stream will be a series of `usize` values.
    /// Each value will be the total length of the file at that point in time.
    pub fn for_file(filename: &str) -> impl Stream<Item = usize> {
        let entry = FILES.entry(filename.to_string()).or_insert_with(|| {
            leptos::logging::log!("[{filename}]\tinserting channel");
            // This limits the amount of chunks we can store in the channel
            // If we don't limit it, we could potentially run out of memory
            // if the client is sending data very slowly
            // Each block is ~0.5MB, so this is a 2GB limit
            let (tx, rx) = broadcast(4096);
            File { total: 0, tx, rx }
        });
        entry.rx.clone()
    }
}

/// Upload a file.
/// This function will receive a multipart form data request and save the file to disk.
#[server(input = MultipartFormData,)]
pub async fn upload_file(data: MultipartData) -> Result<(), ServerFnError> {
    let Some(mut data) = data.into_inner() else {
        return Err(ServerFnError::new("No data in request.".to_string()));
    };

    let mut more = true;
    while more {
        match data.next_field().await {
            Ok(None) => {
                more = false;
            }
            Ok(Some(mut field)) => {
                let name = match field.file_name() {
                    Some(name) => name.to_string(),
                    None => {
                        return Err(ServerFnError::new("No filename on field.".to_string()));
                    }
                };

                // TODO: this is a hardcoded path, pull it from an environment variable
                let path = format!("/home/anthony/Projects/Rust/white-label/uploads/{name}");
                let mut f = File::create(path)?;
                let mut chunk_more = true;
                while chunk_more {
                    match field.chunk().await {
                        Ok(None) => {
                            chunk_more = false;
                        }

                        Ok(Some(chunk)) => {
                            let len = chunk.len();
                            progress::add_chunk(&name, len).await;
                            f.write_all(&chunk)?;
                        }
                        Err(e) => {
                            return Err(ServerFnError::new(e.to_string()));
                        }
                    }
                }
            }
            Err(e) => {
                return Err(ServerFnError::new(e.to_string()));
            }
        }
    }

    Ok(())
}

/// Get the progress of a file upload.
/// This function will return a stream of the current length of the file.
/// The stream will be a series of `usize` values, each separated by a newline.
#[allow(clippy::unused_async)] // Although this function doesn't use `async`, it's required for the `#[server]` macro
#[server(output = StreamingText)]
pub async fn file_progress(filename: String) -> Result<TextStream, ServerFnError> {
    use futures::StreamExt;
    leptos::logging::log!("Getting progress on {filename}");
    // Get the stream of current length for the file
    let progress = progress::for_file(&filename);
    // Separate each number with a newline
    // the HTTP response might pack multiple lines of this into a single chunk
    // we need some way of dividing them up
    let progress = progress.map(|bytes| Ok(format!("{bytes}\n")));
    Ok(TextStream::new(progress))
}
