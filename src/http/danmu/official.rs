use crate::http::danmu::Danmu;
use crate::types::Result;
use iced::futures::stream::{iter, try_unfold, unfold};
use iced::futures::{pin_mut, stream, FutureExt, Stream, StreamExt, TryStreamExt};
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::join;
use tokio::sync::Mutex;
use tracing::info;

#[derive(Deserialize, Debug)]
struct ApiResponse {
    data: Option<Data>,
}

#[derive(Deserialize, Debug)]
struct Data {
    total: Total,
}

#[derive(Deserialize, Debug)]
struct Total {
    cursor: Cursor,
    items: Vec<Item>,
}

#[derive(Deserialize, Debug)]
struct Cursor {
    is_end: bool,
    id: u64,
}

#[derive(Deserialize, Debug)]
struct Item {
    id: u64,
    item: ItemDetails,
    like_time: u64,
}

#[derive(Deserialize, Debug)]
struct ItemDetails {
    item_id: u64,
    #[serde(rename = "type")]
    item_type: String,
    title: String,
    native_uri: String,
}

async fn fetch_page(client: Arc<Client>, url: &str) -> Result<ApiResponse> {
    Ok(client.get(url).send().await?.json::<ApiResponse>().await?)
}

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
            dbg!(&url);
            let client = client.clone();
            async move {
                let url = url.unwrap();
                let response = fetch_page(client, &url).await?;
                let data = response.data.ok_or("无数据")?;

                let mut results = vec![];
                let mut last_time = None;
                let mut query_id = None;

                for item in data.total.items {
                    if item.item.item_type == "danmu" {
                        if let Some(cid) = extract_cid(&item.item.native_uri) {
                            results.push((item.item.item_id, Danmu::new(item.item.title, cid)));
                        }
                    }
                    last_time = Some(item.like_time);
                }

                if data.total.cursor.is_end {
                    return Ok(None);
                }

                query_id = Some(data.total.cursor.id);
                let next_url = format!(
                "https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web&id={}&like_time={}",
                query_id.unwrap(), last_time.unwrap()
            );

                Ok(Some((results, Some(next_url))))
            }
        },
    )
}

fn create_replyed_danmu_stream(
    client: Arc<Client>,
) -> impl Stream<Item = Result<Vec<(u64, Danmu)>>> {
    try_unfold(
        Some(String::from(
            "https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web",
        )),
        move |url| {
            dbg!(&url);
            let client = client.clone();
            async move {
                let url = url.unwrap();
                let response = fetch_page(client, &url).await?;
                let data = response.data.ok_or("无数据")?;

                let mut results = vec![];
                let mut last_time = None;
                let mut query_id = None;

                for item in data.total.items {
                    if item.item.item_type == "danmu" {
                        if let Some(cid) = extract_cid(&item.item.native_uri) {
                            results.push((item.item.item_id, Danmu::new(item.item.title, cid)));
                        }
                    }
                    last_time = Some(item.like_time);
                }

                if data.total.cursor.is_end {
                    return Ok(None);
                }

                query_id = Some(data.total.cursor.id);
                let next_url = format!(
                    "https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web&id={}&like_time={}",
                    query_id.unwrap(), last_time.unwrap()
                );

                Ok(Some((results, Some(next_url))))
            }
        },
    )
}

fn create_ated_danmu_stream(client: Arc<Client>) -> impl Stream<Item = Result<Vec<(u64, Danmu)>>> {
    try_unfold(
        Some(String::from(
            "https://api.bilibili.com/x/msgfeed/at?build=0&mobi_app=web",
        )),
        move |url| {
            dbg!(&url);
            let client = client.clone();
            async move {
                let url = url.unwrap();
                let response = fetch_page(client, &url).await?;
                let data = response.data.ok_or("无数据")?;

                let mut results = vec![];
                let mut last_time = None;
                let mut query_id = None;

                for item in data.total.items {
                    if item.item.item_type == "danmu" {
                        if let Some(cid) = extract_cid(&item.item.native_uri) {
                            results.push((item.item.item_id, Danmu::new(item.item.title, cid)));
                        }
                    }
                    last_time = Some(item.like_time);
                }

                if data.total.cursor.is_end {
                    return Ok(None);
                }

                query_id = Some(data.total.cursor.id);
                let next_url = format!(
                    "https://api.bilibili.com/x/msgfeed/at?build=0&mobi_app=web&id={}&like_time={}",
                    query_id.unwrap(),
                    last_time.unwrap()
                );

                Ok(Some((results, Some(next_url))))
            }
        },
    )
}

pub async fn fetch(client: Arc<Client>) -> Result<Arc<Mutex<HashMap<u64, Danmu>>>> {
    let mut map = Arc::new(Mutex::new(HashMap::new()));

    let stream = create_liked_danmu_stream(client.clone())
        .chain(create_replyed_danmu_stream(client.clone()))
        .chain(create_ated_danmu_stream(client.clone()));
    stream
        .try_for_each_concurrent(3, |e| {
            let map_cloned = map.clone();
            async move {
                for (item_id, danmu) in e {
                    info!("Fetched danmu {item_id}");
                    map_cloned.lock().await.insert(item_id, danmu);
                }
                Ok(())
            }
        })
        .await?;
    info!("弹幕处理完毕。弹幕数量：{}", map.lock().await.len());

    Ok(map)
}
