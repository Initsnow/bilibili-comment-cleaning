use super::api_service::ApiService;
use crate::http::notify::Notify;
use crate::types::{Error, RemoveAble, Result};
use serde_json::Value;
use std::sync::Arc;

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
    pub fn new_with_notify(content: String, cid: u64, notify_id: u64) -> Danmu {
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
        let json_res: Value = api
            .post_form("https://api.bilibili.com/x/msgfeed/del", &form_data)
            .await?
            .error_for_status()?
            .json()
            .await?;
        if json_res["code"].as_i64().unwrap() == 0 {
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
