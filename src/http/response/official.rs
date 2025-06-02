use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Cursor {
    pub is_end: bool,
    pub id: u64,
    pub time: u64,
}
#[derive(Deserialize, Debug)]
pub struct NestedDetail {
    #[serde(rename = "type")]
    pub item_type: String,
    pub business_id: u8,
    pub title: String,
    pub uri: String,
    pub native_uri: String,
}
pub mod like {
    use super::{Cursor, NestedDetail};
    use serde::Deserialize;
    #[derive(Deserialize, Debug)]
    /// x/msgfeed/like
    pub struct ApiResponse {
        pub data: Data,
    }

    #[derive(Deserialize, Debug)]
    pub struct Data {
        pub total: Total,
    }

    #[derive(Deserialize, Debug)]
    pub struct Total {
        pub cursor: Option<Cursor>,
        pub items: Vec<Item>,
    }

    #[derive(Deserialize, Debug)]
    pub struct Item {
        pub id: u64,
        pub item: ItemDetails,
        pub like_time: u64,
    }

    #[derive(Deserialize, Debug)]
    pub struct ItemDetails {
        pub item_id: u64,
        #[serde(flatten)]
        pub nested: NestedDetail,
    }
}

pub mod reply {
    use super::{Cursor, NestedDetail};
    use serde::Deserialize;
    #[derive(Deserialize, Debug)]
    /// x/msgfeed/reply
    pub struct ApiResponse {
        pub data: Data,
    }
    #[derive(Deserialize, Debug)]
    pub struct Data {
        pub cursor: Option<Cursor>,
        pub items: Vec<Item>,
    }

    #[derive(Deserialize, Debug)]
    pub struct Item {
        /// notify_id
        pub id: u64,
        pub item: ItemDetails,
    }
    #[derive(Deserialize, Debug)]
    pub struct ItemDetails {
        pub subject_id: u64,
        pub target_id: u64,
        pub source_content: String,
        pub target_reply_content: Option<String>,
        #[serde(flatten)]
        pub nested: NestedDetail,
    }
}

pub mod at {
    use super::{Cursor, NestedDetail};
    use serde::Deserialize;
    #[derive(Deserialize, Debug)]
    /// x/msgfeed/at
    pub struct ApiResponse {
        pub data: Data,
    }
    #[derive(Deserialize, Debug)]
    pub struct Data {
        pub cursor: Option<Cursor>,
        pub items: Vec<Item>,
    }

    #[derive(Deserialize, Debug)]
    pub struct Item {
        /// notify_id
        pub id: u64,
        pub item: ItemDetails,
    }
    #[derive(Deserialize, Debug)]
    pub struct ItemDetails {
        pub subject_id: u64,
        pub target_id: u64,
        pub source_content: String,
        #[serde(flatten)]
        pub nested: NestedDetail,
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct SystemNotifyItem {
    pub id: u64,
    #[serde(rename = "type")] // "type" is a Rust keyword
    pub item_type: u64, // Bilibili API often uses u64 for types that could be smaller
    pub title: String,
    pub content: String,
    pub cursor: u64, // This cursor is present on each item, usually the same for a page,
                     // and the one from the last item is used for the next page.
}

// For the initial calls: query_user_notify, query_unified_notify
#[derive(Deserialize, Debug)]
pub struct InitialSystemNotifyData {
    pub system_notify_list: Option<Vec<SystemNotifyItem>>,
}

#[derive(Deserialize, Debug)]
pub struct InitialSystemNotifyApiResponse {
    // We might not need code/message if ApiService already handles non-zero codes
    // code: i32,
    // message: String,
    pub data: Option<InitialSystemNotifyData>,
}

// For the continuation call: query_notify_list
#[derive(Deserialize, Debug)]
pub struct ContinuationSystemNotifyApiResponse {
    // code: i32,
    // message: String,
    pub data: Option<Vec<SystemNotifyItem>>, // Here, data is directly the list
}
