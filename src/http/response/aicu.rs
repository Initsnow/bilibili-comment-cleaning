use serde::{de, Deserialize, Deserializer};

#[derive(Deserialize, Debug)]
pub struct Cursor {
    pub is_end: bool,
    pub all_count: u32,
}

fn string_to_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    s.parse::<u64>().map_err(de::Error::custom)
}

pub mod danmu {
    use super::string_to_u64;
    use super::Cursor;
    use serde::Deserialize;
    #[derive(Deserialize, Debug)]
    pub struct ApiResponse {
        pub data: Data,
    }
    #[derive(Deserialize, Debug)]
    pub struct Data {
        pub cursor: Cursor,
        pub videodmlist: Vec<Item>,
    }

    #[derive(Deserialize, Debug)]
    pub struct Item {
        #[serde(deserialize_with = "string_to_u64")]
        pub id: u64,
        pub content: String,
        #[serde(deserialize_with = "string_to_u64")]
        pub oid: u64,
    }
}

pub mod comment {
    use super::string_to_u64;
    use crate::http::response::aicu::Cursor;
    use serde::Deserialize;

    #[derive(Deserialize, Debug)]
    pub struct ApiResponse {
        pub data: Data,
    }
    #[derive(Deserialize, Debug)]
    pub struct Data {
        pub cursor: Cursor,
        pub replies: Vec<Item>,
    }
    #[derive(Deserialize, Debug)]
    pub struct Item {
        #[serde(deserialize_with = "string_to_u64")]
        pub rpid: u64,
        pub message: String,
        pub r#dyn: Dyn,
    }
    #[derive(Deserialize, Debug)]
    pub struct Dyn {
        #[serde(deserialize_with = "string_to_u64")]
        pub oid: u64,
        pub r#type: u8,
    }
}
