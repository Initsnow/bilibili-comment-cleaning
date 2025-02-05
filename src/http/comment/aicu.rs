use crate::http::comment::Comment;
use crate::http::response::aicu::comment::ApiResponse;
use crate::http::utility::{fetch_data, get_uid};
use crate::types::Result;
use indicatif::ProgressBar;
use reqwest::{Client};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::info;

pub async fn fetch(cl: Arc<Client>) -> Result<Arc<Mutex<HashMap<u64, Comment>>>> {
    let uid = get_uid(Arc::clone(&cl)).await?;
    let mut page = 1;
    let mut h = HashMap::new();

    let all_count = fetch_data::<ApiResponse>(
        cl.clone(),
        format!("https://api.aicu.cc/api/v3/search/getreply?uid={uid}&pn=1&ps=0&mode=0&keyword=",),
    )
    .await?
    .data
    .cursor
    .all_count;
    let pb = ProgressBar::new(all_count as u64);
    sleep(Duration::from_secs(1)).await;

    info!("正在从aicu.cc获取评论...");
    loop {
        let res = fetch_data::<ApiResponse>(
            cl.clone(),
            format!(
            "https://api.aicu.cc/api/v3/search/getreply?uid={uid}&pn={page}&ps=500&mode=0&keyword="
        ),
        )
        .await?
        .data;
        for i in res.replies {
            h.insert(i.rpid, Comment::new(i.r#dyn.oid, i.r#dyn.r#type, i.message));
            pb.inc(1);
        }
        if res.cursor.is_end {
            pb.finish_with_message("Fetched successful from aicu.cc");
            break;
        }
        page += 1;
        sleep(Duration::from_secs(3)).await;
    }
    Ok(Arc::new(Mutex::new(h)))
}
