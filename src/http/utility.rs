use crate::types::{ChannelMsg, Message, Result};
use reqwest::{header, Client, IntoUrl};
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
        .ok_or("Can't get uid. Please check your cookie data")?;
    info!(
        "Got uid: {uid}\nI found u, {} 😎",
        json_res["data"]["uname"]
    );
    Ok(uid)
}

pub async fn create_client(ck: String) -> Result<(Client, String)> {
    let a = ck
        .find("bili_jct=")
        .ok_or("Create Client Failed: Can't find csrf data. Make sure that your cookie data has a bili_jct field.")?;
    let b = ck[a..]
        .find(";")
        .ok_or("Create Client Failed: Can't find b")?;
    let csrf = &ck[a + 9..b + a];

    let mut headers = header::HeaderMap::new();
    headers.insert(header::USER_AGENT, header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36 Edg/127.0.2651.86"));
    headers.insert(header::COOKIE, header::HeaderValue::from_str(&ck).unwrap());
    let cl = Client::builder()
        .default_headers(headers)
        .cookie_store(true)
        .use_rustls_tls()
        .build()?;

    Ok((cl, csrf.to_string()))
}
