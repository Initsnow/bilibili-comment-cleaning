use crate::http::danmu::Danmu;
use crate::http::response::aicu::danmu::ApiResponse;
use crate::types::Result;
use indicatif::ProgressBar;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::info;
use crate::http::api_service::ApiService;

pub async fn fetch(api: Arc<ApiService>) -> Result<Arc<Mutex<HashMap<u64, Danmu>>>> {
    let uid = api.get_uid().await?;
    let mut page = 1;
    let mut h = HashMap::new();

    let all_count =
        api.fetch_data::<ApiResponse>(
            format!(
                "https://api.aicu.cc/api/v3/search/getvideodm?uid={uid}&pn=1&ps=0&mode=0&keyword=",
            ),
        )
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
            // 获取cid的逻辑需要修改为使用ApiService
            let cid = api.fetch_data::<crate::http::utility::video_info::PageList>(
                format!("https://api.bilibili.com/x/player/pagelist?aid={}", i.oid)
            ).await?.data.map(|e| e[0].cid);
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
