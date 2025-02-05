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
        pub cursor: Cursor,
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
        pub cursor: Cursor,
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
        pub cursor: Cursor,
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
