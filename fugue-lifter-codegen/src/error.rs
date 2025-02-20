use thiserror::Error;

#[derive(Debug, Error)]
pub enum LifterGeneratorError {
    #[error(transparent)]
    Translator(anyhow::Error),
}

impl LifterGeneratorError {
    pub fn translator<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Translator(anyhow::Error::new(e))
    }

    pub fn translator_with<M>(m: M) -> Self
    where
        M: std::fmt::Debug + std::fmt::Display + Send + Sync + 'static,
    {
        Self::Translator(anyhow::Error::msg(m))
    }
}
