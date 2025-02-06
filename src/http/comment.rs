pub mod aicu;
pub mod official;

use crate::cvmsg;
use crate::http::notify::Notify;
use crate::screens::main;
use crate::types::{Message, RemoveAble, Result};
use iced::Task;
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::mem;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::try_join;
use tracing::error;

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
    fn new_with_notify(oid: u64, r#type: u8, content: String, notify_id: u64, tp: u8) -> Comment {
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
    async fn remove(&self, rpid: u64, cl: Arc<Client>, csrf: Arc<String>) -> Result<u64> {
        let json_res: Value = if self.r#type == 11 {
            cl.post(format!(
                "https://api.bilibili.com/x/v2/reply/del?csrf={}",
                csrf.clone()
            ))
            .form(&[
                ("oid", self.oid.to_string()),
                ("type", self.r#type.to_string()),
                ("rpid", rpid.to_string()),
            ])
            .send()
            .await?
            .json()
            .await?
        } else {
            cl.post("https://api.bilibili.com/x/v2/reply/del")
                .form(&[
                    ("oid", self.oid.to_string()),
                    ("type", self.r#type.to_string()),
                    ("rpid", rpid.to_string()),
                    ("csrf", csrf.to_string()),
                ])
                .send()
                .await?
                .json()
                .await?
        };
        if json_res["code"].as_i64().unwrap() == 0 {
            // 如果is_some则删除通知
            if let Some(notify_id) = self.notify_id {
                Notify::new(String::new(), self.tp.unwrap())
                    .remove(notify_id, cl, csrf)
                    .await?;
            }
            Ok(rpid)
        } else {
            let e = format!("Can't remove comment. Response json: {}", json_res);
            error!("{:?}",e);
            Err(e.into())
        }
    }
}

pub async fn fetch_both(cl: Arc<Client>) -> Result<Arc<Mutex<HashMap<u64, Comment>>>> {
    let (m1, m2) = try_join!(official::fetch(cl.clone()), aicu::fetch(cl.clone()))?;

    let (m1, m2) = {
        let mut lock1 = m1.lock().await;
        let mut lock2 = m2.lock().await;
        (mem::take(&mut *lock1), mem::take(&mut *lock2))
    };

    Ok(Arc::new(Mutex::new(m1.into_iter().chain(m2).collect())))
}

pub fn fetch_via_aicu_state(cl: Arc<Client>, aicu_state: bool) -> Task<Message> {
    if aicu_state {
        Task::perform(fetch_both(cl), |e| {
            Message::Main(main::Message::CommentMsg(cvmsg::CommentsFetched(e)))
        })
    } else {
        Task::perform(official::fetch(cl), |e| {
            Message::Main(main::Message::CommentMsg(cvmsg::CommentsFetched(e)))
        })
    }
}
