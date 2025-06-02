use super::api_service::ApiService;
use super::comment::official::parse_oid;
use super::comment::{self, Comment};
use super::danmu::official::extract_cid;
use super::danmu::{self, Danmu};
use super::response::official::{
    ContinuationSystemNotifyApiResponse, InitialSystemNotifyApiResponse, SystemNotifyItem,
};
use crate::http::response::official::{like, reply};
use crate::screens::main;
use crate::types::{
    AtedRecovery, Error, FetchProgressState, LikedRecovery, Message, RemoveAble, ReplyedRecovery,
    Result, SystemNotifyRecovery,
};
use iced::Task;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rand::Rng;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

fn sleep_duration() -> Duration {
    // 随机延迟1到2秒
    let milliseconds = rand::rng().random_range(1000..2000);
    Duration::from_millis(milliseconds)
}

#[derive(Clone, Debug)]
pub struct Notify {
    pub content: String,
    tp: u8,
    pub is_selected: bool,
    /// 删除系统通知的两种api
    system_notify_api: Option<u8>,
}
impl Notify {
    pub fn new(content: String, tp: u8) -> Notify {
        Notify {
            content,
            tp,
            is_selected: true,
            system_notify_api: None,
        }
    }

    fn new_system_notify(content: String, tp: u8, api_type: u8) -> Notify {
        Notify {
            content,
            tp,
            is_selected: true,
            system_notify_api: Some(api_type),
        }
    }
}
impl RemoveAble for Notify {
    async fn remove(&self, id: u64, api: Arc<ApiService>) -> Result<u64> {
        match self.system_notify_api {
            Some(api_type) => {
                let csrf = api.csrf();
                let json = if api_type == 0 {
                    json!({"csrf":csrf,"ids":[id],"station_ids":[],"type":self.tp,"build":8140300,"mobi_app":"android"})
                } else {
                    json!({"csrf":csrf,"ids":[],"station_ids":[id],"type":self.tp,"build":8140300,"mobi_app":"android"})
                };
                let url = format!("https://message.bilibili.com/x/sys-msg/del_notify_list?build=8140300&mobi_app=android&csrf={csrf}");
                let json_res: Value = api.post_json(url, &json).await?.json().await?;
                if json_res["code"].as_i64().unwrap() == 0 {
                    Ok(id)
                } else {
                    Err(Error::DeleteSystemNotifyError(json_res.into()))
                }
            }
            None => {
                let form_data = [
                    ("tp", self.tp.to_string()),
                    ("id", id.to_string()),
                    ("build", 0.to_string()),
                    ("mobi_app", "web".to_string()),
                    ("csrf_token", api.csrf().to_string()),
                    ("csrf", api.csrf().to_string()),
                ];
                let json_res: Value = api
                    .post_form("https://api.bilibili.com/x/msgfeed/del", &form_data)
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                if json_res["code"].as_i64().unwrap() == 0 {
                    Ok(id)
                } else {
                    Err(Error::DeleteNotifyError(json_res.into()))
                }
            }
        }
    }
}

