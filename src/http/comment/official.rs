use crate::http::response::official::*;
use crate::types::{Error, Result};
use regex::Regex;
use std::sync::Arc;
use std::sync::LazyLock;
// use crate::http::api_service::ApiService;
// use crate::http::comment::Comment;
// use indicatif::ProgressBar;
// use std::collections::HashMap;
// use tokio::sync::Mutex;
// use tokio::try_join;
// use tracing::{info, warn};

static VIDEO_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"bilibili://video/(\d+)").unwrap());

pub fn parse_oid(detail: &NestedDetail) -> Result<(u64, u8)> {
    let uri = &detail.uri;
    let business_id = detail.business_id;
    let native_uri = &detail.native_uri;

    if uri.contains("t.bilibili.com") {
        // 动态内评论
        let oid = uri.replace("https://t.bilibili.com/", "").parse()?;
        // 我不知道半年前的我是怎么写出来这段神b代码的
        let tp = if business_id != 0 { business_id } else { 17 };
        Ok((oid, tp))
    } else if uri.contains("https://h.bilibili.com/ywh/") {
        // 带图动态内评论
        let oid = uri.replace("https://h.bilibili.com/ywh/", "").parse()?;
        return Ok((oid, 11));
    } else if uri.contains("https://www.bilibili.com/read/cv") {
        // 专栏内评论
        Ok((
            uri.replace("https://www.bilibili.com/read/cv", "")
                .parse()?,
            12,
        ))
    } else if uri.contains("https://www.bilibili.com/video/") {
        // 视频内评论
        let oid = VIDEO_REGEX
            .captures(native_uri)
            .unwrap()
            .get(1)
            .unwrap()
            .as_str()
            .parse()?;
        return Ok((oid, 1));
    } else if uri.contains("https://www.bilibili.com/bangumi/play/") {
        // 番剧（电影）内评论
        let oid = VIDEO_REGEX
            .captures(native_uri)
            .unwrap()
            .get(1)
            .unwrap()
            .as_str()
            .parse()?;
        return Ok((oid, 1));
    } else {
        Err(Error::UnrecognizedURI(Arc::new(uri.clone())))?
    }
}
// async fn fetch_liked(api: Arc<ApiService>) -> Result<HashMap<u64, Comment>> {
//     let mut h = HashMap::new();
//     let mut cursor_id = None;
//     let mut cursor_time = None;
//     let pb = ProgressBar::new_spinner();

//     loop {
//         let res = if cursor_id.is_none() && cursor_time.is_none() {
//             // 第一次请求
//             api.fetch_data::<like::ApiResponse>(
//                 "https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web",
//             )
//             .await?
//             .data
//             .total
//         } else {
//             api.fetch_data::<like::ApiResponse>(
//                 format!("https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web&id={}&like_time={}",
//                 cursor_id.unwrap(),
//                 cursor_time.unwrap())
//             )
//             .await?
//             .data
//             .total
//         };
//         if let Some(c) = &res.cursor {
//             cursor_id = Some(c.id);
//             cursor_time = Some(c.time);
//         } else {
//             return Ok(h);
//         }
//         for item in res.items {
//             let i = item.item;
//             if i.nested.item_type == "reply" {
//                 let rpid = i.item_id;
//                 match parse_oid(&i.nested) {
//                     Ok((oid, r#type)) => {
//                         let content = i.nested.title;
//                         let notify_id = item.id;
//                         h.insert(
//                             rpid,
//                             Comment::new_with_notify(oid, r#type, content, notify_id, 0),
//                         );
//                         pb.set_message(format!(
//                             "Fetched liked comment: {rpid}. Counts now: {}",
//                             h.len()
//                         ));
//                         pb.tick();
//                     }
//                     Err(e) => {
//                         warn!("{:?}", e);
//                     }
//                 }
//             }
//         }
//         if res.cursor.unwrap().is_end {
//             info!("被点赞的评论处理完毕。");
//             break;
//         }
//     }
//     Ok(h)
// }
// async fn fetch_replyed(api: Arc<ApiService>) -> Result<HashMap<u64, Comment>> {
//     let mut h = HashMap::new();
//     let mut cursor_id = None;
//     let mut cursor_time = None;
//     let pb = ProgressBar::new_spinner();

//     loop {
//         let res = if cursor_id.is_none() && cursor_time.is_none() {
//             // 第一次请求
//             api.fetch_data::<reply::ApiResponse>(
//                 "https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web",
//             )
//             .await?
//             .data
//         } else {
//             api.fetch_data::<reply::ApiResponse>(
//                 format!("https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web&id={}&reply_time={}",
//                 cursor_id.unwrap(),
//                 cursor_time.unwrap())
//             )
//             .await?
//             .data
//         };
//         if let Some(c) = &res.cursor {
//             cursor_id = Some(c.id);
//             cursor_time = Some(c.time);
//         } else {
//             return Ok(h);
//         }
//         for item in res.items {
//             let i = item.item;
//             if i.nested.item_type == "reply" {
//                 let rpid = i.target_id;
//                 match parse_oid(&i.nested) {
//                     Ok((oid, r#type)) => {
//                         let content = match i.target_reply_content {
//                             Some(v) if !v.is_empty() => v,
//                             Some(_) => i.nested.title,
//                             None => i.nested.title,
//                         };
//                         let notify_id = item.id;
//                         h.insert(
//                             rpid,
//                             Comment::new_with_notify(oid, r#type, content, notify_id, 1),
//                         );
//                         pb.set_message(format!(
//                             "Fetched replyed comment: {rpid}. Counts now: {}",
//                             h.len()
//                         ));
//                         pb.tick();
//                     }
//                     Err(e) => {
//                         warn!("{:?}", e);
//                     }
//                 }
//             }
//         }
//         if res.cursor.unwrap().is_end {
//             info!("被评论的评论处理完毕。");
//             break;
//         }
//     }
//     Ok(h)
// }

// pub async fn fetch(api: Arc<ApiService>) -> Result<Arc<Mutex<HashMap<u64, Comment>>>> {
//     let (mut h1, h2) = try_join!(fetch_liked(api.clone()), fetch_replyed(api.clone()))?;
//     h1.extend(h2.into_iter());
//     Ok(Arc::new(Mutex::new(h1)))
// }
