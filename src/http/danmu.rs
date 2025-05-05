use crate::dvmsg;
use crate::http::notify::Notify;
use crate::screens::main;
use crate::types::{Error, Message, RemoveAble, Result};
use iced::Task;
use serde_json::Value;
use std::collections::HashMap;
use std::mem;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::try_join;
use super::api_service::ApiService;

pub mod aicu;
pub mod official;

#[derive(Clone, Debug)]
pub struct Danmu {
    pub content: String,
    cid: u64,
    // r#type: u8,
    pub is_selected: bool,
    pub notify_id: Option<u64>,
}
impl Danmu {
    fn new(content: String, cid: u64) -> Danmu {
        Danmu {
            content,
            cid,
            is_selected: true,
            notify_id: None,
        }
    }
    fn new_with_notify(content: String, cid: u64, notify_id: u64) -> Danmu {
        Danmu {
            content,
            cid,
            is_selected: true,
            notify_id: Some(notify_id),
        }
    }
}

impl RemoveAble for Danmu {
    async fn remove(&self, dmid: u64, api: Arc<ApiService>) -> Result<u64> {
        let form_data = [
            ("dmid", dmid.to_string()),
            ("cid", self.cid.to_string()),
            ("type", 1.to_string()),
            ("csrf", api.csrf().to_string()),
        ];
        let json_res: Value = api.post_form("https://api.bilibili.com/x/msgfeed/del", &form_data)
            .await?
            .error_for_status()?
            .json()
            .await?;
        if json_res["code"]
            .as_i64()
            .unwrap()
            == 0
        {
            if let Some(notify_id) = self.notify_id {
                Notify::new(String::new(), 0)
                    .remove(notify_id, api.clone())
                    .await?;
            }
            Ok(dmid)
        } else {
            Err(Error::DeleteDanmuError(json_res.into()))
        }
    }
}

async fn fetch_both(api: Arc<ApiService>) -> Result<Arc<Mutex<HashMap<u64, Danmu>>>> {
    let (m1, m2) = try_join!(official::fetch(api.clone()), aicu::fetch(api.clone()))?;

    let (m1, m2) = {
        let mut lock1 = m1.lock().await;
        let mut lock2 = m2.lock().await;
        (mem::take(&mut *lock1), mem::take(&mut *lock2))
    };

    Ok(Arc::new(Mutex::new(m1.into_iter().chain(m2).collect())))
}

pub fn fetch_via_aicu_state(api: Arc<ApiService>, aicu_state: bool) -> Task<Message> {
    if aicu_state {
        Task::perform(fetch_both(api), |e| {
            Message::Main(main::Message::DanmuMsg(dvmsg::DanmusFetched(e)))
        })
    } else {
        Task::perform(official::fetch(api), |e| {
            Message::Main(main::Message::DanmuMsg(dvmsg::DanmusFetched(e)))
        })
    }
}
