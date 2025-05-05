use crate::types::Result;
use reqwest::{header, Client, IntoUrl};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, info};

pub async fn get_json<T: IntoUrl>(cl: Arc<Client>, url: T) -> Result<Value> {
    let res: Value = cl.get(url).send().await?.error_for_status()?.json().await?;
    debug!("Got res: {}", res);
    Ok(res)
}

pub async fn get_uid(cl: Arc<Client>) -> Result<u64> {
    let json_res = get_json(cl, "https://api.bilibili.com/x/member/web/account").await?;
    let uid = json_res["data"]["mid"]
        .as_u64()
        .unwrap();
    info!(
        "Got uid: {uid}  I found u, {} 😎",
        json_res["data"]["uname"].as_str().unwrap()
    );
    Ok(uid)
}

pub async fn fetch_data<T: DeserializeOwned>(cl: Arc<Client>, url: impl IntoUrl) -> Result<T> {
    let res = cl.get(url).send().await?.error_for_status()?;
    debug!("{:?}", &res);
    Ok(res.json().await?)
}

pub mod video_info {
    use crate::types::Result;
    use reqwest::Client;
    use serde::Deserialize;
    use std::sync::Arc;
    pub async fn get_cid(cl: Arc<Client>, av: u64) -> Result<Option<u64>> {
        Ok(cl
            .get("https://api.bilibili.com/x/player/pagelist")
            .query(&[("aid", av)])
            .send()
            .await?
            .error_for_status()?
            .json::<PageList>()
            .await?
            .data
            .map(|e| e[0].cid))
    }
    #[derive(Deserialize)]
    struct PageList {
        data: Option<Vec<Item>>,
    }
    #[derive(Deserialize)]
    struct Item {
        cid: u64,
        // snip
    }
}

pub async fn create_client(ck: String) -> Result<(Client, String)> {
    let a = ck
        .find("bili_jct=")
        .unwrap();
    let b = ck[a..]
        .find(";")
        .unwrap();
    let csrf = &ck[a + 9..b + a];

    let mut headers = header::HeaderMap::new();
    headers.insert(header::USER_AGENT, header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36 Edg/127.0.2651.86"));
    headers.insert(header::COOKIE, header::HeaderValue::from_str(&ck).unwrap());
    let cl = Client::builder()
        .default_headers(headers)
        .cookie_store(true)
        .build()?;

    Ok((cl, csrf.to_string()))
}
