use crate::http::notify::Notify;
use crate::http::utility::{get_json, get_uid};
use crate::types::Result;
use indicatif::ProgressBar;
use regex::Regex;
use reqwest::{Client, Url};
use serde_json::Value;
use std::collections::HashMap;
use std::mem;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio::try_join;
use tracing::{error, info, warn};

#[derive(Debug, Default, Clone)]
pub struct Comment {
    pub oid: u64,
    pub r#type: u8,
    // pub rpid: u64,
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
    pub async fn remove(&self, rpid: u64, cl: Arc<Client>, csrf: Arc<String>) -> Result<u64> {
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
            error!(e);
            Err(e.into())
        }
    }
}

pub async fn fetch_from_aicu(cl: Arc<Client>) -> Result<Arc<Mutex<HashMap<u64, Comment>>>> {
    let uid = get_uid(Arc::clone(&cl)).await?;
    let mut page = 1;
    let mut h = HashMap::new();

    // get counts & init progress bar
    let total_replies = get_json(
        Arc::clone(&cl),
        format!(
            "https://api.aicu.cc/api/v3/search/getreply?uid={}&pn=1&ps=0&mode=0&keyword=",
            uid
        ),
    )
    .await?["data"]["cursor"]["all_count"]
        .as_u64()
        .ok_or("fetch_from_aicu: Parse `all_count` failed")?;
    let pb = ProgressBar::new(total_replies);
    let max = total_replies as f32;
    println!("正在从aicu.cc获取数据...");
    sleep(Duration::from_secs(1)).await;
    loop {
        let res = get_json(
            Arc::clone(&cl),
            format!(
                "https://api.aicu.cc/api/v3/search/getreply?uid={}&pn={}&ps=500&mode=0&keyword=",
                uid, page
            ),
        )
        .await?;
        let replies = &res["data"]["replies"];
        for i in replies
            .as_array()
            .ok_or("fetch_from_aicu: replies isn't a array")?
        {
            let rpid = i["rpid"]
                .as_str()
                .unwrap()
                .parse()
                .map_err(|_| "fetch_from_aicu: Parse `rpid` failed")?;
            h.insert(
                rpid,
                Comment::new(
                    i["dyn"]["oid"].as_str().unwrap().parse().unwrap(),
                    i["dyn"]["type"].as_u64().unwrap() as u8,
                    i["message"].as_str().unwrap().to_string(),
                ),
            );
            pb.inc(1);
        }
        page += 1;
        if res["data"]["cursor"]["is_end"].as_bool().unwrap() {
            pb.finish_with_message("Fetched successful from aicu.cc");
            break;
        }
        sleep(Duration::from_secs(3)).await;
    }
    Ok(Arc::new(Mutex::new(h)))
}

enum MsgType {
    Like,
    Reply,
    At,
}

