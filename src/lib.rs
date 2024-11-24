use iced::futures::SinkExt;
use iced::{stream, Subscription};
use indicatif::ProgressBar;
use reqwest::{header, Client, IntoUrl};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::spawn;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tracing::{debug, error, info};
pub mod types;
use crate::types::{ChannelMsg, Message};

pub async fn get_json<T: IntoUrl>(cl: Arc<Client>, url: T) -> Value {
    let res_str = cl.get(url).send().await.unwrap().text().await.unwrap();
    debug!("Got res: {}", res_str);
    let res = serde_json::from_str::<Value>(res_str.as_str())
        .unwrap_or_else(|_| panic!("Cannot get json, res string: {res_str}"));
    // dbg!(&res);
    if res["code"] != 0 {
        panic!("Can't get request, Json response: {}", res);
    } else {
        res
    }
}

pub async fn get_uid(cl: Arc<Client>) -> u64 {
    let json_res = get_json(cl, "https://api.bilibili.com/x/member/web/account").await;
    let uid = json_res["data"]["mid"]
        .as_u64()
        .expect("Can't get uid. Please check your cookie data");
    info!(
        "Got uid: {uid}\nI found u, {} 😎",
        json_res["data"]["uname"]
    );
    uid
}

pub async fn create_client(ck: String) -> (Client, String) {
    let a = ck
        .find("bili_jct=")
        .expect("Can't find csrf data.Make sure that your cookie data has a bili_jct field.");
    let b = ck[a..].find(";").unwrap();
    let csrf = &ck[a + 9..b + a];

    let mut headers = header::HeaderMap::new();
    headers.insert("User-Agent", header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36 Edg/127.0.2651.86"));
    headers.insert("Cookie", header::HeaderValue::from_str(&ck).unwrap());
    let cl = Client::builder()
        .default_headers(headers)
        .cookie_store(true)
        .build()
        .unwrap();

    (cl, csrf.to_string())
}

pub mod notify {
    use crate::get_json;
    use reqwest::{Client, Url};
    use serde_json::json;
    use std::{sync::Arc, time::Duration};
    use tracing::{error, info, instrument};

    #[instrument(skip_all)]
    pub async fn remove_notify(cl: Arc<Client>, id: u64, csrf: Arc<String>, tp: String) {
        let res = cl
            .post(
                "
    https://api.bilibili.com/x/msgfeed/del",
            )
            .form(&[
                ("tp", tp),
                ("id", id.to_string()),
                ("build", 0.to_string()),
                ("mobi_app", "web".to_string()),
                ("csrf_token", csrf.to_string()),
                ("csrf", csrf.to_string()),
            ])
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        let json_res: serde_json::Value = serde_json::from_str(res.as_str()).unwrap();
        if json_res["code"].as_i64().unwrap() == 0 {
            info!("Remove notify {id} successfully");
        } else {
            error!("Can't remove notify. Response json: {}", json_res);
        }
    }

    #[instrument(skip_all)]
    pub async fn fetch_remove_liked_notify(cl: Arc<Client>, csrf: Arc<String>) {
        let mut v: Vec<(u64, u8)> = Vec::new();
        let mut queryid = None;
        let mut last_time = None;

        loop {
            let json: serde_json::Value;
            let notifys: &serde_json::Value;
            // first get
            if queryid.is_none() && last_time.is_none() {
                json = get_json(
                    cl.clone(),
                    "https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web",
                )
                .await;
                notifys = &json["data"]["total"]["items"];
                if notifys.as_array().unwrap().is_empty() {
                    info!("没有收到赞的通知。");
                    return;
                }
                last_time = notifys.as_array().unwrap().last().unwrap()["like_time"].as_u64();
                queryid = json["data"]["total"]["cursor"]["id"].as_u64();
            } else {
                json=get_json(cl.clone(), format!("https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web&id={}&like_time={}",&queryid.unwrap().to_string(),&last_time.unwrap().to_string())).await;
                notifys = &json["data"]["total"]["items"];
                last_time = notifys.as_array().unwrap().last().unwrap()["like_time"].as_u64();
                queryid = json["data"]["total"]["cursor"]["id"].as_u64();
            }

            for i in notifys.as_array().unwrap() {
                let notify_id = i["id"].as_u64().unwrap();
                v.push((notify_id, 0));
                info!("Fetched notify {notify_id}");
            }

            if json["data"]["total"]["cursor"]["is_end"].as_bool().unwrap() {
                info!("收到赞的通知处理完毕。通知数量：{}", v.len());
                break;
            }
        }
        for i in v {
            remove_notify(Arc::clone(&cl), i.0, Arc::clone(&csrf), i.1.to_string()).await;
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    }

    #[instrument(skip_all)]
    pub async fn fetch_remove_replyed_notify(cl: Arc<Client>, csrf: Arc<String>) {
        let mut v: Vec<(u64, u8)> = Vec::new();
        let mut queryid = None;
        let mut last_time = None;

        loop {
            let json: serde_json::Value;
            let notifys: &serde_json::Value;
            // first get
            if queryid.is_none() && last_time.is_none() {
                json = get_json(
                    cl.clone(),
                    "https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web",
                )
                .await;
                notifys = &json["data"]["items"];
                if notifys.as_array().unwrap().is_empty() {
                    info!("没有收到评论的通知。");
                    return;
                }
                last_time = notifys.as_array().unwrap().last().unwrap()["reply_time"].as_u64();
                queryid = json["data"]["cursor"]["id"].as_u64();
            } else {
                let mut url = Url::parse(
                    "https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web",
                )
                .unwrap();
                url.query_pairs_mut()
                    .append_pair("id", &queryid.unwrap().to_string())
                    .append_pair("reply_time", &last_time.unwrap().to_string());
                json = get_json(cl.clone(), url).await;
                notifys = &json["data"]["items"];
                last_time = notifys.as_array().unwrap().last().unwrap()["reply_time"].as_u64();
                queryid = json["data"]["cursor"]["id"].as_u64();
            }

            for i in notifys.as_array().unwrap() {
                let notify_id = i["id"].as_u64().unwrap();
                v.push((notify_id, 1));
                info!("Fetched notify {notify_id}");
            }

            if json["data"]["cursor"]["is_end"].as_bool().unwrap() {
                info!("收到评论的通知处理完毕。通知数量：{}", v.len());
                break;
            }
        }
        for i in v {
            remove_notify(Arc::clone(&cl), i.0, Arc::clone(&csrf), i.1.to_string()).await;
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    }

    #[instrument(skip_all)]
    pub async fn fetch_remove_ated_notify(cl: Arc<Client>, csrf: Arc<String>) {
        let mut v: Vec<(u64, u8)> = Vec::new();
        let mut queryid = None;
        let mut last_time = None;

        loop {
            let json: serde_json::Value;
            let notifys: &serde_json::Value;
            // first get
            if queryid.is_none() && last_time.is_none() {
                json = get_json(
                    cl.clone(),
                    "https://api.bilibili.com/x/msgfeed/at?build=0&mobi_app=web",
                )
                .await;
                notifys = &json["data"]["items"];
                if notifys.as_array().unwrap().is_empty() {
                    info!("没有被At的通知。");
                    return;
                }
                last_time = notifys.as_array().unwrap().last().unwrap()["at_time"].as_u64();
                queryid = json["data"]["cursor"]["id"].as_u64();
            } else {
                let mut url =
                    Url::parse("https://api.bilibili.com/x/msgfeed/at?build=0&mobi_app=web")
                        .unwrap();
                url.query_pairs_mut()
                    .append_pair("id", &queryid.unwrap().to_string())
                    .append_pair("at_time", &last_time.unwrap().to_string());
                json = get_json(cl.clone(), url).await;
                notifys = &json["data"]["items"];
                last_time = notifys.as_array().unwrap().last().unwrap()["at_time"].as_u64();
                queryid = json["data"]["cursor"]["id"].as_u64();
            }

            for i in notifys.as_array().unwrap() {
                let notify_id = i["id"].as_u64().unwrap();
                v.push((notify_id, 2));
                info!("Fetched notify {notify_id}");
            }

            if json["data"]["cursor"]["is_end"].as_bool().unwrap() {
                info!("被At的通知处理完毕。通知数量：{}", v.len());
                break;
            }
        }
        for i in v {
            remove_notify(Arc::clone(&cl), i.0, Arc::clone(&csrf), i.1.to_string()).await;
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    }

    #[instrument(skip_all)]
    pub async fn fetch_remove_system_notify(cl: Arc<Client>, csrf: Arc<String>) {
        let mut v: Vec<(u64, u8, u8)> = Vec::new();
        let mut cursor = None;
        let mut api_type = 0_u8;
        loop {
            let mut json: serde_json::Value;
            let mut notifys: &serde_json::Value;
            // first get
            if cursor.is_none() {
                json = get_json(
                    cl.clone(),
                    format!("https://message.bilibili.com/x/sys-msg/query_user_notify?csrf={csrf}&csrf={csrf}&page_size=20&build=0&mobi_app=web"),
                )
                .await;
                notifys = &json["data"]["system_notify_list"];
                if notifys.is_null() {
                    api_type = 1;
                    json = get_json(cl.clone(), format!("https://message.bilibili.com/x/sys-msg/query_unified_notify?csrf={csrf}&csrf={csrf}&page_size=10&build=0&mobi_app=web")).await;
                    notifys = &json["data"]["system_notify_list"];
                    if notifys.is_null() {
                        return;
                    }
                }
                cursor = notifys.as_array().unwrap().last().unwrap()["cursor"].as_u64();
            } else {
                let url =
                    format!("https://message.bilibili.com/x/sys-msg/query_notify_list?csrf={csrf}&data_type=1&cursor={}&build=0&mobi_app=web",cursor.unwrap());
                json = get_json(cl.clone(), url).await;
                notifys = &json["data"];
                if json["data"].as_array().unwrap().is_empty() {
                    info!("系统通知处理完毕。通知数量：{}", v.len());
                    break;
                }
                cursor = notifys.as_array().unwrap().last().unwrap()["cursor"].as_u64();
            }

            for i in notifys.as_array().unwrap() {
                let notify_id = i["id"].as_u64().unwrap();
                let notify_type = i["type"].as_u64().unwrap() as u8;
                v.push((notify_id, notify_type, api_type));
                info!("Fetched notify {notify_id}");
            }
        }
        for i in v {
            remove_system_notify(Arc::clone(&cl), i.0, Arc::clone(&csrf), i.1, i.2).await;
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    }

    #[instrument(skip_all)]
    pub async fn remove_system_notify(
        cl: Arc<Client>,
        id: u64,
        csrf: Arc<String>,
        tp: u8,
        api_type: u8,
    ) {
        let json = if api_type == 0 {
            json!({"csrf":*csrf,"ids":[id],"station_ids":[],"type":tp,"build":8140300,"mobi_app":"android"})
        } else {
            json!({"csrf":*csrf,"ids":[],"station_ids":[id],"type":tp,"build":8140300,"mobi_app":"android"})
        };
        let res = cl
            .post(
                format!("https://message.bilibili.com/x/sys-msg/del_notify_list?build=8140300&mobi_app=android&csrf={csrf}"),
            )
            .json(&json)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        let json_res: serde_json::Value = serde_json::from_str(res.as_str()).unwrap();
        if json_res["code"].as_i64().unwrap() == 0 {
            info!("Remove system notify {id} successfully");
        } else {
            error!(
                "Can't remove the system notify. Response json: {}",
                json_res
            );
        }
    }
}

pub mod comment {
    use crate::types::{Comment, Message};
    use crate::{get_json, get_uid};
    use iced::futures::{SinkExt, Stream, StreamExt};
    use iced::stream;
    use indicatif::ProgressBar;
    use regex::Regex;
    use reqwest::{Client, Url};
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;
    use tokio::time::sleep;
    use tracing::{error, info};
    fn fetch_comment_from_aicu(cl: Arc<Client>) -> impl Stream<Item = Message> {
        stream::channel(10, |mut output| async move {
            let uid = get_uid(Arc::clone(&cl)).await;
            let mut page = 1;
            let mut v = Vec::new();

            // get counts & init progress bar
            let total_replies = get_json(
                Arc::clone(&cl),
                format!(
                    "https://api.aicu.cc/api/v3/search/getreply?uid={}&pn=1&ps=0&mode=0&keyword=",
                    uid
                ),
            )
            .await["data"]["cursor"]["all_count"]
                .as_u64()
                .unwrap();
            let pb = ProgressBar::new(total_replies);
            let max = total_replies as f32;
            let mut count = 0.0;
            output
                .send(Message::AicuFetchingState { now: count, max })
                .await
                .unwrap();
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
                    .await;
                let replies = &res["data"]["replies"];
                for i in replies.as_array().unwrap() {
                    let rpid = i["rpid"].as_str().unwrap().parse().unwrap();
                    v.push(Comment {
                        oid: i["dyn"]["oid"].as_str().unwrap().parse().unwrap(),
                        r#type: i["dyn"]["type"].as_u64().unwrap() as u8,
                        rpid,
                        content: i["message"].as_str().unwrap().to_string(),
                        remove_state: true,
                        notify_id: None,
                        tp: None,
                    });
                    pb.inc(1);
                    count += 1.0;
                    output
                        .send(Message::AicuFetchingState { now: count, max })
                        .await
                        .unwrap();
                }
                page += 1;
                if res["data"]["cursor"]["is_end"].as_bool().unwrap() {
                    pb.finish_with_message("Fetched successful from aicu.cc");
                    break;
                }
                sleep(Duration::from_secs(3)).await;
            }
            output
                .send(Message::CommentsFetched(Arc::new(Mutex::new(v))))
                .await
                .unwrap();
        })
    }

    enum MsgType {
        Like,
        Reply,
        At,
    }

    pub fn fetch_comment(cl: Arc<Client>) -> impl Stream<Item = Message> {
        stream::channel(10, |mut output| async move {
            let mut v: Vec<Comment> = Vec::new();
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
                        .await;

                    match msgtype {
                        MsgType::Like => {
                            notifys = &json["data"]["total"]["items"];
                            if notifys.as_array().unwrap().is_empty() {
                                msgtype = MsgType::Reply;
                                info!("没有收到赞的评论。");
                                continue;
                            }
                            last_time =
                                notifys.as_array().unwrap().last().unwrap()["like_time"].as_u64();
                            queryid = json["data"]["total"]["cursor"]["id"].as_u64();
                        }
                        MsgType::Reply => {
                            notifys = &json["data"]["items"];
                            if notifys.as_array().unwrap().is_empty() {
                                msgtype = MsgType::At;
                                info!("没有收到评论的评论。");
                                continue;
                            }
                            last_time =
                                notifys.as_array().unwrap().last().unwrap()["reply_time"].as_u64();
                            queryid = json["data"]["cursor"]["id"].as_u64();
                        }
                        MsgType::At => {
                            notifys = &json["data"]["items"];
                            if notifys.as_array().unwrap().is_empty() {
                                info!("没有被At的评论。");
                                break;
                            }
                            last_time =
                                notifys.as_array().unwrap().last().unwrap()["at_time"].as_u64();
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
                            json = get_json(Arc::clone(&cl), url).await;
                            notifys = &json["data"]["total"]["items"];
                            last_time =
                                notifys.as_array().unwrap().last().unwrap()["like_time"].as_u64();
                            queryid = json["data"]["total"]["cursor"]["id"].as_u64();
                        }
                        MsgType::Reply => {
                            url.query_pairs_mut()
                                .append_pair("id", &queryid.unwrap().to_string())
                                .append_pair("reply_time", &last_time.unwrap().to_string());
                            json = get_json(Arc::clone(&cl), url).await;
                            notifys = &json["data"]["items"];
                            last_time =
                                notifys.as_array().unwrap().last().unwrap()["reply_time"].as_u64();
                            queryid = json["data"]["cursor"]["id"].as_u64();
                        }
                        MsgType::At => {
                            url.query_pairs_mut()
                                .append_pair("id", &queryid.unwrap().to_string())
                                .append_pair("at_time", &last_time.unwrap().to_string());
                            json = get_json(Arc::clone(&cl), url).await;
                            notifys = &json["data"]["items"];
                            last_time =
                                notifys.as_array().unwrap().last().unwrap()["at_time"].as_u64();
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
                        if let MsgType::Like = msgtype {
                        } else {
                            for i in &v {
                                if i.rpid == rpid {
                                    let msg = format!("Duplicate Comment: {rpid}");
                                    output
                                        .send(Message::OfficialFetchingState(msg.clone()))
                                        .await
                                        .unwrap();
                                    pb.set_message(msg);
                                    continue 'outer;
                                }
                            }
                        }
                        let uri = i["item"]["uri"].as_str().unwrap();
                        let oid;

                        if uri.contains("t.bilibili.com") {
                            // 动态内评论
                            oid = match uri.replace("https://t.bilibili.com/", "").parse::<u64>() {
                                Ok(v) => v,
                                Err(e) => {
                                    output
                                        .send(Message::OfficialFetchingState(e.to_string()))
                                        .await
                                        .unwrap();
                                    error!(
                                        "Error json:{}\nError uri:{}\n{}",
                                        json,
                                        uri,
                                        e.to_string()
                                    );
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
                                    output
                                        .send(Message::OfficialFetchingState(e.to_string()))
                                        .await
                                        .unwrap();
                                    error!(
                                        "Error json:{}\nError uri:{}\n{}",
                                        json,
                                        uri,
                                        e.to_string()
                                    );
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
                                    output
                                        .send(Message::OfficialFetchingState(e.to_string()))
                                        .await
                                        .unwrap();
                                    error!(
                                        "Error json:{}\nError uri:{}\n{}",
                                        json,
                                        uri,
                                        e.to_string()
                                    );
                                    continue;
                                }
                            };
                            r#type = 12;
                        } else if uri.contains("https://www.bilibili.com/video/") {
                            // 视频内评论
                            oid = match oid_regex
                                .captures(i["item"]["native_uri"].as_str().unwrap())
                            {
                                Some(v) => match v.get(1).unwrap().as_str().parse::<u64>() {
                                    Ok(v) => v,
                                    Err(e) => {
                                        output
                                            .send(Message::OfficialFetchingState(e.to_string()))
                                            .await
                                            .unwrap();
                                        error!(
                                            "Error json:{}\nError uri:{}\n{}",
                                            json,
                                            uri,
                                            e.to_string()
                                        );
                                        continue;
                                    }
                                },
                                None => {
                                    output
                                        .send(Message::OfficialFetchingState(
                                            "Can't captures by the oid_regex var".to_string(),
                                        ))
                                        .await
                                        .unwrap();
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
                                    output
                                        .send(Message::OfficialFetchingState(
                                            "The subject_id field is null".to_string(),
                                        ))
                                        .await
                                        .unwrap();
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
                            info!("Undefined URI:{}\nSkip this comment: {}", uri, rpid);
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
                        v.push(Comment {
                            oid,
                            r#type,
                            rpid,
                            content: content.clone(),
                            remove_state: true,
                            notify_id: Some(notify_id),
                            tp: match msgtype {
                                MsgType::Like => Some(0),
                                MsgType::Reply => Some(1),
                                MsgType::At => Some(2),
                            },
                        });
                        let msg = format!("Push Comment: {}, Vec counts now: {}", rpid, v.len());
                        pb.set_message(msg.clone());
                        output
                            .send(Message::OfficialFetchingState(msg))
                            .await
                            .unwrap();
                        pb.tick();
                        // info!("Push Comment: {rpid}");
                        // info!("Vec Counts:{}", v.len());
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
            output
                .send(Message::CommentsFetched(Arc::new(Mutex::new(v))))
                .await
                .unwrap();
        })
    }

    pub fn fetch_comment_both(cl: Arc<Client>) -> impl Stream<Item = Message> {
        stream::channel(10, |mut output| async move {
            let mut a = Box::pin(fetch_comment_from_aicu(Arc::clone(&cl)));
            let mut v1 = None;
            while let Some(v) = a.next().await {
                match v {
                    Message::CommentsFetched(v) => v1 = Some(v),
                    _ => output.send(v).await.unwrap(),
                }
            }
            let mut b = Box::pin(fetch_comment(Arc::clone(&cl)));
            let mut v2 = None;
            while let Some(v) = b.next().await {
                match v {
                    Message::CommentsFetched(v) => v2 = Some(v),
                    _ => output.send(v).await.unwrap(),
                }
            }

            let mut seen_ids = HashSet::new();
            {
                let mut v1_locked = v1.as_ref().unwrap().lock().await;
                v1_locked.retain(|e| seen_ids.insert(e.rpid));

                let v2_locked = v2.as_ref().unwrap().lock().await;
                v2_locked.iter().for_each(|item| {
                    if seen_ids.insert(item.rpid) {
                        v1_locked.push(item.clone());
                    }
                });
            }
            output
                .send(Message::CommentsFetched(v1.unwrap()))
                .await
                .unwrap();
        })
    }
}

pub fn main_subscription() -> Subscription<Message> {
    Subscription::run(|| {
        stream::channel(100, |mut output| async move {
            let (sender, mut receiver) = mpsc::channel(100);
            output
                .send(Message::ChannelConnected(sender))
                .await
                .unwrap();
            let qrcode_refresh_flag = Arc::new(AtomicBool::new(false));
            let delete_flag = Arc::new(AtomicBool::new(true));
            let mut delete_task: Option<JoinHandle<()>> = None;

            loop {
                // 处理消息接收
                if let Some(msg) = receiver.recv().await {
                    match msg {
                        ChannelMsg::DeleteComment(cl, csrf, c, seconds) => {
                            delete_flag.store(true, Ordering::SeqCst);

                            let comments = c
                                .lock()
                                .await
                                .iter()
                                .filter(|e| e.remove_state)
                                .cloned()
                                .collect::<Vec<_>>();

                            if comments.is_empty() {
                                continue;
                            }

                            // 如果已有删除任务正在执行，检查任务是否完成
                            if let Some(handle) = delete_task.take() {
                                if !handle.is_finished() {
                                    handle.abort();
                                    info!("已有删除任务正在进行，已中止");
                                }
                            }

                            // 启动新的删除任务
                            let delete_flag_clone = Arc::clone(&delete_flag);
                            let mut output_clone = output.clone();
                            delete_task = Some(spawn(async move {
                                let len = comments.len();
                                let pb = ProgressBar::new(len as u64);
                                pb.set_style(
                                    indicatif::ProgressStyle::with_template(
                                        "{wide_bar} {pos}/{len} {msg}",
                                    )
                                    .unwrap(),
                                );

                                for (index, comment) in comments.iter().enumerate() {
                                    if !delete_flag_clone.load(Ordering::SeqCst) {
                                        output_clone
                                            .send(Message::AllCommentDeleted)
                                            .await
                                            .unwrap();
                                        info!("删除操作已中止");
                                        break;
                                    }

                                    let cl_clone = Arc::clone(&cl);
                                    let csrf_clone = Arc::clone(&csrf);
                                    match comment.remove(cl_clone, csrf_clone).await {
                                        Ok(rpid) => {
                                            output_clone
                                                .send(Message::CommentDeleted { rpid })
                                                .await
                                                .unwrap();
                                            pb.set_message(format!("已删除评论：{}", rpid));
                                            pb.inc(1);
                                        }
                                        Err(err) => {
                                            error!("{}", err);
                                        }
                                    }

                                    if index + 1 == len {
                                        output_clone
                                            .send(Message::AllCommentDeleted)
                                            .await
                                            .unwrap();
                                        pb.finish_with_message("删除完成");
                                    }

                                    sleep(Duration::from_secs_f32(seconds)).await;
                                }
                            }));
                        }
                        ChannelMsg::StopDelete => {
                            delete_flag.store(false, Ordering::SeqCst);
                            info!("停止删除评论");
                        }
                        ChannelMsg::StartRefreshQRcodeState => {
                            qrcode_refresh_flag.store(true, Ordering::SeqCst);
                            let qrcode_refresh_flag_clone = Arc::clone(&qrcode_refresh_flag);
                            let mut output_clone = output.clone();
                            spawn(async move {
                                while qrcode_refresh_flag_clone.load(Ordering::SeqCst) {
                                    output_clone.send(Message::QRcodeRefresh).await.unwrap();
                                    sleep(Duration::from_secs(1)).await;
                                }
                            });
                        }
                        ChannelMsg::StopRefreshQRcodeState => {
                            qrcode_refresh_flag.store(false, Ordering::SeqCst);
                        }
                    }
                } else {
                    panic!("Channel is closed");
                }
            }
        })
    })
}
