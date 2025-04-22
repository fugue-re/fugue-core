use thiserror::Error;

#[derive(Debug, Error)]
pub enum LifterGeneratorError {
    #[error(transparent)]
    Language(anyhow::Error),
}

impl LifterGeneratorError {
    pub fn language<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Language(anyhow::Error::new(e))
    }

    pub fn language_with<M>(m: M) -> Self
    where
        M: std::fmt::Debug + std::fmt::Display + Send + Sync + 'static,
    {
        Self::Language(anyhow::Error::msg(m))
    }
}
