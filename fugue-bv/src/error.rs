use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("invalid bit-vector format")]
    InvalidFormat,
    #[error("invalid bit-vector size")]
    InvalidSize,
    #[error("invalid bit-vector constant")]
    InvalidConst,
}
