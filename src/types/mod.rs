use super::screens::*;
use crate::http::comment::Comment;
use crate::http::danmu::Danmu;
use crate::http::notify::Notify;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc::Sender, Mutex};
use tracing::error;

#[derive(Debug, Clone)]
pub enum Message {
    ChannelConnected(Sender<ChannelMsg>),
    AicuFetchingState { now: f32, max: f32 },
    OfficialFetchingState(String),
    RefreshUI(()),

    Cookie(cookie::Message),
    QRCode(qrcode::Message),
    Main(main::Message),
}

pub enum ChannelMsg {
    DeleteComment(
        Arc<Client>,
        Arc<String>,
        Arc<Mutex<HashMap<u64, Comment>>>,
        f32,
    ),
    StopDeleteComment,
    DeleteNotify(
        Arc<Client>,
        Arc<String>,
        Arc<Mutex<HashMap<u64, Notify>>>,
        f32,
    ),
    StopDeleteNotify,
    DeleteDanmu(
        Arc<Client>,
        Arc<String>,
        Arc<Mutex<HashMap<u64, Danmu>>>,
        f32,
    ),
    StopDeleteDanmu,
    StartRefreshQRcodeState,
    StopRefreshQRcodeState,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("request failed: {0}")]
    RequestFailed(Arc<reqwest::Error>),
    #[error("Error: {0}")]
    Other(&'static str),
    #[error("Error: {0}")]
    OtherString(Arc<String>),
}
impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Self::RequestFailed(Arc::new(error))
    }
}

impl From<&'static str> for Error {
    fn from(error: &'static str) -> Self {
        Self::Other(error)
    }
}

impl From<String> for Error {
    fn from(error: String) -> Self {
        Self::OtherString(Arc::new(error))
    }
}