pub async fn fetch(
    api: Arc<ApiService>,
    aicu_state: bool,
    mut progress_state: FetchProgressState,
) -> Result<(
    Option<
        Arc<(
            HashMap<u64, Notify>,
            HashMap<u64, Comment>,
            HashMap<u64, Danmu>,
        )>,
    >, // The fully aggregated data (might be none if interrupted)
    Option<FetchProgressState>, // The updated state to be saved by the caller
)> {
    // --- Liked ---
    if progress_state.liked_recovery.is_some() || progress_state.liked_data.0.is_empty() {
        // Fetch if recovering or not fetched at all
        info!("Fetching liked data (resuming if applicable)...");
        let (n, c, d, recovery) = fetch_liked(
            api.clone(),
            std::mem::take(&mut progress_state.liked_data.0),
            std::mem::take(&mut progress_state.liked_data.1),
            std::mem::take(&mut progress_state.liked_data.2),
            progress_state.liked_recovery.clone(), // Pass current recovery point
        )
        .await?; // Propagate unrecoverable errors from fetch_liked itself (e.g. programming error)

        progress_state.liked_data = (n, c, d);
        progress_state.liked_recovery = recovery;

        if progress_state.liked_recovery.is_some() {
            info!("Liked data fetching interrupted. Saving progress.");
            return Ok((None, Some(progress_state)));
        }
        info!("Liked data fetched completely.");
    } else {
        info!("Skipping liked data, already fetched.");
    }
    // --- Replyed ---
    if progress_state.replyed_recovery.is_some() || progress_state.replyed_data.0.is_empty() {
        info!("Fetching replyed data (resuming if applicable)...");

        let (n, c, recovery) = fetch_replyed(
            api.clone(),
            std::mem::take(&mut progress_state.replyed_data.0),
            std::mem::take(&mut progress_state.replyed_data.1),
            progress_state.replyed_recovery.clone(),
        )
        .await?;
        progress_state.replyed_data = (n, c);
        progress_state.replyed_recovery = recovery;
        if progress_state.replyed_recovery.is_some() {
            info!("Replyed data fetching interrupted. Saving progress.");
            return Ok((None, Some(progress_state)));
        }
        info!("Replyed data fetched completely.");
    } else {
        info!("Skipping replyed data, already fetched.");
    }

    // --- Ated ---
    if progress_state.ated_recovery.is_some() || progress_state.ated_data.is_empty() {
        info!("Fetching ated data (resuming if applicable)...");

        let (n, recovery) = fetch_ated(
            api.clone(),
            std::mem::take(&mut progress_state.ated_data),
            progress_state.ated_recovery.clone(),
        )
        .await?;
        progress_state.ated_data = n;
        progress_state.ated_recovery = recovery;
        if progress_state.ated_recovery.is_some() {
            info!("Ated data fetching interrupted. Saving progress.");
            return Ok((None, Some(progress_state)));
        }
        info!("Ated data fetched completely.");
    } else {
        info!("Skipping ated data, already fetched.");
    }

    // --- System Notify ---
    if progress_state.system_notify_recovery.is_some()
        || progress_state.system_notify_data.is_empty()
    {
        info!("Fetching system notify (resuming if applicable)...");

        let (n, recovery) = fetch_system_notify_adapted(
            api.clone(),
            std::mem::take(&mut progress_state.system_notify_data),
            progress_state.system_notify_recovery.clone(),
        )
        .await?;
        progress_state.system_notify_data = n;
        progress_state.system_notify_recovery = recovery;
        if progress_state.system_notify_recovery.is_some() {
            info!("System notify fetching interrupted. Saving progress.");
            return Ok((None, Some(progress_state)));
        }
        info!("System notify fetched completely.");
    } else {
        info!("Skipping system notify, already fetched.");
    }

    // --- AICU Data ---
    if aicu_state {
        // --- AICU Comments ---
        // Fetch if:
        // 1. Recovery point exists (was interrupted).
        // 2. No data AND no recovery point (fresh start for this data type).
        // 3. AICU was disabled in the last run but is enabled now (need to fetch fresh).
        if progress_state.aicu_comment_recovery.is_some()
            || (progress_state.aicu_comment_data.is_empty()
                && progress_state.aicu_comment_recovery.is_none())
            || !progress_state.aicu_enabled_last_run
        {
            info!("Fetching/Resuming AICU comments...");
            let current_aicu_comments = std::mem::take(&mut progress_state.aicu_comment_data);
            let recovery_opt = progress_state.aicu_comment_recovery.clone(); // Clone recovery_point

            match comment::aicu::fetch_adapted(api.clone(), current_aicu_comments, recovery_opt)
                .await
            {
                Ok((c, r)) => {
                    progress_state.aicu_comment_data = c;
                    progress_state.aicu_comment_recovery = r;
                    progress_state.aicu_enabled_last_run = true;
                }
                Err(e) => {
                    if let Error::GetUIDError(_) = e {
                        warn!("{}", e);
                        info!("AICU comment fetching interrupted. Saving progress.");
                        return Ok((None, Some(progress_state)));
                    }
                    return Err(e);
                }
            }
            if progress_state.aicu_comment_recovery.is_some() {
                info!("AICU comment fetching interrupted. Saving progress.");
                return Ok((None, Some(progress_state)));
            }
            info!("AICU comments fetched completely.");
        } else {
            info!("Skipping AICU comments (already fetched or not meeting fetch criteria).");
        }

        // --- AICU Danmus ---
        // Same logic as for comments
        if progress_state.aicu_danmu_recovery.is_some()
            || (progress_state.aicu_danmu_data.is_empty()
                && progress_state.aicu_danmu_recovery.is_none())
            || !progress_state.aicu_enabled_last_run
        {
            info!("Fetching/Resuming AICU danmus...");
            let current_aicu_danmus = std::mem::take(&mut progress_state.aicu_danmu_data);
            let recovery_opt = progress_state.aicu_danmu_recovery.clone();

            match danmu::aicu::fetch_adapted(api.clone(), current_aicu_danmus, recovery_opt).await {
                Ok((c, r)) => {
                    progress_state.aicu_danmu_data = c;
                    progress_state.aicu_danmu_recovery = r;
                    progress_state.aicu_enabled_last_run = true;
                }
                Err(e) => {
                    if let Error::GetUIDError(_) = e {
                        warn!("{}", e);
                        info!("AICU danmu fetching interrupted. Saving progress.");
                        return Ok((None, Some(progress_state)));
                    }
                    return Err(e);
                }
            }

            if progress_state.aicu_danmu_recovery.is_some() {
                info!("AICU danmu fetching interrupted. Saving progress.");
                return Ok((None, Some(progress_state)));
            }
            info!("AICU danmus fetched completely.");
        } else {
            info!("Skipping AICU danmus (already fetched or not meeting fetch criteria).");
        }

        // If aicu_state is true, and we've gone through the fetching logic (even if skipped),
        // then for the *next* run, aicu_enabled_last_run should reflect that AICU was enabled in *this* run.
    } else {
        // aicu_state is false
        if progress_state.aicu_enabled_last_run {
            // AICU was enabled in the last run but is now disabled.
            // Clear out the data and recovery points to prevent stale data/resumption attempts.
            info!(
                "AICU is now disabled. Clearing AICU data and recovery points from progress state."
            );
            progress_state.aicu_comment_data.clear();
            progress_state.aicu_comment_recovery = None;
            progress_state.aicu_danmu_data.clear();
            progress_state.aicu_danmu_recovery = None;
        }
        progress_state.aicu_enabled_last_run = false;
    }

    let (agg_n, agg_c, agg_d) = aggregate_data_from_state(&progress_state, aicu_state);
    Ok((Some(Arc::new((agg_n, agg_c, agg_d))), None))
}

