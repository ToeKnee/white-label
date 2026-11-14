//! Models module

pub mod artist;
pub mod auth;
pub mod music_service;
pub mod page;
pub mod record_label;
pub mod release;
pub mod social_media;
// Test data factories talk to Postgres through sqlx, so they exist only
// where sqlx does: the `server` feature's test builds. Kept as two `cfg`
// attributes rather than `all(test, feature = "server")` so tooling
// recognises the test module (same shape as the server-gated test modules
// in `src/ui`).
#[cfg(test)]
#[cfg(feature = "server")]
pub mod test_helpers;
pub mod track;
pub mod track_play_stats;
pub mod track_with_artists;
pub mod traits;
pub mod visibility;

pub use visibility::Visibility;
