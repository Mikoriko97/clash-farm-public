use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChainError {
    #[error("rpc transport error: {0}")]
    Transport(String),
    #[error("rpc response too large")]
    TooLarge,
    #[error("rpc returned error {code}: {message}")]
    Rpc { code: i64, message: String },
    #[error("rpc response did not match the expected shape: {0}")]
    Shape(String),
    #[error("both rpc providers failed")]
    AllProvidersFailed,
}
