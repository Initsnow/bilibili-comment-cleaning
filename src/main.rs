use iced::stream;
use iced::widget::toggler;
use iced::{
    futures::SinkExt,
    widget::{
        button, center, checkbox, column, image, row, scrollable, text, text_input, Column, Space,
    },
    Alignment, Element, Length, Subscription, Task,
};
use indicatif::ProgressBar;
use regex::Regex;
use reqwest::Client;
use reqwest::{header, Url};
use serde_json::Value;
use std::collections::HashSet;
use std::{sync::Arc, time::Duration};
use tokio::spawn;
use tokio::sync::mpsc::{self, Sender};
use tracing::{error, info};

static HONGWEN: &[u8] = include_bytes!("assets/mysterious.jpg");
static TAFFY: &[u8] = include_bytes!("assets/taffy.png");

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    {
        let args: Vec<String> = std::env::args().collect();
        if args.len() == 3 {
            if args[1] == "--remove_notifys" {
                let cookie = args[2].clone();
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(fetch_remove_notifys(cookie));
            }
        }
    }

    let icon = iced::window::icon::from_file_data(TAFFY, None).unwrap();
    iced::application("BilibiliCommentCleaning", Main::update, Main::view)
        .window(iced::window::Settings {
            icon: Some(icon),
            size: (600.0, 500.0).into(),
            ..Default::default()
        })
        .subscription(Main::subscription)
        .run_with(Main::new)
}

#[derive(Debug, Default)]
struct Main {
    cookie: String,
    client: Arc<Client>,
    csrf: String,
    state: State,
    comments: Option<Vec<Comment>>,
    select_state: bool,
    aicu_state: bool,
    sender: Option<Sender<(Arc<Client>, String, Comment)>>,
}

#[derive(Debug, Default)]
enum State {
    #[default]
    WaitingForCookie,
    InitCompleted,
    CommentsFetched,
}

#[derive(Debug, Clone)]
enum Message {
    CookieSubmited(String),
    CookieInputChanged(String),
    ClientCreated { client: Client, csrf: String },
    CommentsFetched(Vec<Comment>),
    ChangeCommentRemoveState(u64, bool),
    CommentsSelectAll,
    CommentsDeselectAll,
    DeleteComment,
    CommentDeleted { rpid: u64 },
    CommentDeleteError(i64),
    ChannelConnected(Sender<(Arc<Client>, String, Comment)>),
    AicuToggle(bool),
}