pub fn fetch_task(
    api: Arc<ApiService>,
    aicu_state: bool,
    progress_state: FetchProgressState,
) -> Task<Message> {
    Task::perform(fetch(api, aicu_state, progress_state), |e| {
        Message::Main(main::Message::Fetched(e))
    })
}

// Helper to aggregate data (outside fetch or as a private fn)
fn aggregate_data_from_state(
    state: &FetchProgressState,
    aicu_enabled: bool,
) -> (
    HashMap<u64, Notify>,
    HashMap<u64, Comment>,
    HashMap<u64, Danmu>,
) {
    let mut combined_notify = HashMap::new();
    combined_notify.extend(state.liked_data.0.clone());
    combined_notify.extend(state.replyed_data.0.clone());
    combined_notify.extend(state.ated_data.clone());
    combined_notify.extend(state.system_notify_data.clone());

    let mut combined_comment = HashMap::new();
    combined_comment.extend(state.liked_data.1.clone());
    combined_comment.extend(state.replyed_data.1.clone());
    if aicu_enabled {
        combined_comment.extend(state.aicu_comment_data.clone());
    }

    let mut combined_danmu = HashMap::new();
    combined_danmu.extend(state.liked_data.2.clone());
    if aicu_enabled {
        combined_danmu.extend(state.aicu_danmu_data.clone());
    }

    (combined_notify, combined_comment, combined_danmu)
}

