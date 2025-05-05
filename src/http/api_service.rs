use crate::{types::Result, UA};
use reqwest::{header, Client, IntoUrl, Response};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tracing::debug;

#[derive(Debug)]
pub struct ApiService {
    client: Client,
    csrf: String,
}

impl Default for ApiService {
    fn default() -> Self {
        Self {
            client: Client::builder().default_headers({
                let mut headers = header::HeaderMap::new();
                headers.insert("User-Agent", header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36 Edg/127.0.2651.86"));
                headers
            }).cookie_store(true).build().unwrap(),
            csrf: String::new(),
        }
    }
}

impl ApiService {
    pub fn new(ck: String) -> Self {
        let a = ck.find("bili_jct=").unwrap();
        let b = ck[a..].find(";").unwrap();
        let csrf = ck[a + 9..b + a].to_string();

        let mut headers = header::HeaderMap::new();
        headers.insert(header::USER_AGENT, header::HeaderValue::from_static(UA));
        headers.insert(header::COOKIE, header::HeaderValue::from_str(&ck).unwrap());
        let client = Client::builder()
            .default_headers(headers)
            .cookie_store(true)
            .build().unwrap();

        Self { client, csrf }
    }

    pub fn new_with_fields(client: Client, csrf: String) -> Self {
        Self { client, csrf }
    }

    // 获取内部的Client
    pub fn client(&self) -> &Client {
        &self.client
    }

    // 获取CSRF令牌
    pub fn csrf(&self) -> &str {
        &self.csrf
    }

    // 发送GET请求并返回JSON响应
    pub async fn get_json<T: IntoUrl>(&self, url: T) -> Result<Value> {
        let res: Value = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        debug!("Got res: {}", res);
        Ok(res)
    }

    // 发送GET请求并反序列化为指定类型
    pub async fn fetch_data<T: DeserializeOwned>(&self, url: impl IntoUrl) -> Result<T> {
        let res = self.client.get(url).send().await?.error_for_status()?;
        debug!("{:?}", &res);
        Ok(res.json().await?)
    }

    // 发送POST请求（JSON格式）
    pub async fn post_json<T: IntoUrl>(&self, url: T, json_data: &Value) -> Result<Response> {
        Ok(self.client.post(url).json(json_data).send().await?)
    }

    // 发送POST请求（表单格式）
    pub async fn post_form<T: IntoUrl>(
        &self,
        url: T,
        form_data: &[(&str, String)],
    ) -> Result<Response> {
        Ok(self.client.post(url).form(form_data).send().await?)
    }

    // 获取用户ID
    pub async fn get_uid(&self) -> Result<u64> {
        let json_res = self
            .get_json("https://api.bilibili.com/x/member/web/account")
            .await?;
        let uid = json_res["data"]["mid"].as_u64().unwrap();
        Ok(uid)
    }
}