impl Main {
    fn new() -> (Self, Task<Message>) {
        (Self::default(), Task::none())
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match self.state {
            State::WaitingForCookie => {
                match msg {
                    Message::CookieSubmited(s) => {
                        return Task::perform(create_client(s), move |m| m);
                    }
                    Message::CookieInputChanged(s) => {
                        self.cookie = s;
                    }
                    Message::ClientCreated { client, csrf } => {
                        self.client = Arc::new(client);
                        self.csrf = csrf;
                        self.state = State::InitCompleted;
                        if self.aicu_state {
                            return Task::perform(
                                fetch_comment_both(Arc::clone(&self.client)),
                                Message::CommentsFetched,
                            );
                        }
                        return Task::perform(
                            fetch_comment(Arc::clone(&self.client)),
                            Message::CommentsFetched,
                        );
                    }
                    Message::ChannelConnected(s) => {
                        self.sender = Some(s);
                    }
                    Message::AicuToggle(b) => {
                        self.aicu_state = b;
                    }
                    _ => {}
                }
                Task::none()
            }

            State::InitCompleted => {
                if let Message::CommentsFetched(comments) = msg {
                    self.comments = Some(comments);
                    self.state = State::CommentsFetched;
                }
                Task::none()
            }
            State::CommentsFetched => {
                match msg {
                    Message::ChangeCommentRemoveState(rpid, b) => {
                        for i in self.comments.as_mut().unwrap() {
                            if i.rpid == rpid {
                                i.remove_state = b;
                            }
                        }
                    }
                    Message::CommentsSelectAll => {
                        for i in self.comments.as_mut().unwrap() {
                            i.remove_state = true;
                        }
                        self.select_state = false;
                    }
                    Message::CommentsDeselectAll => {
                        for i in self.comments.as_mut().unwrap() {
                            i.remove_state = false;
                        }
                        self.select_state = true;
                    }
                    Message::DeleteComment => {
                        for i in self.comments.as_ref().unwrap() {
                            let sender = self.sender.as_ref().unwrap().clone();
                            let cl = Arc::clone(&self.client);
                            let csrf = self.csrf.clone();
                            let comment = i.clone();
                            if i.remove_state {
                                spawn(async move {
                                    sender.send((cl, csrf, comment)).await.unwrap();
                                });
                            }
                        }
                    }
                    Message::CommentDeleted { rpid } => {
                        self.comments.as_mut().unwrap().retain(|e| e.rpid != rpid);
                    }
                    _ => {}
                }
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::run(|| {
            stream::channel(100, |mut output| async move {
                let (sender, mut receiver) = mpsc::channel(100);
                output
                    .send(Message::ChannelConnected(sender))
                    .await
                    .unwrap();
                loop {
                    match receiver.recv().await {
                        Some(message) => {
                            info!("Channel Get: {}", message.2.rpid);
                            output
                                .send(remove_comment(message.0, message.1, message.2).await)
                                .await
                                .unwrap();
                        }
                        None => error!("Channel接收错误"),
                    }
                }
            })
        })
    }

    fn view(&self) -> Element<Message> {
        match self.state {
            State::WaitingForCookie => center(
                column![
                    row![
                        text_input("Input cookie here", &self.cookie)
                            .on_input(Message::CookieInputChanged)
                            .on_submit(Message::CookieSubmited(self.cookie.to_owned())),
                        button("enter").on_press(Message::CookieSubmited(self.cookie.to_owned())),
                    ]
                    .spacing(5),
                    toggler(
                        Some("Also fetch comments from aicu.cc".into()),
                        self.aicu_state,
                        Message::AicuToggle
                    )
                ]
                .spacing(5),
            )
            .padding(20)
            .into(),
            State::InitCompleted => center(
                column![
                    image(image::Handle::from_bytes(HONGWEN)).height(Length::FillPortion(2)),
                    text("Fetching").height(Length::FillPortion(1))
                ]
                .padding(20)
                .spacing(10)
                .align_x(Alignment::Center),
            )
            .into(),
            State::CommentsFetched => {
                if let Some(comments) = &self.comments {
                    let head = text(format!("There are currently {} comments", comments.len()));
                    let mut cl = Column::new().padding([0, 15]);
                    for i in comments {
                        cl = cl.push(
                            checkbox(i.content.to_owned(), i.remove_state)
                                .text_shaping(iced::widget::text::Shaping::Advanced)
                                .on_toggle(|b| Message::ChangeCommentRemoveState(i.rpid, b)),
                        );
                    }
                    let comments = center(scrollable(cl).height(Length::Fill));

                    let controls = row![
                        if self.select_state {
                            button("select all").on_press(Message::CommentsSelectAll)
                        } else {
                            button("deselect all").on_press(Message::CommentsDeselectAll)
                        },
                        Space::with_width(Length::Fill),
                        button("remove").on_press(Message::DeleteComment)
                    ]
                    .height(Length::Shrink)
                    .padding([0, 15]);

                    center(
                        column![head, comments, controls]
                            .spacing(10)
                            .align_x(Alignment::Center),
                    )
                    .padding([5, 20])
                    .into()
                } else {
                    center(text("任何邪恶，终将绳之以法😭").shaping(text::Shaping::Advanced)).into()
                }
            }
        }
    }
}

#[derive(Debug, Default, Clone)]

struct Comment {
    oid: u64,
    r#type: u8,
    rpid: u64,
    content: String,
    remove_state: bool,
    notify_id: Option<u64>,
    /// 删除通知用 0为收到赞的 1为收到评论的 2为被At的
    tp: Option<u8>,
}
async fn create_client(ck: String) -> Message {
    let a = ck
        .find("bili_jct=")
        .expect("Can't find csrf data.Make sure that your cookie data has a bili_jct field.");
    let b = ck[a..].find(";").unwrap();
    let csrf = &ck[a + 9..b + a];

    let mut headers = header::HeaderMap::new();
    headers.insert("User-Agent", header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36 Edg/127.0.2651.86"));
    headers.insert("Cookie", header::HeaderValue::from_str(&ck).unwrap());
    let cl = reqwest::Client::builder()
        .default_headers(headers)
        .cookie_store(true)
        .build()
        .unwrap();

    Message::ClientCreated {
        client: cl,
        csrf: csrf.to_string(),
    }
}

enum MsgType {
    Like,
    Reply,
    At,
}

async fn fetch_comment(cl: Arc<Client>) -> Vec<Comment> {
    let mut v: Vec<Comment> = Vec::new();
    let oid_regex = Regex::new(r"bilibili://video/(\d+)").unwrap();
    let mut msgtype = MsgType::Like;
    let mut queryid = None;
    let mut last_time = None;
    let pb = ProgressBar::new_spinner();
    loop {
        let json: serde_json::Value;
        let notifys: &serde_json::Value;
        if queryid.is_none() && last_time.is_none() {
            // 第一次请求
            let first = cl
                .get(
                    Url::parse(
                        match msgtype {
                            MsgType::Like=>"https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web",
                            MsgType::Reply=>"https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web",
                            MsgType::At=>"https://api.bilibili.com/x/msgfeed/at?build=0&mobi_app=web"
                        }
                    )
                    .unwrap(),
                )
                .send()
                .await
                .expect("Can't get first request");
            json = serde_json::from_str(&first.text().await.unwrap()).unwrap();
            if json["code"] != 0 {
                panic!("Can't get first request, Json response: {}", json);
            }
            match msgtype {
                MsgType::Like => {
                    notifys = &json["data"]["total"]["items"];
                    if notifys.as_array().unwrap().is_empty() {
                        msgtype = MsgType::Reply;
                        info!("没有收到赞的评论。");
                        continue;
                    }
                    last_time = notifys.as_array().unwrap().last().unwrap()["like_time"].as_u64();
                    queryid = json["data"]["total"]["cursor"]["id"].as_u64();
                }
                MsgType::Reply => {
                    notifys = &json["data"]["items"];
                    if notifys.as_array().unwrap().is_empty() {
                        msgtype = MsgType::At;
                        info!("没有收到评论的评论。");
                        continue;
                    }
                    last_time = notifys.as_array().unwrap().last().unwrap()["reply_time"].as_u64();
                    queryid = json["data"]["cursor"]["id"].as_u64();
                }
                MsgType::At => {
                    notifys = &json["data"]["items"];
                    if notifys.as_array().unwrap().is_empty() {
                        info!("没有被At的评论。");
                        break;
                    }
                    last_time = notifys.as_array().unwrap().last().unwrap()["at_time"].as_u64();
                    queryid = json["data"]["cursor"]["id"].as_u64();
                }
            }
        } else {
            let mut url = Url::parse(match msgtype {
                MsgType::Like => {
                    "https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web"
                }
                MsgType::Reply => {
                    "https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web"
                }
                MsgType::At => "https://api.bilibili.com/x/msgfeed/at?build=0&mobi_app=web",
            })
            .unwrap();
            match msgtype {
                MsgType::Like => {
                    url.query_pairs_mut()
                        .append_pair("id", &queryid.unwrap().to_string())
                        .append_pair("like_time", &last_time.unwrap().to_string());
                    let other = cl.get(url).send().await.expect("Can't get request");
                    json = serde_json::from_str(&other.text().await.unwrap()).unwrap();
                    notifys = &json["data"]["total"]["items"];
                    last_time = notifys.as_array().unwrap().last().unwrap()["like_time"].as_u64();
                    queryid = json["data"]["total"]["cursor"]["id"].as_u64();
                }
                MsgType::Reply => {
                    url.query_pairs_mut()
                        .append_pair("id", &queryid.unwrap().to_string())
                        .append_pair("reply_time", &last_time.unwrap().to_string());
                    let other = cl.get(url).send().await.expect("Can't get request");
                    json = serde_json::from_str(&other.text().await.unwrap()).unwrap();
                    notifys = &json["data"]["items"];
                    last_time = notifys.as_array().unwrap().last().unwrap()["reply_time"].as_u64();
                    queryid = json["data"]["cursor"]["id"].as_u64();
                }
                MsgType::At => {
                    url.query_pairs_mut()
                        .append_pair("id", &queryid.unwrap().to_string())
                        .append_pair("at_time", &last_time.unwrap().to_string());
                    let other = cl.get(url).send().await.expect("Can't get request");
                    json = serde_json::from_str(&other.text().await.unwrap()).unwrap();
                    notifys = &json["data"]["items"];
                    last_time = notifys.as_array().unwrap().last().unwrap()["at_time"].as_u64();
                    queryid = json["data"]["cursor"]["id"].as_u64();
                }
            }
        }
        // dbg!(queryid, last_time);
        let mut r#type: u8;
        'outer: for i in notifys.as_array().unwrap() {
            if i["item"]["type"] == "reply" {
                let rpid = if let MsgType::Like = msgtype {
                    i["item"]["item_id"].as_u64().unwrap()
                } else {
                    i["item"]["target_id"].as_u64().unwrap()
                };
                if let MsgType::Like = msgtype {
                } else {
                    for i in &v {
                        if i.rpid == rpid {
                            info!("Duplicate Comment: {rpid}");
                            continue 'outer;
                        }
                    }
                }
                let uri = i["item"]["uri"].as_str().unwrap();
                let oid;
                if uri.contains("t.bilibili.com") {
                    // 动态内评论
                    oid = uri
                        .replace("https://t.bilibili.com/", "")
                        .parse::<u64>()
                        .unwrap();
                    let business_id = i["item"]["business_id"].as_u64();
                    r#type = match business_id {
                        Some(v) if v != 0 => v as u8,
                        _ => 17,
                    };
                } else if uri.contains("https://h.bilibili.com/ywh/") {
                    // 带图动态内评论
                    oid = uri
                        .replace("https://h.bilibili.com/ywh/", "")
                        .parse::<u64>()
                        .unwrap();
                    r#type = 11;
                } else if uri.contains("https://www.bilibili.com/read/cv") {
                    // 专栏内评论
                    oid = uri
                        .replace("https://www.bilibili.com/read/cv", "")
                        .parse::<u64>()
                        .unwrap();
                    r#type = 12;
                } else if uri.contains("https://www.bilibili.com/video/") {
                    // 视频内评论
                    oid = oid_regex
                        .captures(i["item"]["native_uri"].as_str().unwrap())
                        .unwrap()
                        .get(1)
                        .unwrap()
                        .as_str()
                        .parse::<u64>()
                        .unwrap();
                    r#type = 1;
                } else if uri.contains("https://www.bilibili.com/bangumi/play/") {
                    // 电影（番剧？）内评论
                    oid = i["item"]["subject_id"].as_u64().unwrap();
                    r#type = 1;
                } else if uri.is_empty() {
                    info!("No URI, Skiped");
                    continue;
                } else {
                    panic!("Undefined URI:{}\nCan't get oid", uri);
                }
                let content = match msgtype {
                    MsgType::Like => i["item"]["title"].as_str().unwrap().to_string(),
                    MsgType::Reply => {
                        let v = i["item"]["target_reply_content"]
                            .as_str()
                            .unwrap()
                            .to_string();
                        if v.is_empty() {
                            i["item"]["title"].as_str().unwrap().to_string()
                        } else {
                            v
                        }
                    }
                    MsgType::At => {
                        format!("{}\n({})", i["item"]["source_content"], i["item"]["title"])
                    }
                };
                let notify_id = i["id"].as_u64().unwrap();
                v.push(Comment {
                    oid,
                    r#type,
                    rpid,
                    content: content.clone(),
                    remove_state: true,
                    notify_id: Some(notify_id),
                    tp: match msgtype {
                        MsgType::Like => Some(0),
                        MsgType::Reply => Some(1),
                        MsgType::At => Some(2),
                    },
                });
                pb.inc(1);
                pb.set_message(format!(
                    "Push Comment: {}, Vec counts now: {}",
                    rpid,
                    v.len()
                ));
                // info!("Push Comment: {rpid}");
                // info!("Vec Counts:{}", v.len());
            }
        }
        // push完检测是否为end
        match msgtype {
            MsgType::Like => {
                if json["data"]["total"]["cursor"]["is_end"].as_bool().unwrap() {
                    msgtype = MsgType::Reply;
                    last_time = None;
                    queryid = None;
                    info!("收到赞的评论处理完毕。");
                }
            }
            MsgType::Reply => {
                if json["data"]["cursor"]["is_end"].as_bool().unwrap() {
                    msgtype = MsgType::At;
                    last_time = None;
                    queryid = None;
                    info!("收到评论的评论处理完毕。");
                    continue;
                }
            }
            MsgType::At => {
                if json["data"]["cursor"]["is_end"].as_bool().unwrap() {
                    info!("被At的评论处理完毕。");
                    pb.finish_with_message("done");
                    break;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    v
}

async fn remove_comment(cl: Arc<Client>, csrf: String, i: Comment) -> Message {
    let res = if i.r#type == 11 {
        cl.post(format!(
            "https://api.bilibili.com/x/v2/reply/del?csrf={}",
            csrf.clone()
        ))
        .form(&[
            ("oid", i.oid.to_string()),
            ("type", i.r#type.to_string()),
            ("rpid", i.rpid.to_string()),
        ])
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
    } else {
        cl.post("https://api.bilibili.com/x/v2/reply/del")
            .form(&[
                ("oid", i.oid.to_string()),
                ("type", i.r#type.to_string()),
                ("rpid", i.rpid.to_string()),
                ("csrf", csrf.clone()),
            ])
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
    };
    let json_res: serde_json::Value = serde_json::from_str(res.as_str()).unwrap();
    if json_res["code"].as_i64().unwrap() == 0 {
        info!("Remove reply {} successful", i.rpid);
        // 如果is_some则删除通知
        if let Some(notify_id) = i.notify_id {
            remove_notify(cl, notify_id, csrf, i.tp.unwrap().to_string()).await;
        }
        Message::CommentDeleted { rpid: i.rpid }
    } else {
        error!("Can't remove comment. Response json: {}", json_res);
        Message::CommentDeleteError(json_res["code"].as_i64().unwrap())
    }
}

async fn get_uid(cl: Arc<Client>) -> u64 {
    let res = cl
        .get("https://api.bilibili.com/x/member/web/account")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let json_res: serde_json::Value = serde_json::from_str(res.as_str()).unwrap();
    json_res["data"]["mid"]
        .as_u64()
        .expect("Can't get uid. Please check your cookie data")
}
async fn fetch_comment_from_aicu(cl: Arc<Client>) -> Vec<Comment> {
    let uid = get_uid(Arc::clone(&cl)).await;
    let mut page = 1;
    let mut v = Vec::new();

    // get counts & init progress bar
    let total_replies = serde_json::from_str::<Value>(
        cl.get(format!(
            "https://api.aicu.cc/api/v3/search/getreply?uid={}&pn=1&ps=0&mode=0&keyword=",
            uid
        ))
        .send()
        .await
        .expect("Can't get total_replies")
        .text()
        .await
        .unwrap()
        .as_str(),
    )
    .unwrap()["data"]["cursor"]["all_count"]
        .as_u64()
        .unwrap();
    let pb = ProgressBar::new(total_replies);
    println!("正在从aicu.cc获取数据...");
    loop {
        let res = serde_json::from_str::<Value>(
            cl.get(format!(
                "https://api.aicu.cc/api/v3/search/getreply?uid={}&pn={}&ps=500&mode=0&keyword=",
                uid, page
            ))
            .send()
            .await
            .expect("Can't get first request")
            .text()
            .await
            .unwrap()
            .as_str(),
        )
        .unwrap();
        let replies = &res["data"]["replies"];
        for i in replies.as_array().unwrap() {
            let rpid = i["rpid"].as_str().unwrap().parse().unwrap();
            v.push(Comment {
                oid: i["dyn"]["oid"].as_str().unwrap().parse().unwrap(),
                r#type: i["dyn"]["type"].as_u64().unwrap() as u8,
                rpid,
                content: i["message"].as_str().unwrap().to_string(),
                remove_state: true,
                notify_id: None,
                tp: None,
            });
            pb.inc(1);
            // info!("Push Comment: {rpid}");
            // info!("Vec Counts:{}", v.len());
        }
        page += 1;
        if res["data"]["cursor"]["is_end"].as_bool().unwrap() == true {
            pb.finish_with_message("Fetched successful from aicu.cc");
            break;
        }
    }
    v
}

async fn fetch_comment_both(cl: Arc<Client>) -> Vec<Comment> {
    let mut seen_ids = HashSet::new();
    let mut v1 = fetch_comment_from_aicu(Arc::clone(&cl)).await;
    let v2 = fetch_comment(Arc::clone(&cl)).await;
    v1.retain(|e| seen_ids.insert(e.rpid));
    v2.into_iter().for_each(|item| {
        if seen_ids.insert(item.rpid) {
            v1.push(item);
        }
    });
    v1
}
async fn remove_notify(cl: Arc<Client>, id: u64, csrf: String, tp: String) {
    let res = cl
        .post(
            "
https://api.bilibili.com/x/msgfeed/del",
        )
        .form(&[
            ("tp", tp),
            ("id", id.to_string()),
            ("build", 0.to_string()),
            ("mobi_app", "web".to_string()),
            ("csrf_token", csrf.clone()),
            ("csrf", csrf),
        ])
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let json_res: serde_json::Value = serde_json::from_str(res.as_str()).unwrap();
    if json_res["code"].as_i64().unwrap() == 0 {
        info!("remove notify {id} success");
    } else {
        error!("Can't remove notify. Response json: {}", json_res);
    }
}

async fn fetch_remove_notifys(ck: String) {
    if let Message::ClientCreated { client: cl, csrf } = create_client(ck).await {
        let mut v: Vec<(u64, u8)> = Vec::new();
        let mut msgtype = MsgType::Like;
        let mut queryid = None;
        let mut last_time = None;
        loop {
            let json: serde_json::Value;
            let notifys: &serde_json::Value;
            if queryid.is_none() && last_time.is_none() {
                // 第一次请求
                let first = cl
                    .get(
                        Url::parse(
                            match msgtype {
                                MsgType::Like=>"https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web",
                                MsgType::Reply=>"https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web",
                                MsgType::At=>"https://api.bilibili.com/x/msgfeed/at?build=0&mobi_app=web"
                            }
                        )
                        .unwrap(),
                    )
                    .send()
                    .await
                    .expect("Can't get first request");
                json = serde_json::from_str(&first.text().await.unwrap()).unwrap();
                match msgtype {
                    MsgType::Like => {
                        notifys = &json["data"]["total"]["items"];
                        if notifys.as_array().unwrap().is_empty() {
                            msgtype = MsgType::Reply;
                            info!("没有收到赞的通知。");
                            continue;
                        }
                        last_time =
                            notifys.as_array().unwrap().last().unwrap()["like_time"].as_u64();
                        queryid = json["data"]["total"]["cursor"]["id"].as_u64();
                    }
                    MsgType::Reply => {
                        notifys = &json["data"]["items"];
                        if notifys.as_array().unwrap().is_empty() {
                            msgtype = MsgType::At;
                            info!("没有收到评论的通知。");
                            continue;
                        }
                        last_time =
                            notifys.as_array().unwrap().last().unwrap()["reply_time"].as_u64();
                        queryid = json["data"]["cursor"]["id"].as_u64();
                    }
                    MsgType::At => {
                        notifys = &json["data"]["items"];
                        if notifys.as_array().unwrap().is_empty() {
                            info!("没有被At的通知。");
                            break;
                        }
                        last_time = notifys.as_array().unwrap().last().unwrap()["at_time"].as_u64();
                        queryid = json["data"]["cursor"]["id"].as_u64();
                    }
                }
            } else {
                let mut url = Url::parse(match msgtype {
                    MsgType::Like => {
                        "https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web"
                    }
                    MsgType::Reply => {
                        "https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web"
                    }
                    MsgType::At => "https://api.bilibili.com/x/msgfeed/at?build=0&mobi_app=web",
                })
                .unwrap();
                match msgtype {
                    MsgType::Like => {
                        url.query_pairs_mut()
                            .append_pair("id", &queryid.unwrap().to_string())
                            .append_pair("like_time", &last_time.unwrap().to_string());
                        let other = cl.get(url).send().await.expect("Can't get request");
                        json = serde_json::from_str(&other.text().await.unwrap()).unwrap();
                        notifys = &json["data"]["total"]["items"];
                        last_time =
                            notifys.as_array().unwrap().last().unwrap()["like_time"].as_u64();
                        queryid = json["data"]["total"]["cursor"]["id"].as_u64();
                    }
                    MsgType::Reply => {
                        url.query_pairs_mut()
                            .append_pair("id", &queryid.unwrap().to_string())
                            .append_pair("reply_time", &last_time.unwrap().to_string());
                        let other = cl.get(url).send().await.expect("Can't get request");
                        json = serde_json::from_str(&other.text().await.unwrap()).unwrap();
                        notifys = &json["data"]["items"];
                        last_time =
                            notifys.as_array().unwrap().last().unwrap()["reply_time"].as_u64();
                        queryid = json["data"]["cursor"]["id"].as_u64();
                    }
                    MsgType::At => {
                        url.query_pairs_mut()
                            .append_pair("id", &queryid.unwrap().to_string())
                            .append_pair("at_time", &last_time.unwrap().to_string());
                        let other = cl.get(url).send().await.expect("Can't get request");
                        json = serde_json::from_str(&other.text().await.unwrap()).unwrap();
                        notifys = &json["data"]["items"];
                        last_time = notifys.as_array().unwrap().last().unwrap()["at_time"].as_u64();
                        queryid = json["data"]["cursor"]["id"].as_u64();
                    }
                }
            }
            dbg!(queryid, last_time);
            for i in notifys.as_array().unwrap() {
                let notify_id = i["id"].as_u64().unwrap();
                v.push((
                    notify_id,
                    match msgtype {
                        MsgType::Like => 0,
                        MsgType::Reply => 1,
                        MsgType::At => 2,
                    },
                ));
                info!("Fetched notify {notify_id}");
            }
            //push完检测是否为end
            match msgtype {
                MsgType::Like => {
                    if json["data"]["total"]["cursor"]["is_end"].as_bool().unwrap() {
                        msgtype = MsgType::Reply;
                        last_time = None;
                        queryid = None;
                        info!("收到赞的通知处理完毕。");
                    }
                }
                MsgType::Reply => {
                    if json["data"]["cursor"]["is_end"].as_bool().unwrap() {
                        msgtype = MsgType::At;
                        last_time = None;
                        queryid = None;
                        info!("收到评论的通知处理完毕。");
                        continue;
                    }
                }
                MsgType::At => {
                    if json["data"]["cursor"]["is_end"].as_bool().unwrap() {
                        info!("被At的通知处理完毕。");
                        break;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        info!("当前待处理通知数量: {}", v.len());
        let cl = Arc::new(cl);
        for i in v {
            remove_notify(Arc::clone(&cl), i.0, csrf.clone(), i.1.to_string()).await;
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        std::process::exit(0);
    }
}
