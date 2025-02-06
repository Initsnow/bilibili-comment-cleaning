use crate::http::danmu::Danmu;
use crate::http::response::official::like::ApiResponse;
use crate::http::utility::fetch_data;
use crate::types::Result;
use iced::futures::stream::try_unfold;
use iced::futures::{Stream, TryStreamExt};
use indicatif::ProgressBar;
use regex::Regex;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

fn extract_cid(native_uri: &str) -> Option<u64> {
    let re = Regex::new(r"cid=(\d+)").unwrap();
    re.captures(native_uri)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<u64>().ok())
}

fn create_liked_danmu_stream(client: Arc<Client>) -> impl Stream<Item = Result<Vec<(u64, Danmu)>>> {
    try_unfold(
        Some(String::from(
            "https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web",
        )),
        move |url| {
            let client = client.clone();
            async move {
                let url = url.unwrap();
                let response = fetch_data::<ApiResponse>(client, &url).await?;
                let data = response.data;

                let mut results = vec![];

                for item in data.total.items {
                    if item.item.nested.item_type == "danmu" {
                        if let Some(cid) = extract_cid(&item.item.nested.native_uri) {
                            results
                                .push((item.item.item_id, Danmu::new(item.item.nested.title, cid)));
                        }
                    }
                }

                if data.total.cursor.is_end {
                    return Ok(None);
                }

                let cursor_time = data.total.cursor.time;
                let cursor_id = data.total.cursor.id;

                let next_url = format!(
                    "https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web&id={}&like_time={}",
                    cursor_id, cursor_time
                );

                Ok(Some((results, Some(next_url))))
            }
        },
    )
}

pub async fn fetch(client: Arc<Client>) -> Result<Arc<Mutex<HashMap<u64, Danmu>>>> {
    let map = Arc::new(Mutex::new(HashMap::new()));
    let pb = ProgressBar::new_spinner();

    let stream = create_liked_danmu_stream(client.clone());
    stream
        .try_for_each_concurrent(3, |e| {
            let map_cloned = map.clone();
            let pb = pb.clone();
            async move {
                for (item_id, danmu) in e {
                    map_cloned.lock().await.insert(item_id, danmu);
                    pb.set_message(format!("Fetched danmu from official: {}.", item_id));
                    pb.tick();
                }
                Ok(())
            }
        })
        .await?;
    info!("被点赞的弹幕处理完毕。弹幕数量：{}", map.lock().await.len());

    Ok(map)
}
