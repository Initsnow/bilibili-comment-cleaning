use bilibili_comment_cleaning::get_json;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{mpsc::Sender, Mutex};

#[derive(Debug, Clone)]
pub enum Message {
    CookieSubmited(String),
    CookieInputChanged(String),
    ClientCreated { client: Client, csrf: String },
    CommentsFetched(Arc<Mutex<Vec<Comment>>>),
    ChangeCommentRemoveState(u64, bool),
    CommentsSelectAll,
    CommentsDeselectAll,
    DeleteComment,
    CommentDeleted { rpid: u64 },
    ChannelConnected(Sender<ChannelMsg>),
    AicuToggle(bool),
    QRcodeGot(QRcode),
    QRcodeRefresh,
    QRcodeState((u64, Option<std::string::String>)),
    EntertoCookieInput,
    EntertoQRcodeScan,
    SecondsInputChanged(String),
    StopDeleteComment,
    AicuFetchingState { now: f32, max: f32 },
    OfficialFetchingState(String),
    AllCommentDeleted,
    RefreshUI(()),
}

#[derive(Debug, Default, Clone)]
pub struct Comment {
    pub oid: u64,
    pub r#type: u8,
    pub rpid: u64,
    pub content: String,
    pub remove_state: bool,
    pub notify_id: Option<u64>,
    /// 删除通知用 0为收到赞的 1为收到评论的 2为被At的
    pub tp: Option<u8>,
}

pub enum ChannelMsg {
    DeleteComment(Arc<Client>, Arc<String>, Arc<Mutex<Vec<Comment>>>, f32),
    StopDelete,
    StartRefreshQRcodeState,
    StopRefreshQRcodeState,
}

#[derive(Debug, Clone)]
pub struct QRcode {
    pub url: String,
    pub key: String,
}
impl QRcode {
    pub async fn request_qrcode() -> QRcode {
        let a = get_json(
            Arc::new(Client::new()),
            "https://passport.bilibili.com/x/passport-login/web/qrcode/generate",
        )
        .await;
        QRcode {
            url: a["data"]["url"].as_str().unwrap().to_string(),
            key: a["data"]["qrcode_key"].as_str().unwrap().to_string(),
        }
    }
    pub async fn get_state(&self, cl: Arc<Client>) -> (u64, Option<String>) {
        let url = format!(
            "https://passport.bilibili.com/x/passport-login/web/qrcode/poll?qrcode_key={}",
            &self.key
        );
        let res = get_json(cl, &url).await;
        let res_code = res["data"]["code"].as_u64().unwrap();
        if res_code == 0 {
            let res_url = res["data"]["url"].as_str().unwrap();
            let a = res_url.find("bili_jct=").expect("Can't find csrf data.");
            let b = res_url[a..].find("&").unwrap();
            let csrf = res_url[a + 9..b + a].to_string();
            return (res_code, Some(csrf));
        }
        (res_code, None)
    }
}
