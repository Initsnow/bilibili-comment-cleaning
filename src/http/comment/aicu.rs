// In your comment/aicu.rs or similar module

use crate::http::api_service::ApiService;
use crate::http::comment::Comment; // Assuming Comment::new(oid, type, message) exists
use crate::http::response::aicu::comment::ApiResponse as AicuCommentApiResponse; // Renamed for clarity
use crate::types::{AicuCommentRecovery, Error, Result}; // Your project's Result and Error types
use indicatif::ProgressBar;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
// tokio::sync::Mutex is removed from return type, as we return HashMap directly
use tokio::time::sleep;
use tracing::{info, warn}; // Added warn

pub async fn fetch_adapted(
    api: Arc<ApiService>,
    mut current_comment_data: HashMap<u64, Comment>,
    recovery_point: Option<AicuCommentRecovery>,
) -> Result<(HashMap<u64, Comment>, Option<AicuCommentRecovery>)> {
    let uid: u64;
    let mut current_page: u32;
    let all_count: u64;
    let pb: ProgressBar; // Declare ProgressBar here, initialize after knowing all_count

    if let Some(recovery) = recovery_point {
        info!(
            "Resuming AICU comment fetch for UID: {}, from page: {}, total known: {}",
            recovery.uid, recovery.page, recovery.all_count
        );
        uid = recovery.uid;
        current_page = recovery.page;
        all_count = recovery.all_count;
        pb = ProgressBar::new(all_count);
        pb.set_position(current_comment_data.len() as u64);
    } else {
        info!("Starting new AICU comment fetch.");
        uid = api.get_uid().await?;

        // Fetch initial all_count
        match api
            .fetch_data::<AicuCommentApiResponse>(&format!(
                "https://api.aicu.cc/api/v3/search/getreply?uid={}&pn=1&ps=0&mode=0&keyword=", // ps=0 to get count
                uid
            ))
            .await
        {
            Ok(response) => {
                all_count = response.data.cursor.all_count;
                if all_count == 0 {
                    info!("AICU: No comments found for UID: {}. Fetch complete.", uid);
                    return Ok((current_comment_data, None));
                }
                pb = ProgressBar::new(all_count);
                info!("AICU comments: Total count {} for UID: {}", all_count, uid);
            }
            Err(e) => {
                warn!(
                    "Failed to get initial all_count for AICU comments (UID: {}): {:?}",
                    uid, e
                );
                let recovery = AicuCommentRecovery {
                    uid,
                    page: 1,
                    all_count: 0,
                };
                return Ok((current_comment_data, Some(recovery)));
            }
        }
        current_page = 1; // Start from page 1 for a new fetch
                          // sleep(Duration::from_secs(1)).await; // Optional delay
    }

    info!(
        "Fetching AICU comments (UID: {}, Page: {}, Total: {})...",
        uid, current_page, all_count
    );

    loop {
        if current_comment_data.len() >= all_count as usize && all_count > 0 {
            info!("AICU comments: Fetched count ({}) meets or exceeds all_count ({}). Assuming completion.", current_comment_data.len(), all_count);
            pb.finish_with_message("AICU Comments: Fetch complete (count matched).");
            return Ok((current_comment_data, None));
        }

        let fetch_url = format!(
            "https://api.aicu.cc/api/v3/search/getreply?uid={}&pn={}&ps=500&mode=0&keyword=",
            uid, current_page
        );

        pb.set_message(format!(
            "AICU Comments: Page {}, Count: {}",
            current_page,
            current_comment_data.len()
        ));

        match api.fetch_data::<AicuCommentApiResponse>(&fetch_url).await {
            Ok(response) => {
                let data_segment = response.data;
                if data_segment.replies.is_empty() && !data_segment.cursor.is_end {
                    warn!("AICU comments: Page {} for UID {} was empty but cursor.is_end is false. Continuing...", current_page, uid);
                }

                for item in data_segment.replies {
                    // Assuming rpid is unique and suitable as a key.
                    // If a comment with this rpid already exists, this will overwrite it.
                    // This is usually fine if data from API is canonical for that rpid.
                    if !current_comment_data.contains_key(&item.rpid) {
                        // Ensure dyn and type are accessible. The original code used i.r#dyn.oid and i.r#dyn.r#type.
                        // Adjust if the structure of AicuCommentApiResponse.replies.item is different.
                        // Assuming item.r#dyn.oid and item.r#dyn.r#type are correct paths.
                        // Also ensure Comment::new takes these types.
                        // If item.r#dyn or item.r#dyn.r#type can be None, you'll need to handle that.
                        // For simplicity, assuming they are always present based on original code.
                        current_comment_data.insert(
                            item.rpid,
                            Comment::new(item.r#dyn.oid, item.r#dyn.r#type, item.message),
                        );
                        pb.inc(1);
                    }
                }

                if data_segment.cursor.is_end {
                    info!("AICU comments: Fetch successful from aicu.cc (UID: {}). Cursor indicates end.", uid);
                    pb.finish_with_message("AICU Comments: Fetch complete.");
                    return Ok((current_comment_data, None));
                }

                current_page += 1;
            }
            Err(e) => {
                warn!(
                    "AICU comments: API error fetching page {} for UID {}: {:?}. Will attempt to resume later.",
                    current_page, uid, e
                );
                let recovery = AicuCommentRecovery {
                    uid,
                    page: current_page,
                    all_count,
                };
                pb.abandon_with_message(format!("AICU Comments: API Error. Page {}", current_page));
                return Ok((current_comment_data, Some(recovery)));
            }
        }
        sleep(Duration::from_secs(3)).await; // API rate limiting
    }
}
