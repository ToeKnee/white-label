//! Upload progress totals, keyed by `"<username>-<filename>"`.
//!
//! In theory one request could carry an upload *and* answer its progress. In
//! practice no browser duplexes a request that way: it will not start reading a
//! response until the request body has been sent, and Firefox does not even
//! resolve a stream response that has no bytes in it yet. So a widget that
//! wants a progress bar runs two requests side by side:
//!
//! 1. `POST` the file, whose handler counts each chunk into this registry, and
//! 2. `GET` the totals of that same upload as a stream that stays open until
//!    the upload ends.
//!
//! An entry belongs to the upload: [`add_chunk`] creates it with the upload's
//! first chunk and its totals are broadcast as they arrive; the entry goes away
//! when the upload ends (`services::files::upload::finalise_upload`, or the
//! handler undoing a rejected upload). A totals watcher deliberately never
//! creates an entry: an entry with no upload behind it holds a live broadcast
//! sender that never closes, so its watcher would stream forever and the entry
//! would outlive the request that made it.

use async_broadcast::{Receiver, Sender, broadcast};
use dashmap::DashMap;
use futures::Stream;
use std::sync::LazyLock;
use std::time::Duration;

/// How long a totals stream waits for its upload to arrive before giving up.
/// The upload is sent at the same moment the totals request is, so this only
/// has to cover the network and the handler's own permission checks.
const REGISTRATION_WAIT: Duration = Duration::from_secs(15);
/// How long a registered upload may go without a chunk before its totals
/// stream stops. An upload large enough to be worth a progress bar sends a
/// chunk far more often than this; the stream also ends when the upload
/// finishes, which is the usual case.
const CHUNK_WAIT: Duration = Duration::from_secs(30);
/// How often a waiting totals stream looks for the entry it is watching.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// This represents the state of a file upload.
pub struct File {
    total: usize,
    tx: Sender<usize>,
    rx: Receiver<usize>,
}

/// This is a global map of all file uploads.
pub static FILES: LazyLock<DashMap<String, File>> = LazyLock::new(DashMap::new);

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
    }

    new_total
}

/// The running totals of one upload, one per line as `usize` values: the total
/// so far each time a chunk arrives.
///
/// The stream is `Send` because the dioxus upload endpoints wrap it in
/// `Streaming`, which boxes a `dyn Stream + Send`.
pub fn progress_for_file(
    filename: &str,
    username: &str,
) -> impl Stream<Item = usize> + Send + use<> {
    totals_stream(
        format!("{username}-{filename}"),
        REGISTRATION_WAIT,
        CHUNK_WAIT,
    )
}

/// [`progress_for_file`]'s stream, with the waits exposed so the tests can run
/// quickly.
///
/// The stream ends by itself in every case, which is what keeps a client that
/// walks away from a stalled upload from holding a request open:
/// - the upload finishes (its entry is removed, which drops the broadcast
///   sender this stream is reading from),
/// - the upload never arrives, after `registration_wait`,
/// - the upload stalls, after `chunk_wait` with no chunk.
///
/// Totals recorded before this stream subscribed are still delivered: the
/// broadcast channel replays them, so the order the two requests arrive in does
/// not matter.
fn totals_stream(
    key: String,
    registration_wait: Duration,
    chunk_wait: Duration,
) -> impl Stream<Item = usize> + Send + use<> {
    futures::stream::unfold(
        TotalsWatcher {
            key,
            receiver: None,
            waited: Duration::ZERO,
            registration_wait,
            chunk_wait,
        },
        |mut watcher| async move {
            loop {
                // The receiver is taken out of the watcher for the wait, so no
                // borrow of `watcher` reaches across the `await` and the state
                // can be handed back to `unfold` below.
                if let Some(mut receiver) = watcher.receiver.take() {
                    let received = tokio::time::timeout(watcher.chunk_wait, receiver.recv()).await;
                    watcher.receiver = Some(receiver);
                    match received {
                        Ok(Ok(total)) => return Some((total, watcher)),
                        // The upload ended and took the sender with it.
                        Ok(Err(closed)) => {
                            tracing::debug!("[{}]\ttotals ended: {closed}", watcher.key);
                            return None;
                        }
                        Err(_stalled) => {
                            tracing::debug!(
                                "[{}]\tno chunk for {:?}; stopping the totals stream",
                                watcher.key,
                                watcher.chunk_wait
                            );
                            return None;
                        }
                    }
                }
                // The upload registers itself with its first chunk. Cloning the
                // receiver needs no lock held over the awaits below.
                if let Some(entry) = FILES.get(&watcher.key) {
                    watcher.receiver = Some(entry.rx.clone());
                    continue;
                }
                if watcher.waited >= watcher.registration_wait {
                    tracing::debug!(
                        "[{}]\tno upload arrived within {:?}; the totals stream has \
                         nothing to watch",
                        watcher.key,
                        watcher.registration_wait
                    );
                    return None;
                }
                tokio::time::sleep(POLL_INTERVAL).await;
                watcher.waited += POLL_INTERVAL;
            }
        },
    )
}

