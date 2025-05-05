use super::api_service::ApiService;
use crate::http::response::official::{like, reply};
use crate::nvmsg;
use crate::types::{Error, Message, RemoveAble, Result};
use iced::Task;
use indicatif::ProgressBar;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::try_join;
use tracing::{info, warn};

#[derive(Clone, Debug)]
pub struct Notify {
    pub content: String,
    tp: u8,
    pub is_selected: bool,
    /// 删除系统通知的两种api
    system_notify_api: Option<u8>,
}
impl Notify {
    pub fn new(content: String, tp: u8) -> Notify {
        Notify {
            content,
            tp,
            is_selected: true,
            system_notify_api: None,
        }
    }

    fn new_system_notify(content: String, tp: u8, api_type: u8) -> Notify {
        Notify {
            content,
            tp,
            is_selected: true,
            system_notify_api: Some(api_type),
        }
    }
}
impl RemoveAble for Notify {
    async fn remove(&self, id: u64, api: Arc<ApiService>) -> Result<u64> {
        match self.system_notify_api {
            Some(api_type) => {
                let csrf = api.csrf();
                let json = if api_type == 0 {
                    json!({"csrf":csrf,"ids":[id],"station_ids":[],"type":self.tp,"build":8140300,"mobi_app":"android"})
                } else {
                    json!({"csrf":csrf,"ids":[],"station_ids":[id],"type":self.tp,"build":8140300,"mobi_app":"android"})
                };
                let url = format!("https://message.bilibili.com/x/sys-msg/del_notify_list?build=8140300&mobi_app=android&csrf={csrf}");
                let json_res: Value = api.post_json(url, &json).await?.json().await?;
                if json_res["code"].as_i64().unwrap() == 0 {
                    Ok(id)
                } else {
                    Err(Error::DeleteSystemNotifyError(json_res.into()))
                }
            }
            None => {
                let form_data = [
                    ("tp", self.tp.to_string()),
                    ("id", id.to_string()),
                    ("build", 0.to_string()),
                    ("mobi_app", "web".to_string()),
                    ("csrf_token", api.csrf().to_string()),
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
                    Ok(id)
                } else {
                    Err(Error::DeleteNotifyError(json_res.into()))
                }
            }
        }
    }
}

pub async fn fetch(api: Arc<ApiService>) -> Result<Arc<Mutex<HashMap<u64, Notify>>>> {
    let (m1, m2, m3, m4) = try_join!(
        fetch_liked(api.clone()),
        fetch_ated(api.clone()),
        fetch_replyed(api.clone()),
        fetch_system_notify(api.clone())
    )?;

    // 合并所有 HashMap
    let combined = m1.into_iter().chain(m2).chain(m3).chain(m4).collect();
    Ok(Arc::new(Mutex::new(combined)))
}

pub fn fetch_task(api: Arc<ApiService>) -> Task<Message> {
    Task::perform(fetch(api), |e| {
        nvmsg::NotifysFetched(e).into()
    })
}

pub async fn fetch_liked(api: Arc<ApiService>) -> Result<HashMap<u64, Notify>> {
    let mut m: HashMap<u64, Notify> = HashMap::new();
    let mut cursor_id = None;
    let mut cursor_time = None;
    let pb = ProgressBar::new_spinner();

    loop {
        let res = if cursor_id.is_none() && cursor_time.is_none() {
            // 第一次请求
            api.fetch_data::<like::ApiResponse>(
                "https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web",
            )
            .await?
            .data
            .total
        } else {
            api.fetch_data::<like::ApiResponse>(
                format!("https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web&id={}&like_time={}",
                cursor_id.unwrap(),
                cursor_time.unwrap())
            )
            .await?
            .data
            .total
        };
        if let Some(c) = &res.cursor {
            cursor_id = Some(c.id);
            cursor_time = Some(c.time);
        } else {
            return Ok(m);
        }
        for i in res.items {
            m.insert(
                i.id,
                Notify::new(
                    format!("{} ({})", i.item.nested.title, i.item.nested.item_type),
                    0,
                ),
            );
            pb.set_message(format!(
                "Fetched liked notify: {}. Counts now: {}",
                i.id,
                m.len()
            ));
            pb.tick();
        }
        if res.cursor.unwrap().is_end {
            info!("被点赞的通知处理完毕。");
            break;
        }
    }
    Ok(m)
}