pub async fn fetch_liked(
    api: Arc<ApiService>,
    mut current_notify_data: HashMap<u64, Notify>,
    mut current_comment_data: HashMap<u64, Comment>,
    mut current_danmu_data: HashMap<u64, Danmu>,
    recovery_point: Option<LikedRecovery>,
) -> Result<(
    HashMap<u64, Notify>,
    HashMap<u64, Comment>,
    HashMap<u64, Danmu>,
    Option<LikedRecovery>, // Some if interrupted, None if completed
)> {
    let mut cursor_id = recovery_point.as_ref().map(|r| r.cursor_id);
    let mut cursor_time = recovery_point.as_ref().map(|r| r.cursor_time);

    let mp = MultiProgress::new();
    let pb_notify = mp.add(ProgressBar::new_spinner());
    pb_notify.set_style(
        ProgressStyle::with_template("[{elapsed_precise}] {spinner:.green} {msg}").unwrap(),
    );
    pb_notify.enable_steady_tick(Duration::from_millis(100));
    pb_notify.set_message(format!(
        "Liked notify. Counts: {}",
        current_notify_data.len()
    ));

    let pb_comment = mp.add(ProgressBar::new_spinner());
    pb_comment.set_style(
        ProgressStyle::with_template("[{elapsed_precise}] {spinner:.green} {msg}").unwrap(),
    );
    pb_comment.enable_steady_tick(Duration::from_millis(100));
    pb_comment.set_message(format!(
        "Liked comment. Counts: {}",
        current_comment_data.len()
    ));

    let pb_danmu = mp.add(ProgressBar::new_spinner());
    pb_danmu
        .set_style(ProgressStyle::with_template("[{elapsed_precise}] {spinner} {msg}").unwrap());
    pb_danmu.enable_steady_tick(Duration::from_millis(100));
    pb_danmu.set_message(format!("Liked danmu. Counts: {}", current_danmu_data.len()));

    loop {
        let api_call_result = if cursor_id.is_none() && cursor_time.is_none() {
            // 第一次请求
            api.fetch_data::<like::ApiResponse>(
                "https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web",
            )
            .await
        } else {
            api.fetch_data::<like::ApiResponse>(
                format!("https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web&id={}&like_time={}",
                cursor_id.unwrap(), // Safe due to check
                cursor_time.unwrap()) // Safe due to check
            )
            .await
        };

        match api_call_result {
            Ok(response_data) => {
                let res = response_data.data.total; // Assuming this is the structure

                let new_cursor_id = res.cursor.as_ref().map(|c| c.id);
                let new_cursor_time = res.cursor.as_ref().map(|c| c.time);

                // if let Some(c) = &api_call_result.cursor {
                //     cursor_id = Some(c.id);
                //     cursor_time = Some(c.time);
                // } else {
                //     return Ok((
                //         current_notify_data,
                //         current_comment_data,
                //         current_danmu_data,
                //     ));
                // }
                if res.items.is_empty()
                    && (res.cursor.is_none() || res.cursor.as_ref().map_or(false, |c| c.is_end))
                {
                    info!("被点赞的通知处理完毕");
                    return Ok((
                        current_notify_data,
                        current_comment_data,
                        current_danmu_data,
                        None,
                    )); // None for recovery means completed
                }

                for item in res.items {
                    // notify
                    current_notify_data.insert(
                        item.id,
                        Notify::new(
                            format!(
                                "{} ({})",
                                item.item.nested.title, item.item.nested.item_type
                            ),
                            0,
                        ),
                    );
                    pb_notify.set_message(format!(
                        "Fetched liked notify: {}. Counts now: {}",
                        item.id,
                        current_notify_data.len()
                    ));

                    //comment
                    let i = item.item;
                    if i.nested.item_type == "reply" {
                        let rpid = i.item_id;
                        match parse_oid(&i.nested) {
                            Ok((oid, r#type)) => {
                                let content = i.nested.title.clone();
                                let notify_id = item.id;
                                current_comment_data.insert(
                                    rpid,
                                    Comment::new_with_notify(oid, r#type, content, notify_id, 0),
                                );
                                pb_comment.set_message(format!(
                                    "Fetched liked comment: {rpid}. Counts now: {}",
                                    current_comment_data.len()
                                ));
                            }
                            Err(e) => {
                                warn!("{:?}", e);
                            }
                        }
                    }

                    //danmu
                    if i.nested.item_type == "danmu" {
                        if let Some(cid) = extract_cid(&i.nested.native_uri) {
                            current_danmu_data.insert(
                                i.item_id,
                                Danmu::new_with_notify(i.nested.title, cid, item.id),
                            );
                            pb_danmu.set_message(format!(
                                "Fetched liked danmu: {}. Counts now: {}",
                                i.item_id,
                                current_comment_data.len()
                            ));
                        }
                    }
                }
                cursor_id = new_cursor_id; // Update cursors for the next loop iteration
                cursor_time = new_cursor_time;

                if res.cursor.as_ref().map_or(true, |c| c.is_end) {
                    // No cursor or is_end
                    info!("被点赞的通知处理完毕。");
                    return Ok((
                        current_notify_data,
                        current_comment_data,
                        current_danmu_data,
                        None,
                    ));
                }
            }
            Err(e) => {
                // This is an error during an API call for a page.
                // We return the data fetched *so far* and the *current* cursors to resume from.
                warn!(
                    "Error fetching liked page: {:?}. Will attempt to resume later.",
                    e
                );
                let recovery = if let (Some(id), Some(time)) = (cursor_id, cursor_time) {
                    Some(LikedRecovery {
                        cursor_id: id,
                        cursor_time: time,
                    })
                } else {
                    // If error on the very first fetch, recovery is from the beginning (None)
                    // Or, if you always want to pass the *next* intended cursor,
                    // this logic depends on whether cursor_id/time were updated *before* the error.
                    // For simplicity, let's say we try to resume with the cursors we *intended* to use for the failed call.
                    // If it was the first call, recovery_point was None.
                    recovery_point.clone()
                };
                return Ok((
                    current_notify_data,
                    current_comment_data,
                    current_danmu_data,
                    recovery,
                ));
            }
        }
        sleep(sleep_duration()).await;
    }
}

pub async fn fetch_replyed(
    api: Arc<ApiService>,
    mut current_notify_data: HashMap<u64, Notify>,
    mut current_comment_data: HashMap<u64, Comment>,
    recovery_point: Option<ReplyedRecovery>,
) -> Result<(
    HashMap<u64, Notify>,
    HashMap<u64, Comment>,
    Option<ReplyedRecovery>,
)> {
    let mut cursor_id = recovery_point.as_ref().map(|r| r.cursor_id);
    let mut cursor_time = recovery_point.as_ref().map(|r| r.cursor_time);

    let mp = MultiProgress::new();
    let pb_notify = mp.add(ProgressBar::new_spinner());
    pb_notify.set_style(
        ProgressStyle::with_template("[{elapsed_precise}] {spinner:.green} {msg}").unwrap(),
    );
    pb_notify.enable_steady_tick(Duration::from_millis(100));
    pb_notify.set_message(format!(
        "Replyed notify. Counts: {}",
        current_notify_data.len()
    ));

    let pb_comment = mp.add(ProgressBar::new_spinner());
    pb_notify.set_style(
        ProgressStyle::with_template("[{elapsed_precise}] {spinner:.green} {msg}").unwrap(),
    );
    pb_notify.enable_steady_tick(Duration::from_millis(100));
    pb_comment.set_message(format!(
        "Replyed comment. Counts: {}",
        current_comment_data.len()
    ));

    loop {
        let api_call_result = if cursor_id.is_none() && cursor_time.is_none() {
            // 第一次请求
            api.fetch_data::<reply::ApiResponse>(
                "https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web",
            )
            .await
        } else {
            api.fetch_data::<reply::ApiResponse>(
                format!("https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web&id={}&reply_time={}",
                cursor_id.unwrap(),
                cursor_time.unwrap())
            )
            .await
        };

        match api_call_result {
            Ok(response_data) => {
                let res = response_data.data;
                let new_cursor_id = res.cursor.as_ref().map(|c| c.id);
                let new_cursor_time = res.cursor.as_ref().map(|c| c.time);

                if res.items.is_empty() && res.cursor.as_ref().map_or(true, |c| c.is_end)
                // No cursor or is_end
                {
                    info!("被评论的通知处理完毕。");
                    return Ok((current_notify_data, current_comment_data, None));
                }

                for item in res.items {
                    // notify
                    current_notify_data.insert(
                        item.id,
                        Notify::new(
                            format!(
                                "{} ({})",
                                item.item.nested.title, item.item.nested.item_type
                            ),
                            1,
                        ),
                    );
                    pb_notify.set_message(format!(
                        "Fetched replyed notify: {}. Counts now: {}",
                        item.id,
                        current_notify_data.len()
                    ));
                    // comment
                    let i = item.item;
                    if i.nested.item_type == "reply" {
                        let rpid = i.target_id;
                        match parse_oid(&i.nested) {
                            Ok((oid, r#type)) => {
                                let content = match i.target_reply_content {
                                    Some(v) if !v.is_empty() => v,
                                    Some(_) => i.nested.title,
                                    None => i.nested.title,
                                };
                                let notify_id = item.id;
                                current_comment_data.insert(
                                    rpid,
                                    Comment::new_with_notify(oid, r#type, content, notify_id, 1),
                                );
                                pb_comment.set_message(format!(
                                    "Fetched replyed comment: {rpid}. Counts now: {}",
                                    current_comment_data.len()
                                ));
                            }
                            Err(e) => {
                                warn!("{:?}", e);
                            }
                        }
                    }
                }
                cursor_id = new_cursor_id;
                cursor_time = new_cursor_time;

                if res.cursor.as_ref().map_or(true, |c| c.is_end) {
                    // No cursor or is_end
                    info!("被评论的通知处理完毕。");
                    return Ok((current_notify_data, current_comment_data, None));
                }
            }
            Err(e) => {
                warn!(
                    "Error fetching replyed page: {:?}. Will attempt to resume later.",
                    e
                );
                let recovery = if let (Some(id), Some(time)) = (cursor_id, cursor_time) {
                    Some(ReplyedRecovery {
                        cursor_id: id,
                        cursor_time: time,
                    })
                } else {
                    recovery_point.clone()
                };
                return Ok((current_notify_data, current_comment_data, recovery));
            }
        }
        sleep(sleep_duration()).await;
    }
}

pub async fn fetch_ated(
    api: Arc<ApiService>,
    mut current_notify_data: HashMap<u64, Notify>,
    recovery_point: Option<AtedRecovery>,
) -> Result<(HashMap<u64, Notify>, Option<AtedRecovery>)> {
    let mut cursor_id = recovery_point.as_ref().map(|r| r.cursor_id);
    let mut cursor_time = recovery_point.as_ref().map(|r| r.cursor_time);

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("[{elapsed_precise}] {spinner:.green} {msg}").unwrap(),
    );
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_message(format!(
        "Ated notify. Counts: {}",
        current_notify_data.len()
    ));
    loop {
        let api_call_result = if cursor_id.is_none() && cursor_time.is_none() {
            // 第一次请求
            api.fetch_data::<reply::ApiResponse>(
                "https://api.bilibili.com/x/msgfeed/at?build=0&mobi_app=web",
            )
            .await
        } else {
            api.fetch_data::<reply::ApiResponse>(format!(
                "https://api.bilibili.com/x/msgfeed/at?build=0&mobi_app=web&id={}&at_time={}",
                cursor_id.unwrap(),
                cursor_time.unwrap()
            ))
            .await
        };

        match api_call_result {
            Ok(response_data) => {
                let res = response_data.data;
                let new_cursor_id = res.cursor.as_ref().map(|c| c.id);
                let new_cursor_time = res.cursor.as_ref().map(|c| c.time);
                let cursor_is_end_or_none = res.cursor.as_ref().map_or(true, |c| c.is_end);
                if res.items.is_empty() && cursor_is_end_or_none {
                    info!("被At的通知处理完毕。");
                    break;
                }

                for i in res.items {
                    current_notify_data.insert(
                        i.id,
                        Notify::new(
                            format!("{} ({})", i.item.nested.title, i.item.nested.item_type),
                            2,
                        ),
                    );
                    pb.set_message(format!(
                        "Fetched ated notify: {}. Counts now: {}",
                        i.id,
                        current_notify_data.len()
                    ));
                }

                cursor_id = new_cursor_id;
                cursor_time = new_cursor_time;

                if cursor_is_end_or_none {
                    info!("被At的通知处理完毕。");
                    break;
                }
            }
            Err(e) => {
                warn!(
                    "Error fetching ated page: {:?}. Will attempt to resume later.",
                    e
                );
                let recovery = if let (Some(id), Some(time)) = (cursor_id, cursor_time) {
                    Some(AtedRecovery {
                        cursor_id: id,
                        cursor_time: time,
                    })
                } else {
                    recovery_point.clone()
                };
                return Ok((current_notify_data, recovery));
            }
        }
        sleep(sleep_duration()).await;
    }
    Ok((current_notify_data, None))
}

// pub async fn fetch_system_notify(api: Arc<ApiService>) -> Result<HashMap<u64, Notify>> {
//     let mut h: HashMap<u64, Notify> = HashMap::new();
//     let mut cursor = None;
//     let mut api_type = 0_u8;
//     let pb = ProgressBar::new_spinner();

//     loop {
//         let mut json: Value;
//         let mut notifys: &Value;
//         // first get
//         if cursor.is_none() {
//             json = api.get_json(
//                 format!("https://message.bilibili.com/x/sys-msg/query_user_notify?csrf={}&csrf={}&page_size=20&build=0&mobi_app=web",
//                 api.csrf(), api.csrf()),
//             )
//             .await?;
//             notifys = &json["data"]["system_notify_list"];
//             // 第一种api（0）获取为空时
//             if notifys.is_null() {
//                 api_type = 1;
//                 json = api.get_json(
//                     format!("https://message.bilibili.com/x/sys-msg/query_unified_notify?csrf={}&csrf={}&page_size=10&build=0&mobi_app=web",
//                     api.csrf(), api.csrf())
//                 ).await?;
//                 notifys = &json["data"]["system_notify_list"];
//                 // 两者都为空
//                 if notifys.is_null() {
//                     let i = "没有系统通知。";
//                     warn!("{}", i);
//                     return Ok(h);
//                 }
//             }
//             cursor = notifys.as_array().unwrap().last().unwrap()["cursor"].as_u64();
//         } else {
//             let url =
//                 format!("https://message.bilibili.com/x/sys-msg/query_notify_list?csrf={}&data_type=1&cursor={}&build=0&mobi_app=web",
//                 api.csrf(), cursor.unwrap());
//             json = api.get_json(url).await?;
//             notifys = &json["data"];
//             if json["data"].as_array().unwrap().is_empty() {
//                 info!("系统通知处理完毕。通知数量：{}", h.len());
//                 break;
//             }
//             cursor = notifys.as_array().unwrap().last().unwrap()["cursor"].as_u64();
//         }

//         for i in notifys.as_array().unwrap() {
//             let notify_id = i["id"].as_u64().unwrap();
//             let notify_type = i["type"].as_u64().unwrap() as u8;
//             h.insert(
//                 notify_id,
//                 Notify::new_system_notify(
//                     format!(
//                         "{}\n{}",
//                         i["title"].as_str().unwrap(),
//                         i["content"].as_str().unwrap()
//                     ),
//                     notify_type,
//                     api_type,
//                 ),
//             );
//             pb.set_message(format!(
//                 "Fetched system notify: {}. Counts now: {}",
//                 notify_id,
//                 h.len()
//             ));
//             pb.tick();
//         }
//         sleep(sleep_duration()).await;
//     }
//     Ok(h)
// }

pub async fn fetch_system_notify_adapted(
    api: Arc<ApiService>,
    mut h: HashMap<u64, Notify>,                  // Pass in current data
    recovery_point: Option<SystemNotifyRecovery>, // Pass in recovery state
) -> Result<(
    HashMap<u64, Notify>,         // Return updated data
    Option<SystemNotifyRecovery>, // Return new recovery state if interrupted, None if complete
)> {
    let mut current_cursor: Option<u64> = recovery_point.as_ref().map(|r| r.cursor);
    // api_type determines which initial endpoint to try if current_cursor is None,
    // and potentially which continuation logic if they differ (though they seem to converge to query_notify_list).
    // Default to 0 if no recovery point, or use the recovered api_type.
    let mut api_type_to_use: u8 = recovery_point.as_ref().map_or(0, |r| r.api_type);

    let pb = ProgressBar::new_spinner();
    // Consider pb.inc_length if you have an estimated total, or just use as spinner.
    pb.set_message(format!("SysNotify: Start. Count: {}", h.len()));

    loop {
        let mut items_on_this_page: Vec<SystemNotifyItem> = Vec::new();
        let mut new_page_cursor: Option<u64> = None; // Cursor for the *next* page

        let api_call_result: Result<Value> = if current_cursor.is_none() {
            // First fetch attempt (or resumed first attempt)
            pb.set_message(format!(
                "SysNotify: Initial fetch (type {}). Count: {}",
                api_type_to_use,
                h.len()
            ));
            let initial_url = if api_type_to_use == 0 {
                format!(
                    "https://message.bilibili.com/x/sys-msg/query_user_notify?csrf={}&page_size=20&build=0&mobi_app=web",
                    api.csrf() // Assuming csrf is still needed and api.csrf() is cheap/cached
                )
            } else {
                format!(
                    "https://message.bilibili.com/x/sys-msg/query_unified_notify?csrf={}&page_size=10&build=0&mobi_app=web",
                    api.csrf()
                )
            };
            api.get_json(&initial_url).await // get_json returns Result<Value>
        } else {
            // Paginated fetch
            pb.set_message(format!(
                "SysNotify: Fetching page (cursor {}). Count: {}",
                current_cursor.unwrap_or(0),
                h.len()
            ));
            let continuation_url = format!(
                "https://message.bilibili.com/x/sys-msg/query_notify_list?csrf={}&data_type=1&cursor={}&build=0&mobi_app=web",
                api.csrf(),
                current_cursor.unwrap() // Safe because current_cursor.is_some() here
            );
            api.get_json(&continuation_url).await
        };

        match api_call_result {
            Ok(json_value) => {
                if current_cursor.is_none() {
                    // Parsing initial response
                    match serde_json::from_value::<InitialSystemNotifyApiResponse>(
                        json_value.clone(),
                    ) {
                        Ok(parsed_response) => {
                            if let Some(data_obj) = parsed_response.data {
                                if let Some(item_list) = data_obj.system_notify_list {
                                    items_on_this_page = item_list;
                                }
                            }
                            // If items_on_this_page is still empty and api_type_to_use was 0, try api_type 1
                            if items_on_this_page.is_empty() && api_type_to_use == 0 {
                                warn!("SysNotify: API type 0 (user_notify) returned no items. Trying API type 1 (unified_notify).");
                                api_type_to_use = 1; // Switch for the next iteration (which will be an initial fetch again)
                                current_cursor = None; // Ensure it retries initial fetch logic
                                sleep(sleep_duration()).await;
                                continue; // Retry loop for API type 1
                            }
                        }
                        Err(e) => {
                            warn!(
                                "SysNotify: Failed to parse initial JSON: {:?}. JSON: {}",
                                e,
                                json_value.to_string()
                            );
                            let recovery = current_cursor.map(|c| SystemNotifyRecovery {
                                cursor: c,
                                api_type: api_type_to_use,
                            });
                            return Ok((h, recovery)); // Return current data and recovery point
                        }
                    }
                } else {
                    // Parsing continuation response
                    match serde_json::from_value::<ContinuationSystemNotifyApiResponse>(
                        json_value.clone(),
                    ) {
                        Ok(parsed_response) => {
                            if let Some(item_list) = parsed_response.data {
                                items_on_this_page = item_list;
                            }
                        }
                        Err(e) => {
                            warn!(
                                "SysNotify: Failed to parse continuation JSON: {:?}. JSON: {}",
                                e,
                                json_value.to_string()
                            );
                            let recovery = current_cursor.map(|c| SystemNotifyRecovery {
                                cursor: c,
                                api_type: api_type_to_use,
                            });
                            return Ok((h, recovery));
                        }
                    }
                }

                if items_on_this_page.is_empty() {
                    info!(
                        "SysNotify: No more items found. Processing complete. Total: {}",
                        h.len()
                    );
                    pb.finish_with_message(format!("SysNotify: Done. Total: {}", h.len()));
                    return Ok((h, None)); // None for recovery means completed
                }

                for item_struct in items_on_this_page {
                    h.insert(
                        item_struct.id,
                        Notify::new_system_notify(
                            format!("{}\n{}", item_struct.title, item_struct.content),
                            item_struct.item_type as u8, // Assuming item_type fits in u8
                            api_type_to_use, // The API type that successfully fetched these items
                        ),
                    );
                    new_page_cursor = Some(item_struct.cursor); // The cursor from the last item is for the next page
                    pb.tick(); // Or set message with current count
                    pb.set_message(format!(
                        "SysNotify: Added {}. Count: {}",
                        item_struct.id,
                        h.len()
                    ));
                }
                current_cursor = new_page_cursor; // Set cursor for the next iteration
            }
            Err(e) => {
                warn!(
                    "SysNotify: API call failed: {:?}. Will attempt to resume later.",
                    e
                );
                // Construct recovery point based on the cursor we *intended* to use or the last successful one
                let recovery = current_cursor.map(|c| SystemNotifyRecovery {
                    cursor: c,
                    api_type: api_type_to_use,
                });
                pb.abandon_with_message(format!("SysNotify: Error. Count: {}", h.len()));
                return Ok((h, recovery)); // Return current data and recovery point
            }
        }

        if current_cursor.is_none() {
            // This can happen if an initial fetch (user_notify or unified_notify)
            // returned items but no cursor on the last item, or if items_on_this_page was empty
            // and it wasn't handled by the "no more items" break.
            // This implies the API might not be strictly paginated or we reached the end.
            info!("SysNotify: current_cursor became None after processing a page. Assuming completion. Total: {}", h.len());
            pb.finish_with_message(format!(
                "SysNotify: Done (cursor became none). Total: {}",
                h.len()
            ));
            return Ok((h, None)); // Completed
        }

        sleep(sleep_duration()).await;
    }
}
