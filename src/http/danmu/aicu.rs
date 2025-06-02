// In your danmu/aicu.rs or similar module

use crate::http::api_service::ApiService;
use crate::http::danmu::Danmu; // Assuming Danmu::new(content, cid) exists
use crate::http::response::aicu::danmu::ApiResponse as AicuDanmuApiResponse; // Renamed for clarity
use crate::http::utility::video_info::get_cid;
use crate::types::{AicuDanmuRecovery, Error, Result}; // Your project's Result and Error types
use indicatif::ProgressBar;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn}; // Added warn

pub async fn fetch_adapted(
    api: Arc<ApiService>,
    mut current_danmu_data: HashMap<u64, Danmu>,
    recovery_point: Option<AicuDanmuRecovery>,
) -> Result<(HashMap<u64, Danmu>, Option<AicuDanmuRecovery>)> {
    let uid: u64;
    let mut current_page: u32;
    let all_count: u64;
    let pb: ProgressBar; // Declare ProgressBar here

    if let Some(recovery) = recovery_point {
        info!(
            "Resuming AICU danmu fetch for UID: {}, from page: {}, total known: {}",
            recovery.uid, recovery.page, recovery.all_count
        );
        uid = recovery.uid;
        current_page = recovery.page;
        all_count = recovery.all_count;
        pb = ProgressBar::new(all_count);
        pb.set_position(current_danmu_data.len() as u64); // Estimate position based on current data count
    } else {
        uid = api.get_uid().await?;

        // Fetch initial all_count
        match api
            .fetch_data::<AicuDanmuApiResponse>(&format!(
                "https://api.aicu.cc/api/v3/search/getvideodm?uid={}&pn=1&ps=0&mode=0&keyword=", // ps=0 to get count
                uid
            ))
            .await
        {
            Ok(response) => {
                all_count = response.data.cursor.all_count;
                if all_count == 0 {
                    info!("AICU: No danmus found for UID: {}. Fetch complete.", uid);
                    return Ok((current_danmu_data, None)); // None for recovery means completed
                }
                pb = ProgressBar::new(all_count);
                info!("AICU danmu: Total count {} for UID: {}", all_count, uid);
            }
            Err(e) => {
                warn!(
                    "Failed to get initial all_count for AICU danmus (UID: {}): {:?}",
                    uid, e
                );
                // Cannot proceed meaningfully. Return current data and a recovery point that signifies "try initial fetch again".
                // Or, if this is considered a fatal setup error, return Err.
                // For resumability, we'll make a recovery point that tries from page 1 again.
                let recovery = AicuDanmuRecovery {
                    uid,
                    page: 1,
                    all_count: 0,
                }; // all_count 0 indicates it needs re-fetching
                return Ok((current_danmu_data, Some(recovery)));
            }
        }
        current_page = 1; // Start from page 1 for a new fetch
                          // Brief sleep after initial count fetch if desired, though maybe not necessary here.
                          // sleep(Duration::from_secs(1)).await;
    }

    info!(
        "Fetching AICU danmus (UID: {}, Page: {}, Total: {})...",
        uid, current_page, all_count
    );

    loop {
        if current_danmu_data.len() >= all_count as usize && all_count > 0 {
            info!("AICU danmu: Fetched count ({}) meets or exceeds all_count ({}). Assuming completion.", current_danmu_data.len(), all_count);
            pb.finish_with_message("AICU Danmu: Fetch complete (count matched).");
            return Ok((current_danmu_data, None)); // Completed
        }

        let fetch_url = format!(
            "https://api.aicu.cc/api/v3/search/getvideodm?uid={}&pn={}&ps=500&mode=0&keyword=",
            uid, current_page
        );

        pb.set_message(format!(
            "AICU Danmu: Page {}, Count: {}",
            current_page,
            current_danmu_data.len()
        ));

        match api.fetch_data::<AicuDanmuApiResponse>(&fetch_url).await {
            Ok(response) => {
                let data_segment = response.data;
                if data_segment.videodmlist.is_empty() && !data_segment.cursor.is_end {
                    // This might indicate an issue or an empty page before the actual end.
                    // If the API guarantees is_end is reliable, this condition might not be strictly needed.
                    warn!("AICU danmu: Page {} for UID {} was empty but cursor.is_end is false. Continuing...", current_page, uid);
                }

                for item in data_segment.videodmlist {
                    // Avoid re-fetching cid if danmu item.id is already present,
                    // though AICU IDs should be unique.
                    if !current_danmu_data.contains_key(&item.id) {
                        match get_cid(api.clone(), item.oid.clone()).await {
                            // Clone oid if it's a String
                            Ok(Some(cid_val)) => {
                                current_danmu_data
                                    .insert(item.id, Danmu::new(item.content, cid_val));
                                pb.inc(1);
                            }
                            Ok(None) => {
                                warn!("AICU danmu: Could not find CID for OID: {} (Danmu ID: {}). Skipping.", item.oid, item.id);
                            }
                            Err(e) => {
                                warn!(
                                    "AICU danmu: Error fetching CID for OID: {} (Danmu ID: {}): {:?}. Skipping item, will retry page.",
                                    item.oid, item.id, e
                                );
                                // Error fetching a CID for an item. This is tricky.
                                // Option 1: Skip item (current behavior implicitly)
                                // Option 2: Abort page and retry this page later.
                                let recovery = AicuDanmuRecovery {
                                    uid,
                                    page: current_page,
                                    all_count,
                                };
                                pb.abandon_with_message(format!(
                                    "AICU Danmu: Error CID. Page {}",
                                    current_page
                                ));
                                return Ok((current_danmu_data, Some(recovery)));
                            }
                        }
                    }
                }

                if data_segment.cursor.is_end {
                    info!("AICU danmu: Fetch successful from aicu.cc (UID: {}). Cursor indicates end.", uid);
                    pb.finish_with_message("AICU Danmu: Fetch complete.");
                    return Ok((current_danmu_data, None)); // None for recovery means completed
                }

                current_page += 1;
            }
            Err(e) => {
                warn!(
                    "AICU danmu: API error fetching page {} for UID {}: {:?}. Will attempt to resume later.",
                    current_page, uid, e
                );
                let recovery = AicuDanmuRecovery {
                    uid,
                    page: current_page,
                    all_count,
                };
                pb.abandon_with_message(format!("AICU Danmu: API Error. Page {}", current_page));
                return Ok((current_danmu_data, Some(recovery))); // Return current data and recovery point
            }
        }
        sleep(Duration::from_secs(3)).await; // API rate limiting
    }
}
