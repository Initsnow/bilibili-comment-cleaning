use crate::types::Result;
use reqwest::Client;
use serde_json::Value;
use std::sync::Arc;
use tracing::{error, info};

pub mod official;

#[derive(Clone, Debug)]
pub struct Danmu {
    pub content: String,
    cid: u64,
    // r#type: u8,
    pub is_selected: bool,
}
impl Danmu {
    fn new(content: String, cid: u64) -> Danmu {
        Danmu {
            content,
            cid,
            is_selected: true,
        }
    }

    pub async fn remove(&self, dmid: u64, cl: Arc<Client>, csrf: Arc<String>) -> Result<u64> {
        let json_res: Value = cl
            .post(
                "
    https://api.bilibili.com/x/msgfeed/del",
            )
            .form(&[
                ("dmid", dmid.to_string()),
                ("cid", self.cid.to_string()),
                ("type", 1.to_string()),
                ("csrf", csrf.to_string()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if json_res["code"]
            .as_i64()
            .ok_or("Remove Danmu: Parse json res code failed")?
            == 0
        {
            info!("Remove danmu {} successfully", dmid);
            Ok(dmid)
        } else {
            let e = format!("Can't remove danmu. Response json: {}", json_res);
            error!(e);
            Err(e.into())
        }
    }
}
