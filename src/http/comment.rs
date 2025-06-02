pub mod aicu;
pub mod official;

use super::api_service::ApiService;
use crate::cvmsg;
use crate::http::notify::Notify;
use crate::types::{Error, Message, RemoveAble, Result};
use iced::Task;
use serde_json::Value;
use std::collections::HashMap;
use std::mem;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::try_join;

#[derive(Debug, Default, Clone)]
pub struct Comment {
    pub oid: u64,
    pub r#type: u8,
    pub content: String,
    pub is_selected: bool,
    pub notify_id: Option<u64>,
    /// 删除通知用 0为收到赞的 1为收到评论的 2为被At的
    pub tp: Option<u8>,
}
impl Comment {
    fn new(oid: u64, r#type: u8, content: String) -> Comment {
        Comment {
            oid,
            r#type,
            content,
            is_selected: true,
            notify_id: None,
            tp: None,
        }
    }
    pub fn new_with_notify(
        oid: u64,
        r#type: u8,
        content: String,
        notify_id: u64,
        tp: u8,
    ) -> Comment {
        Comment {
            oid,
            r#type,
            content,
            is_selected: true,
            notify_id: Some(notify_id),
            tp: Some(tp),
        }
    }
}
impl RemoveAble for Comment {
    async fn remove(&self, rpid: u64, api: Arc<ApiService>) -> Result<u64> {
        let json_res: Value = if self.r#type == 11 {
            let form_data = [
                ("oid", self.oid.to_string()),
                ("type", self.r#type.to_string()),
                ("rpid", rpid.to_string()),
            ];
            api.post_form(
                format!(
                    "https://api.bilibili.com/x/v2/reply/del?csrf={}",
                    api.csrf()
                ),
                &form_data,
            )
            .await?
            .json()
            .await?
        } else {
            let form_data = [
                ("oid", self.oid.to_string()),
                ("type", self.r#type.to_string()),
                ("rpid", rpid.to_string()),
                ("csrf", api.csrf().to_string()),
            ];
            api.post_form("https://api.bilibili.com/x/v2/reply/del", &form_data)
                .await?
                .json()
                .await?
        };
        if json_res["code"].as_i64().unwrap() == 0 {
            // 如果is_some则删除通知
            if let Some(notify_id) = self.notify_id {
                Notify::new(String::new(), self.tp.unwrap())
                    .remove(notify_id, api.clone())
                    .await?;
            }
            Ok(rpid)
        } else {
            Err(Error::DeleteCommentError(json_res.into()))
        }
    }
}