pub async fn fetch_from_official(cl: Arc<Client>) -> Result<Arc<Mutex<HashMap<u64, Comment>>>> {
    let mut h = HashMap::new();
    let oid_regex = Regex::new(r"bilibili://video/(\d+)").unwrap();
    let mut msgtype = MsgType::Like;
    let mut queryid = None;
    let mut last_time = None;
    let pb = ProgressBar::new_spinner();
    loop {
        let json: serde_json::Value;
        let notifys: &serde_json::Value;
        if queryid.is_none() && last_time.is_none() {
            // 第一次请求
            json = get_json(
                Arc::clone(&cl),
                match msgtype {
                    MsgType::Like => {
                        "https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web"
                    }
                    MsgType::Reply => {
                        "https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web"
                    }
                    MsgType::At => "https://api.bilibili.com/x/msgfeed/at?build=0&mobi_app=web",
                },
            )
            .await?;

            match msgtype {
                MsgType::Like => {
                    notifys = &json["data"]["total"]["items"];
                    if notifys.as_array().unwrap().is_empty() {
                        msgtype = MsgType::Reply;
                        info!("没有收到赞的评论。");
                        continue;
                    }
                    last_time = notifys.as_array().unwrap().last().unwrap()["like_time"].as_u64();
                    queryid = json["data"]["total"]["cursor"]["id"].as_u64();
                }
                MsgType::Reply => {
                    notifys = &json["data"]["items"];
                    if notifys.as_array().unwrap().is_empty() {
                        msgtype = MsgType::At;
                        info!("没有收到评论的评论。");
                        continue;
                    }
                    last_time = notifys.as_array().unwrap().last().unwrap()["reply_time"].as_u64();
                    queryid = json["data"]["cursor"]["id"].as_u64();
                }
                MsgType::At => {
                    notifys = &json["data"]["items"];
                    if notifys.as_array().unwrap().is_empty() {
                        info!("没有被At的评论。");
                        break;
                    }
                    last_time = notifys.as_array().unwrap().last().unwrap()["at_time"].as_u64();
                    queryid = json["data"]["cursor"]["id"].as_u64();
                }
            }
        } else {
            let mut url = Url::parse(match msgtype {
                MsgType::Like => {
                    "https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web"
                }
                MsgType::Reply => {
                    "https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web"
                }
                MsgType::At => "https://api.bilibili.com/x/msgfeed/at?build=0&mobi_app=web",
            })
            .unwrap();
            match msgtype {
                MsgType::Like => {
                    url.query_pairs_mut()
                        .append_pair("id", &queryid.unwrap().to_string())
                        .append_pair("like_time", &last_time.unwrap().to_string());
                    json = get_json(Arc::clone(&cl), url).await?;
                    notifys = &json["data"]["total"]["items"];
                    last_time = notifys.as_array().unwrap().last().unwrap()["like_time"].as_u64();
                    queryid = json["data"]["total"]["cursor"]["id"].as_u64();
                }
                MsgType::Reply => {
                    url.query_pairs_mut()
                        .append_pair("id", &queryid.unwrap().to_string())
                        .append_pair("reply_time", &last_time.unwrap().to_string());
                    json = get_json(Arc::clone(&cl), url).await?;
                    notifys = &json["data"]["items"];
                    last_time = notifys.as_array().unwrap().last().unwrap()["reply_time"].as_u64();
                    queryid = json["data"]["cursor"]["id"].as_u64();
                }
                MsgType::At => {
                    url.query_pairs_mut()
                        .append_pair("id", &queryid.unwrap().to_string())
                        .append_pair("at_time", &last_time.unwrap().to_string());
                    json = get_json(Arc::clone(&cl), url).await?;
                    notifys = &json["data"]["items"];
                    last_time = notifys.as_array().unwrap().last().unwrap()["at_time"].as_u64();
                    queryid = json["data"]["cursor"]["id"].as_u64();
                }
            }
        }
        // dbg!(queryid, last_time);
        let mut r#type: u8;
        'outer: for i in notifys.as_array().unwrap() {
            if i["item"]["type"] == "reply" {
                let rpid = if let MsgType::Like = msgtype {
                    i["item"]["item_id"].as_u64().unwrap()
                } else {
                    i["item"]["target_id"].as_u64().unwrap()
                };

                let uri = i["item"]["uri"].as_str().unwrap();
                let oid;

                if uri.contains("t.bilibili.com") {
                    // 动态内评论
                    oid = match uri.replace("https://t.bilibili.com/", "").parse::<u64>() {
                        Ok(v) => v,
                        Err(e) => {
                            error!("Error json:{}\nError uri:{}\n{}", json, uri, e.to_string());
                            continue;
                        }
                    };

                    let business_id = i["item"]["business_id"].as_u64();
                    r#type = match business_id {
                        Some(v) if v != 0 => v as u8,
                        _ => 17,
                    };
                } else if uri.contains("https://h.bilibili.com/ywh/") {
                    // 带图动态内评论
                    oid = match uri
                        .replace("https://h.bilibili.com/ywh/", "")
                        .parse::<u64>()
                    {
                        Ok(v) => v,
                        Err(e) => {
                            error!("Error json:{}\nError uri:{}\n{}", json, uri, e.to_string());
                            continue;
                        }
                    };
                    r#type = 11;
                } else if uri.contains("https://www.bilibili.com/read/cv") {
                    // 专栏内评论
                    oid = match uri
                        .replace("https://www.bilibili.com/read/cv", "")
                        .parse::<u64>()
                    {
                        Ok(v) => v,
                        Err(e) => {
                            error!("Error json:{}\nError uri:{}\n{}", json, uri, e.to_string());
                            continue;
                        }
                    };
                    r#type = 12;
                } else if uri.contains("https://www.bilibili.com/video/") {
                    // 视频内评论
                    oid = match oid_regex.captures(i["item"]["native_uri"].as_str().unwrap()) {
                        Some(v) => match v.get(1).unwrap().as_str().parse::<u64>() {
                            Ok(v) => v,
                            Err(e) => {
                                error!("Error json:{}\nError uri:{}\n{}", json, uri, e.to_string());
                                continue;
                            }
                        },
                        None => {
                            error!(
                                "Error json:{}\nError uri:{}\n{}",
                                json,
                                uri,
                                "Can't captures by the oid_regex var".to_string()
                            );
                            continue;
                        }
                    };
                    r#type = 1;
                } else if uri.contains("https://www.bilibili.com/bangumi/play/") {
                    // 电影（番剧？）内评论
                    oid = match i["item"]["subject_id"].as_u64() {
                        Some(v) => v,
                        None => {
                            error!(
                                "Error json:{}\nError uri:{}\n{}",
                                json,
                                uri,
                                "The subject_id field is null".to_string()
                            );
                            continue;
                        }
                    };
                    r#type = 1;
                } else if uri.is_empty() {
                    info!("No URI, Skiped");
                    continue;
                } else {
                    warn!("Undefined URI:{}\nSkip this comment: {}", uri, rpid);
                    continue;
                }
                let content = match msgtype {
                    MsgType::Like => i["item"]["title"].as_str().unwrap().to_string(),
                    MsgType::Reply => {
                        let v = i["item"]["target_reply_content"]
                            .as_str()
                            .unwrap()
                            .to_string();
                        if v.is_empty() {
                            i["item"]["title"].as_str().unwrap().to_string()
                        } else {
                            v
                        }
                    }
                    MsgType::At => {
                        format!("{}\n({})", i["item"]["source_content"], i["item"]["title"])
                    }
                };
                let notify_id = i["id"].as_u64().unwrap();
                h.insert(
                    rpid,
                    Comment {
                        oid,
                        r#type,
                        content: content.clone(),
                        is_selected: true,
                        notify_id: Some(notify_id),
                        tp: match msgtype {
                            MsgType::Like => Some(0),
                            MsgType::Reply => Some(1),
                            MsgType::At => Some(2),
                        },
                    },
                );
                let msg = format!("Push Comment: {}, Counts now: {}", rpid, h.len());
                pb.set_message(msg.clone());
                pb.tick();
            }
        }
        // push完检测是否为end
        match msgtype {
            MsgType::Like => {
                if json["data"]["total"]["cursor"]["is_end"].as_bool().unwrap() {
                    msgtype = MsgType::Reply;
                    last_time = None;
                    queryid = None;
                    info!("收到赞的评论处理完毕。");
                }
            }
            MsgType::Reply => {
                if json["data"]["cursor"]["is_end"].as_bool().unwrap() {
                    msgtype = MsgType::At;
                    last_time = None;
                    queryid = None;
                    info!("收到评论的评论处理完毕。");
                    continue;
                }
            }
            MsgType::At => {
                if json["data"]["cursor"]["is_end"].as_bool().unwrap() {
                    info!("被At的评论处理完毕。");
                    pb.finish_with_message("done");
                    break;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Ok(Arc::new(Mutex::new(h)))
}

pub async fn fetch_both(cl: Arc<Client>) -> Result<Arc<Mutex<HashMap<u64, Comment>>>> {
    let (m1, m2) = try_join!(fetch_from_official(cl.clone()), fetch_from_aicu(cl.clone()))?;

    let (m1, m2) = {
        let mut lock1 = m1.lock().await;
        let mut lock2 = m2.lock().await;
        (
            mem::replace(&mut *lock1, HashMap::new()),
            mem::replace(&mut *lock2, HashMap::new()),
        )
    };

    Ok(Arc::new(Mutex::new(m1.into_iter().chain(m2).collect())))
}