/// Where [`totals_stream`] is up to: watching nothing, or watching a broadcast
/// receiver it found for its key.
struct TotalsWatcher {
    key: String,
    receiver: Option<Receiver<usize>>,
    waited: Duration,
    registration_wait: Duration,
    chunk_wait: Duration,
}

#[cfg(test)]
mod tests {
    use futures::StreamExt as _;

    use super::*;

    /// A current-thread runtime: the totals stream needs tokio's timer, and
    /// these tests have no database to hand (which is what `#[sqlx::test]`
    /// supplies elsewhere).
    fn run(test: impl Future<Output = ()>) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("tokio runtime for the progress registry");
        runtime.block_on(test);
    }

    #[test]
    fn totals_stop_when_no_upload_arrives() {
        run(async {
            let totals = totals_stream(
                "nobody-never.png".to_string(),
                Duration::from_millis(100),
                Duration::from_secs(30),
            );
            assert_eq!(totals.collect::<Vec<usize>>().await, Vec::<usize>::new());
            assert!(
                !FILES.contains_key("nobody-never.png"),
                "watching an upload must not create an entry for it"
            );
        });
    }

    /// The totals request usually arrives first, and the upload's own chunks
    /// are replayed to it as soon as it registers.
    #[test]
    fn totals_reach_a_watcher_that_arrived_before_the_upload() {
        run(async {
            let filename = "late.png";
            let username = "watcher";
            let key = format!("{username}-{filename}");
            let totals = totals_stream(
                key.clone(),
                Duration::from_secs(15),
                Duration::from_secs(30),
            );

            let publisher = async {
                // Long enough for the watcher's first polls to miss, then long
                // enough for one of them to find the entry again.
                tokio::time::sleep(Duration::from_millis(120)).await;
                add_chunk(filename, 4, username).await;
                add_chunk(filename, 3, username).await;
                tokio::time::sleep(Duration::from_millis(300)).await;
                // What `finalise_upload` does when the upload finishes.
                FILES.remove(&key);
            };
            let (collected, ()) = futures::join!(totals.collect::<Vec<usize>>(), publisher);
            assert_eq!(collected, vec![4, 7], "both totals arrive replayed");
        });
    }

    /// An upload that stops sending cannot hold a totals stream open for good.
    #[test]
    fn totals_stop_when_an_upload_stalls() {
        run(async {
            let filename = "stalled.png";
            let username = "staller";
            let key = format!("{username}-{filename}");
            add_chunk(filename, 9, username).await;

            let totals = totals_stream(
                key.clone(),
                Duration::from_secs(15),
                Duration::from_millis(50),
            );
            let collected = totals.collect::<Vec<usize>>().await;

            assert_eq!(collected, vec![9], "the last total still arrives");
            // Cleaning up after the stalled upload is its handler's job.
            FILES.remove(&key);
        });
    }
}
