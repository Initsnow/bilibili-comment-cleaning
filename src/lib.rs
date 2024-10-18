use reqwest::{Client, IntoUrl};
use serde_json::Value;
use std::sync::Arc;

pub async fn get_json<T: IntoUrl>(cl: Arc<Client>, url: T) -> Value {
    let res = serde_json::from_str::<Value>(
        cl.get(url)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
            .as_str(),
    )
    .unwrap();
    // dbg!(&res);
    if res["code"] != 0 {
        panic!("Can't get request, Json response: {}", res);
    } else {
        res
    }
}