pub async fn fetch_replyed(api: Arc<ApiService>) -> Result<HashMap<u64, Notify>> {
    let mut m: HashMap<u64, Notify> = HashMap::new();
    let mut cursor_id = None;
    let mut cursor_time = None;
    let pb = ProgressBar::new_spinner();

    loop {
        let res = if cursor_id.is_none() && cursor_time.is_none() {
            // 第一次请求
            api.fetch_data::<reply::ApiResponse>(
                "https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web",
            )
            .await?
            .data
        } else {
            api.fetch_data::<reply::ApiResponse>(
                format!("https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web&id={}&reply_time={}",
                cursor_id.unwrap(),
                cursor_time.unwrap())
            )
            .await?
            .data
        };
        if let Some(c) = &res.cursor {
            cursor_id = Some(c.id);
            cursor_time = Some(c.time);
        } else {
            return Ok(m);
        }
        for i in res.items {
            m.insert(
                i.id,
                Notify::new(
                    format!("{} ({})", i.item.nested.title, i.item.nested.item_type),
                    1,
                ),
            );
            pb.set_message(format!(
                "Fetched replyed notify: {}. Counts now: {}",
                i.id,
                m.len()
            ));
            pb.tick();
        }
        if res.cursor.unwrap().is_end {
            info!("被评论的通知处理完毕。");
            break;
        }
    }
    Ok(m)
}

pub async fn fetch_ated(api: Arc<ApiService>) -> Result<HashMap<u64, Notify>> {
    let mut m: HashMap<u64, Notify> = HashMap::new();
    let mut cursor_id = None;
    let mut cursor_time = None;
    let pb = ProgressBar::new_spinner();

    loop {
        let res = if cursor_id.is_none() && cursor_time.is_none() {
            // 第一次请求
            api.fetch_data::<reply::ApiResponse>(
                "https://api.bilibili.com/x/msgfeed/at?build=0&mobi_app=web",
            )
            .await?
            .data
        } else {
            api.fetch_data::<reply::ApiResponse>(
                format!(
                    "https://api.bilibili.com/x/msgfeed/at?build=0&mobi_app=web&id={}&at_time={}",
                    cursor_id.unwrap(),
                    cursor_time.unwrap()
                ),
            )
            .await?
            .data
        };
        if let Some(c) = &res.cursor {
            cursor_id = Some(c.id);
            cursor_time = Some(c.time);
        } else {
            return Ok(m);
        }
        for i in res.items {
            m.insert(
                i.id,
                Notify::new(
                    format!("{} ({})", i.item.nested.title, i.item.nested.item_type),
                    2,
                ),
            );
            pb.set_message(format!(
                "Fetched ated notify: {}. Counts now: {}",
                i.id,
                m.len()
            ));
            pb.tick();
        }
        if res.cursor.unwrap().is_end {
            info!("被At的通知处理完毕。");
            break;
        }
    }
    Ok(m)
}

pub async fn fetch_system_notify(
    api: Arc<ApiService>,
) -> Result<HashMap<u64, Notify>> {
    let mut h: HashMap<u64, Notify> = HashMap::new();
    let mut cursor = None;
    let mut api_type = 0_u8;
    let pb = ProgressBar::new_spinner();

    loop {
        let mut json: Value;
        let mut notifys: &Value;
        // first get
        if cursor.is_none() {
            json = api.get_json(
                format!("https://message.bilibili.com/x/sys-msg/query_user_notify?csrf={}&csrf={}&page_size=20&build=0&mobi_app=web",
                api.csrf(), api.csrf()),
            )
            .await?;
            notifys = &json["data"]["system_notify_list"];
            // 第一种api（0）获取为空时
            if notifys.is_null() {
                api_type = 1;
                json = api.get_json(
                    format!("https://message.bilibili.com/x/sys-msg/query_unified_notify?csrf={}&csrf={}&page_size=10&build=0&mobi_app=web",
                    api.csrf(), api.csrf())
                ).await?;
                notifys = &json["data"]["system_notify_list"];
                // 两者都为空
                if notifys.is_null() {
                    let i = "没有系统通知。";
                    warn!("{}", i);
                    return Ok(h);
                }
            }
            cursor = notifys.as_array().unwrap().last().unwrap()["cursor"].as_u64();
        } else {
            let url =
                format!("https://message.bilibili.com/x/sys-msg/query_notify_list?csrf={}&data_type=1&cursor={}&build=0&mobi_app=web",
                api.csrf(), cursor.unwrap());
            json = api.get_json(url).await?;
            notifys = &json["data"];
            if json["data"].as_array().unwrap().is_empty() {
                info!("系统通知处理完毕。通知数量：{}", h.len());
                break;
            }
            cursor = notifys.as_array().unwrap().last().unwrap()["cursor"].as_u64();
        }

        for i in notifys.as_array().unwrap() {
            let notify_id = i["id"].as_u64().unwrap();
            let notify_type = i["type"].as_u64().unwrap() as u8;
            h.insert(
                notify_id,
                Notify::new_system_notify(
                    format!(
                        "{}\n{}",
                        i["title"].as_str().unwrap(),
                        i["content"].as_str().unwrap()
                    ),
                    notify_type,
                    api_type,
                ),
            );
            pb.set_message(format!(
                "Fetched system notify: {}. Counts now: {}",
                notify_id,
                h.len()
            ));
            pb.tick();
        }
    }
    Ok(h)
}
