use super::screens::*;
use crate::http::comment::Comment;
use crate::http::danmu::Danmu;
use crate::http::notify::Notify;
use crate::screens::main;
use crate::screens::main::comment_viewer::CvMsg;
use crate::screens::main::danmu_viewer::DvMsg;
use crate::screens::main::notify_viewer::NvMsg;
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
        Arc<super::http::api_service::ApiService>,
        Arc<Mutex<HashMap<u64, Comment>>>,
        f32,
    ),
    StopDeleteComment,
    DeleteNotify(
        Arc<super::http::api_service::ApiService>,
        Arc<Mutex<HashMap<u64, Notify>>>,
        f32,
    ),
    StopDeleteNotify,
    DeleteDanmu(
        Arc<super::http::api_service::ApiService>,
        Arc<Mutex<HashMap<u64, Danmu>>>,
        f32,
    ),
    StopDeleteDanmu,
}

pub trait RemoveAble {
    fn remove(
        &self,
        id: u64,
        api: Arc<super::http::api_service::ApiService>,
    ) -> impl std::future::Future<Output = Result<u64>> + Send;
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
    #[error("Failed to delete comment, Response json: {0}")]
    DeleteCommentError(Arc<serde_json::Value>),
    #[error("Failed to delete danmu, Response json: {0}")]
    DeleteDanmuError(Arc<serde_json::Value>),
    #[error("Failed to delete notify, Response json: {0}")]
    DeleteNotifyError(Arc<serde_json::Value>),
    #[error("Failed to delete system notify, Response json: {0}")]
    DeleteSystemNotifyError(Arc<serde_json::Value>),
    #[error("Failed to create api service, cookie didn't contain bili_jct")]
    CreateApiServiceError,
}
impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Self::RequestFailed(Arc::new(error))
    }
}

impl From<ParseIntError> for Error {
    fn from(error: ParseIntError) -> Self {
        Self::ParseIntError(Arc::new(error))
    }
}

#[derive(Debug, Clone)]
pub struct LikedRecovery {
    pub cursor_id: u64,
    pub cursor_time: u64,
}

#[derive(Debug, Clone)]
pub struct ReplyedRecovery {
    pub cursor_id: u64,
    pub cursor_time: u64,
}

#[derive(Debug, Clone)]
pub struct AtedRecovery {
    pub cursor_id: u64,
    pub cursor_time: u64,
}

#[derive(Debug, Clone)]
pub struct SystemNotifyRecovery {
    pub cursor: u64,
    pub api_type: u8, // 0 or 1
}

#[derive(Debug, Clone)]
pub struct AicuCommentRecovery {
    pub uid: u64,
    pub page: u32,
    pub all_count: u64,
}

#[derive(Debug, Clone)]
pub struct AicuDanmuRecovery {
    pub uid: u64,
    pub page: u32,
    pub all_count: u64,
}

#[derive(Default, Debug, Clone)]
pub struct FetchProgressState {
    pub liked_data: (
        HashMap<u64, Notify>,
        HashMap<u64, Comment>,
        HashMap<u64, Danmu>,
    ),
    pub liked_recovery: Option<LikedRecovery>,

    pub replyed_data: (HashMap<u64, Notify>, HashMap<u64, Comment>),
    pub replyed_recovery: Option<ReplyedRecovery>,

    pub ated_data: HashMap<u64, Notify>,
    pub ated_recovery: Option<AtedRecovery>,

    pub system_notify_data: HashMap<u64, Notify>,
    pub system_notify_recovery: Option<SystemNotifyRecovery>,

    pub aicu_comment_data: HashMap<u64, Comment>,
    pub aicu_comment_recovery: Option<AicuCommentRecovery>,

    pub aicu_danmu_data: HashMap<u64, Danmu>,
    pub aicu_danmu_recovery: Option<AicuDanmuRecovery>,

    pub aicu_enabled_last_run: bool,
}
