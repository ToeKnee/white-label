/// In theory, you could create a single server function which
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
use async_broadcast::{Receiver, Sender, broadcast};
use dashmap::DashMap;
use futures::Stream;
use once_cell::sync::Lazy;

pub struct File {
    total: usize,
    tx: Sender<usize>,
    rx: Receiver<usize>,
}

pub static FILES: Lazy<DashMap<String, File>> = Lazy::new(DashMap::new);

/// Add a chunk to the progress of a file upload.
/// This function will update the total length of the file and send a message to the stream.
/// The message will be the new total length of the file.
pub async fn add_chunk(filename: &str, len: usize, username: &str) -> usize {
    tracing::debug!("[{filename}]\tadding chunk {len}");
    let id = format!("{username}-{filename}");
    let mut entry = FILES.entry(id).or_insert_with(|| {
        tracing::debug!("[{filename}]\tInserting channel");
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
            tracing::error!("[{filename}]\tCouldn't send a message over channel: {e}");
        }
    };

    new_total
}

/// Get the progress of a file upload.
///
/// This function will return a stream of the current length of the file.
/// The stream will be a series of `usize` values.
/// Each value will be the total length of the file at that point in time.
pub fn progress_for_file(filename: &str, username: &str) -> impl Stream<Item = usize> + use<> {
    let id = format!("{username}-{filename}");
    let entry = FILES.entry(id).or_insert_with(|| {
        tracing::debug!("[{}]\tInserting channel.", filename.to_string());
        // This limits the amount of chunks we can store in the channel
        // If we don't limit it, we could potentially run out of memory
        // if the client is sending data very slowly
        // Each block is ~0.5MB, so this is a 2GB limit
        let (tx, rx) = broadcast(4096);
        File { total: 0, tx, rx }
    });

    entry.rx.clone()
}
