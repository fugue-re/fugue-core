//! channel error types
//! 

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0} channel emit error: {1}")]
    Emit(String, String),
    #[error("{0} channel recv error: {1}")]
    Recv(String, String),
}

impl Error {
    pub fn emit(chnl_type: &str, err: impl std::fmt::Debug) -> Self {
        Self::Emit(String::from(chnl_type), format!("{:?}", err))
    }

    pub fn recv(chnl_type: &str, err: impl std::fmt::Debug) -> Self {
        Self::Recv(String::from(chnl_type), format!("{:?}", err))
    }
}
