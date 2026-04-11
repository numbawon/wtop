use thiserror::Error;

#[derive(Error, Debug)]
pub enum WtopError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Windows API error: {0}")]
    Windows(#[from] windows::core::Error),

    #[error("Terminal error: {0}")]
    Terminal(String),

    #[error("Collector error: {0}")]
    Collector(String),
}
