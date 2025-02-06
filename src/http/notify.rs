use super::utility::{fetch_data, get_json};
use crate::http::response::official::{like, reply};
use crate::nvmsg;
use crate::screens::main;
use crate::types::{Message, RemoveAble, Result};
use iced::Task;
use indicatif::ProgressBar;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::try_join;
use tracing::{error, info, instrument};

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
    async fn remove(&self, id: u64, cl: Arc<Client>, csrf: Arc<String>) -> Result<u64> {
        match self.system_notify_api {
            Some(api_type) => {
                let json = if api_type == 0 {
                    json!({"csrf":*csrf,"ids":[id],"station_ids":[],"type":self.tp,"build":8140300,"mobi_app":"android"})
                } else {
                    json!({"csrf":*csrf,"ids":[],"station_ids":[id],"type":self.tp,"build":8140300,"mobi_app":"android"})
                };
                let json_res:Value = cl
                    .post(
                        format!("https://message.bilibili.com/x/sys-msg/del_notify_list?build=8140300&mobi_app=android&csrf={csrf}"),
                    )
                    .json(&json)
                    .send()
                    .await
                    ?
                    .json()
                    .await
                    ?;
                if json_res["code"].as_i64().unwrap() == 0 {
                    Ok(id)
                } else {
                    let e = format!(
                        "Can't remove the system notify. Response json: {}",
                        json_res
                    );
                    error!(e);
                    Err(e.into())
                }
            }
            None => {
                let json_res: Value = cl
                    .post(
                        "
    https://api.bilibili.com/x/msgfeed/del",
                    )
                    .form(&[
                        ("tp", self.tp.to_string()),
                        ("id", id.to_string()),
                        ("build", 0.to_string()),
                        ("mobi_app", "web".to_string()),
                        ("csrf_token", csrf.to_string()),
                        ("csrf", csrf.to_string()),
                    ])
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                if json_res["code"]
                    .as_i64()
                    .ok_or("Remove Notify: Parse json res code failed")?
                    == 0
                {
                    Ok(id)
                } else {
                    let e = format!("Can't remove notify. Response json: {}", json_res);
                    error!(e);
                    Err(e.into())
                }
            }
        }
    }
}

async fn fetch(cl: Arc<Client>, csrf: Arc<String>) -> Result<Arc<Mutex<HashMap<u64, Notify>>>> {
    let (m1, m2, m3, m4) = try_join!(
        fetch_liked(cl.clone()),
        fetch_ated(cl.clone()),
        fetch_replyed(cl.clone()),
        fetch_system_notify(cl.clone(), csrf.clone())
    )?;

    // 合并所有 HashMap
    let combined = m1.into_iter().chain(m2).chain(m3).chain(m4).collect();
    Ok(Arc::new(Mutex::new(combined)))
}

pub fn fetch_task(cl: Arc<Client>, csrf: Arc<String>) -> Task<Message> {
    Task::perform(fetch(cl.clone(), csrf.clone()), |e| {
        Message::Main(main::Message::NotifyMsg(nvmsg::NotifysFetched(e)))
    })
}

