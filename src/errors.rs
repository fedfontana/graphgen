use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScraperError {
    #[error("Could not fetch data: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("Could not find any content in the page with url {0}")]
    NoContentFound(String),

    #[error("Could not send data to internal channel")]
    ChannelError(#[from] crossbeam_channel::SendError<(String, u64)>),

    #[error("Could not read response: {0}")]
    ReadError(#[from] std::io::Error),
}