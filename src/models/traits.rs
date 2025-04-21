#[cfg(feature = "ssr")]
use sqlx::PgPool;

/// Trait for validating models
pub trait Validate {
    /// Validates the model
    ///
    /// # Arguments
    ///
    /// * `pool` - The database connection pool
    ///
    /// # Returns
    ///
    /// A future that resolves to a result
    ///   - Ok(()) if the model is valid
    ///   - Err if the model is invalid
    #[cfg(feature = "ssr")]
    fn validate(&self, pool: &PgPool) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;
}