pub async fn fetch_liked(cl: Arc<Client>) -> Result<HashMap<u64, Notify>> {
    let mut m: HashMap<u64, Notify> = HashMap::new();
    let mut cursor_id = None;
    let mut cursor_time = None;
    let pb = ProgressBar::new_spinner();

    loop {
        let res;
        if cursor_id.is_none() && cursor_time.is_none() {
            // 第一次请求
            res = fetch_data::<like::ApiResponse>(
                cl.clone(),
                "https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web",
            )
            .await?
            .data
            .total;
            cursor_id = Some(res.cursor.id);
            cursor_time = Some(res.cursor.time);
        } else {
            res=fetch_data::<like::ApiResponse>(cl.clone(), format!("https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web&id={}&like_time={}",cursor_id.unwrap(),cursor_time.unwrap()))
                .await?.data.total;
            cursor_id = Some(res.cursor.id);
            cursor_time = Some(res.cursor.time);
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
        if res.cursor.is_end {
            info!("被点赞的通知处理完毕。");
            break;
        }
    }
    Ok(m)
}

pub async fn fetch_replyed(cl: Arc<Client>) -> Result<HashMap<u64, Notify>> {
    let mut m: HashMap<u64, Notify> = HashMap::new();
    let mut cursor_id = None;
    let mut cursor_time = None;
    let pb = ProgressBar::new_spinner();

    loop {
        let res;
        if cursor_id.is_none() && cursor_time.is_none() {
            // 第一次请求
            res = fetch_data::<reply::ApiResponse>(
                cl.clone(),
                "https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web",
            )
            .await?
            .data;
            cursor_id = Some(res.cursor.id);
            cursor_time = Some(res.cursor.time);
        } else {
            res=fetch_data::<reply::ApiResponse>(cl.clone(), format!("https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web&id={}&reply_time={}",cursor_id.unwrap(),cursor_time.unwrap()))
                .await?.data;
            cursor_id = Some(res.cursor.id);
            cursor_time = Some(res.cursor.time);
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
        if res.cursor.is_end {
            info!("被评论的通知处理完毕。");
            break;
        }
    }
    Ok(m)
}

pub async fn fetch_ated(cl: Arc<Client>) -> Result<HashMap<u64, Notify>> {
    let mut m: HashMap<u64, Notify> = HashMap::new();
    let mut cursor_id = None;
    let mut cursor_time = None;
    let pb = ProgressBar::new_spinner();

    loop {
        let res;
        if cursor_id.is_none() && cursor_time.is_none() {
            // 第一次请求
            res = fetch_data::<reply::ApiResponse>(
                cl.clone(),
                "https://api.bilibili.com/x/msgfeed/at?build=0&mobi_app=web",
            )
            .await?
            .data;
            cursor_id = Some(res.cursor.id);
            cursor_time = Some(res.cursor.time);
        } else {
            res = fetch_data::<reply::ApiResponse>(
                cl.clone(),
                format!(
                    "https://api.bilibili.com/x/msgfeed/at?build=0&mobi_app=web&id={}&at_time={}",
                    cursor_id.unwrap(),
                    cursor_time.unwrap()
                ),
            )
            .await?
            .data;
            cursor_id = Some(res.cursor.id);
            cursor_time = Some(res.cursor.time);
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
        if res.cursor.is_end {
            info!("被At的通知处理完毕。");
            break;
        }
    }
    Ok(m)
}

pub async fn fetch_system_notify(
    cl: Arc<Client>,
    csrf: Arc<String>,
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
            json = get_json(
                cl.clone(),
                format!("https://message.bilibili.com/x/sys-msg/query_user_notify?csrf={csrf}&csrf={csrf}&page_size=20&build=0&mobi_app=web"),
            )
                .await?;
            notifys = &json["data"]["system_notify_list"];
            // 第一种api（0）获取为空时
            if notifys.is_null() {
                api_type = 1;
                json = get_json(cl.clone(), format!("https://message.bilibili.com/x/sys-msg/query_unified_notify?csrf={csrf}&csrf={csrf}&page_size=10&build=0&mobi_app=web")).await?;
                notifys = &json["data"]["system_notify_list"];
                // 两者都为空
                if notifys.is_null() {
                    let i = "没有系统通知。";
                    info!(i);
                    return Err(i.into());
                }
            }
            cursor = notifys.as_array().unwrap().last().unwrap()["cursor"].as_u64();
        } else {
            let url =
                format!("https://message.bilibili.com/x/sys-msg/query_notify_list?csrf={csrf}&data_type=1&cursor={}&build=0&mobi_app=web",cursor.unwrap());
            json = get_json(cl.clone(), url).await?;
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
