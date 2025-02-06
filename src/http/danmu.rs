use crate::dvmsg;
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
use tracing::{error, info};

pub mod aicu;
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
}

impl RemoveAble for Danmu {
    async fn remove(&self, dmid: u64, cl: Arc<Client>, csrf: Arc<String>) -> Result<u64> {
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
            Ok(dmid)
        } else {
            let e = format!("Can't remove danmu. Response json: {}", json_res);
            error!(e);
            Err(e.into())
        }
    }
}

async fn fetch_both(cl: Arc<Client>) -> Result<Arc<Mutex<HashMap<u64, Danmu>>>> {
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
            Message::Main(main::Message::DanmuMsg(dvmsg::DanmusFetched(e)))
        })
    } else {
        Task::perform(official::fetch(cl), |e| {
            Message::Main(main::Message::DanmuMsg(dvmsg::DanmusFetched(e)))
        })
    }
}
