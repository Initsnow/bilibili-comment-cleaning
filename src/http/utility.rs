use crate::types::Result;
use reqwest::{header, Client};
use std::sync::Arc;
use tracing::info;
use crate::http::api_service::ApiService;

pub mod video_info {
    use crate::types::Result;
    use serde::Deserialize;
    use std::sync::Arc;
    use crate::http::api_service::ApiService;
    
    pub async fn get_cid(api: Arc<ApiService>, av: u64) -> Result<Option<u64>> {
        let url = format!("https://api.bilibili.com/x/player/pagelist?aid={}", av);
        Ok(api.fetch_data::<PageList>(url)
            .await?
            .data
            .map(|e| e[0].cid))
    }
    
    #[derive(Deserialize)]
    pub struct PageList {
        pub data: Option<Vec<Item>>,
    }
    
    #[derive(Deserialize)]
    pub struct Item {
        pub cid: u64,
        // snip
    }
}

