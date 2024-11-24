use crate::get_json;
use crate::notify::remove_notify;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{mpsc::Sender, Mutex};
use tracing::error;

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
    QRcodeState((u64, Option<String>)),
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
impl Comment {
    pub(crate) async fn remove(&self, cl: Arc<Client>, csrf: Arc<String>) -> Result<u64, String> {
        let res = if self.r#type == 11 {
            cl.post(format!(
                "https://api.bilibili.com/x/v2/reply/del?csrf={}",
                csrf.clone()
            ))
            .form(&[
                ("oid", self.oid.to_string()),
                ("type", self.r#type.to_string()),
                ("rpid", self.rpid.to_string()),
            ])
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
        } else {
            cl.post("https://api.bilibili.com/x/v2/reply/del")
                .form(&[
                    ("oid", self.oid.to_string()),
                    ("type", self.r#type.to_string()),
                    ("rpid", self.rpid.to_string()),
                    ("csrf", csrf.to_string()),
                ])
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        };
        let json_res: serde_json::Value = serde_json::from_str(res.as_str()).unwrap();
        if json_res["code"].as_i64().unwrap() == 0 {
            // 如果is_some则删除通知
            if let Some(notify_id) = self.notify_id {
                remove_notify(cl, notify_id, csrf, self.tp.unwrap().to_string()).await;
            }
            Ok(self.rpid)
        } else {
            error!("Can't remove comment. Response json: {}", json_res);
            Err(format!("Can't remove comment. Response json: {}", json_res))
        }
    }
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
