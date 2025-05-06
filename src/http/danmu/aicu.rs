use crate::http::api_service::ApiService;
use crate::http::danmu::Danmu;
use crate::http::response::aicu::danmu::ApiResponse;
use crate::http::utility::video_info::get_cid;
use crate::types::Result;
use indicatif::ProgressBar;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::info;

pub async fn fetch(api: Arc<ApiService>) -> Result<Arc<Mutex<HashMap<u64, Danmu>>>> {
    let uid = api.get_uid().await?;
    let mut page = 1;
    let mut h = HashMap::new();

    let all_count = api
        .fetch_data::<ApiResponse>(format!(
            "https://api.aicu.cc/api/v3/search/getvideodm?uid={uid}&pn=1&ps=0&mode=0&keyword=",
        ))
        .await?
        .data
        .cursor
        .all_count;
    let pb = ProgressBar::new(all_count as u64);
    sleep(Duration::from_secs(1)).await;

    info!("正在从aicu.cc获取弹幕...");
    loop {
        let res = api.fetch_data::<ApiResponse>(
            format!(
                "https://api.aicu.cc/api/v3/search/getvideodm?uid={uid}&pn={page}&ps=500&mode=0&keyword="
            )
        ).await?.data;
        for i in res.videodmlist {
            let cid = get_cid(api.clone(), i.oid).await?;
            if let Some(cid) = cid {
                h.insert(i.id, Danmu::new(i.content, cid));
                pb.inc(1);
            }
        }
        if res.cursor.is_end {
            info!("Fetch successfully from aicu.cc");
            break;
        }
        page += 1;
        sleep(Duration::from_secs(3)).await;
    }
    Ok(Arc::new(Mutex::new(h)))
}
