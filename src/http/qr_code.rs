use crate::http::utility::get_json;
use crate::types::Result;
use reqwest::Client;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct QRdata {
    pub url: String,
    pub key: String,
}
impl QRdata {
    pub async fn request_qrcode() -> Result<QRdata> {
        let a = get_json(
            Arc::new(Client::new()),
            "https://passport.bilibili.com/x/passport-login/web/qrcode/generate",
        )
        .await?;
        Ok(QRdata {
            url: a["data"]["url"].as_str().unwrap().to_string(),
            key: a["data"]["qrcode_key"].as_str().unwrap().to_string(),
        })
    }
    pub async fn get_state(&self, cl: Arc<Client>) -> Result<(u64, Option<String>)> {
        let url = format!(
            "https://passport.bilibili.com/x/passport-login/web/qrcode/poll?qrcode_key={}",
            &self.key
        );
        let res = get_json(cl, &url).await?;
        let res_code = res["data"]["code"].as_u64().unwrap();
        if res_code == 0 {
            let res_url = res["data"]["url"].as_str().unwrap();
            let a = res_url.find("bili_jct=").expect("Can't find csrf data.");
            let b = res_url[a..].find("&").unwrap();
            let csrf = res_url[a + 9..b + a].to_string();
            return Ok((res_code, Some(csrf)));
        }
        Ok((res_code, None))
    }
}
