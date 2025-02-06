use super::screens::*;
use crate::http::comment::Comment;
use crate::http::danmu::Danmu;
use crate::http::notify::Notify;
use crate::screens::main;
use crate::screens::main::comment_viewer::CvMsg;
use crate::screens::main::danmu_viewer::DvMsg;
use crate::screens::main::notify_viewer::NvMsg;
use reqwest::Client;
use std::collections::HashMap;
use std::num::ParseIntError;
use std::sync::Arc;
use tokio::sync::{mpsc::Sender, Mutex};
use tracing::error;

#[derive(Debug, Clone)]
pub enum Message {
    ChannelConnected(Sender<ChannelMsg>),
    RefreshUI(()),

    Cookie(cookie::Message),
    QRCode(qrcode::Message),
    Main(main::Message),
}

impl From<CvMsg> for Message {
    fn from(value: CvMsg) -> Self {
        Message::Main(main::Message::CommentMsg(value))
    }
}
impl From<NvMsg> for Message {
    fn from(value: NvMsg) -> Self {
        Message::Main(main::Message::NotifyMsg(value))
    }
}
impl From<DvMsg> for Message {
    fn from(value: DvMsg) -> Self {
        Message::Main(main::Message::DanmuMsg(value))
    }
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

pub trait RemoveAble {
    async fn remove(&self, id: u64, cl: Arc<Client>, csrf: Arc<String>) -> Result<u64>;
}

pub type Result<T> = std::result::Result<T, Error>;

// ELM 架构所以 Arc, IDK🤣
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("request failed: {0}")]
    RequestFailed(Arc<reqwest::Error>),
    #[error("Failed to parse: {0}")]
    ParseIntError(Arc<ParseIntError>),
    #[error("Unrecognized URI: {0}")]
    UnrecognizedURI(Arc<String>),
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
impl From<ParseIntError> for Error {
    fn from(error: ParseIntError) -> Self {
        Self::ParseIntError(Arc::new(error))
    }
}
