use reqwest::{Client, IntoUrl};
use serde_json::Value;
use std::sync::Arc;

pub async fn get_json<T: IntoUrl>(cl: Arc<Client>, url: T) -> Value {
    let res = serde_json::from_str::<Value>(
        cl.get(url)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
            .as_str(),
    )
    .unwrap();
    // dbg!(&res);
    if res["code"] != 0 {
        panic!("Can't get request, Json response: {}", res);
    } else {
        res
    }
}

pub mod notify {
    use crate::get_json;
    use reqwest::{Client, Url};
    use serde_json::json;
    use std::{sync::Arc, time::Duration};
    use tracing::{error, info, instrument};

    #[instrument(skip(cl, csrf))]
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

    #[instrument(skip(cl, csrf))]
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
