use iced::stream;
use iced::{
    futures::SinkExt,
    widget::{
        button, center, checkbox, column, image, row, scrollable, text, text_input, Column, Space,
    },
    Alignment, Element, Length, Subscription, Task,
};
use regex::Regex;
use reqwest::Client;
use reqwest::{header, Url};
use std::{sync::Arc, time::Duration};
use tokio::spawn;
use tokio::sync::mpsc::{self, Sender};
use tracing::{error, info};

static HONGWEN: &[u8] = include_bytes!("assets/mysterious.jpg");
static TAFFY: &[u8] = include_bytes!("assets/taffy.png");

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();
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
    ChangeCommentRemoveState(usize, bool),
    CommentsSelectAll,
    CommentsDeselectAll,
    DeleteComment,
    CommentDeleted { rpid: usize },
    CommentDeleteError(i64),
    ChannelConnected(Sender<(Arc<Client>, String, Comment)>),
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
                        println!("cookie submited: {}", s);
                        return Task::perform(create_client(s), move |m| m);
                    }
                    Message::CookieInputChanged(s) => {
                        self.cookie = s;
                    }
                    Message::ClientCreated { client, csrf } => {
                        self.client = Arc::new(client);
                        self.csrf = csrf;
                        self.state = State::InitCompleted;
                        return Task::perform(
                            fetch_comment(Arc::clone(&self.client)),
                            Message::CommentsFetched,
                        );
                    }
                    Message::ChannelConnected(s) => {
                        self.sender = Some(s);
                    }
                    _ => {}
                }
                Task::none()
            }

            State::InitCompleted => {
                match msg {
                    Message::CommentsFetched(comments) => {
                        self.comments = Some(comments);
                        self.state = State::CommentsFetched;
                    }
                    _ => {}
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
                            self.select_state = true;
                        }
                    }
                    Message::CommentsDeselectAll => {
                        for i in self.comments.as_mut().unwrap() {
                            i.remove_state = false;
                            self.select_state = false;
                        }
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
                row![
                    text_input("Input cookie here", &self.cookie)
                        .on_input(Message::CookieInputChanged)
                        .on_submit(Message::CookieSubmited(self.cookie.to_owned())),
                    button("enter").on_press(Message::CookieSubmited(self.cookie.to_owned())),
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
    oid: usize,
    r#type: usize,
    rpid: usize,
    content: String,
    remove_state: bool,
    notify_id: usize,
    /// 0为收到赞的评论 1为收到评论的评论
    tp: u8,
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

async fn fetch_comment(cl: Arc<Client>) -> Vec<Comment> {
    let mut v: Vec<Comment> = Vec::new();
    let oid_regex = Regex::new(r"bilibili://video/(\d+)").unwrap();
    // true => like; false => reply
    let mut like_or_reply = true;

    let mut queryid = None;
    let mut last_liketime = None;
    let mut last_replytime = None;
    loop {
        let json: serde_json::Value;
        let notifys: &serde_json::Value;
        if queryid.is_none() && last_liketime.is_none() {
            // 第一次请求
            let first = cl
                .get(
                    Url::parse(if like_or_reply {
                        "https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web"
                    } else {
                        "https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web"
                    })
                    .unwrap(),
                )
                .send()
                .await
                .expect("Can't get first request");
            json = serde_json::from_str(&first.text().await.unwrap()).unwrap();
            if like_or_reply {
                notifys = &json["data"]["total"]["items"];
                last_liketime = notifys.as_array().unwrap().last().unwrap()["like_time"].as_u64();
                queryid = json["data"]["total"]["cursor"]["id"].as_u64();
            } else {
                notifys = &json["data"]["items"];
                last_replytime = notifys.as_array().unwrap().last().unwrap()["reply_time"].as_u64();
                queryid = json["data"]["cursor"]["id"].as_u64();
            }
        } else {
            let mut url = Url::parse(if like_or_reply {
                "https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web"
            } else {
                "https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web"
            })
            .unwrap();
            if like_or_reply {
                url.query_pairs_mut()
                    .append_pair("id", &queryid.unwrap().to_string())
                    .append_pair("like_time", &last_liketime.unwrap().to_string());
                let other = cl.get(url).send().await.expect("Can't get first request");
                json = serde_json::from_str(&other.text().await.unwrap()).unwrap();
                notifys = &json["data"]["total"]["items"];
                last_liketime = notifys.as_array().unwrap().last().unwrap()["like_time"].as_u64();
                queryid = json["data"]["total"]["cursor"]["id"].as_u64();
            } else {
                url.query_pairs_mut()
                    .append_pair("id", &queryid.unwrap().to_string())
                    .append_pair("reply_time", &last_replytime.unwrap().to_string());
                let other = cl.get(url).send().await.expect("Can't get first request");
                json = serde_json::from_str(&other.text().await.unwrap()).unwrap();
                notifys = &json["data"]["items"];
                last_replytime = notifys.as_array().unwrap().last().unwrap()["reply_time"].as_u64();
                queryid = json["data"]["cursor"]["id"].as_u64();
            }
        }
        dbg!(queryid, last_liketime);
        let mut r#type: usize;
        'outer: for i in notifys.as_array().unwrap() {
            if i["item"]["type"] == "reply" {
                // dbg!(&i["item"]);
                // dbg!(
                //     if like_or_reply {
                //         last_liketime
                //     } else {
                //         last_replytime
                //     },
                //     queryid
                // );
                let rpid = if like_or_reply {
                    i["item"]["item_id"].as_u64().unwrap() as usize
                } else {
                    i["item"]["target_id"].as_u64().unwrap() as usize
                };
                if like_or_reply == false {
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
                        .parse::<usize>()
                        .unwrap();
                    let business_id = i["item"]["business_id"].as_u64();
                    r#type = if let Some(v) = business_id {
                        v as usize
                    } else {
                        17
                    };
                } else if uri.contains("https://h.bilibili.com/ywh/") {
                    // 带图动态内评论
                    oid = uri
                        .replace("https://h.bilibili.com/ywh/", "")
                        .parse::<usize>()
                        .unwrap();
                    r#type = 11;
                } else if uri.contains("https://www.bilibili.com/read/cv") {
                    // 专栏内评论
                    oid = uri
                        .replace("https://www.bilibili.com/read/cv", "")
                        .parse::<usize>()
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
                        .parse::<usize>()
                        .unwrap();
                    r#type = 1;
                } else if uri.contains("https://www.bilibili.com/bangumi/play/") {
                    // 电影（番剧？）内评论
                    oid = i["item"]["subject_id"].as_u64().unwrap() as usize;
                    r#type = 1;
                } else if uri == "" {
                    info!("No URI, Skiped");
                    continue;
                } else {
                    panic!("Undefined URI:{}\nCan't get oid", uri);
                }
                let content = if like_or_reply {
                    i["item"]["title"].as_str().unwrap().to_string()
                } else {
                    let v = i["item"]["target_reply_content"]
                        .as_str()
                        .unwrap()
                        .to_string();
                    if v == "" {
                        i["item"]["title"].as_str().unwrap().to_string()
                    } else {
                        v
                    }
                };
                let notify_id = i["id"].as_u64().unwrap() as usize;
                v.push(Comment {
                    oid,
                    r#type,
                    rpid,
                    content: content.clone(),
                    remove_state: true,
                    notify_id,
                    tp: if like_or_reply { 0 } else { 1 },
                });
                info!("Push Comment: {rpid}");
                info!("Vec Counts:{}", v.len());
            }
        }
        // push完检测是否为end
        if like_or_reply {
            if json["data"]["total"]["cursor"]["is_end"].as_bool().unwrap() == true {
                like_or_reply = false;
                last_liketime = None;
                queryid = None;
                info!("收到赞的评论处理完毕。");
            }
        } else {
            if json["data"]["cursor"]["is_end"].as_bool().unwrap() == true {
                info!("收到评论的评论处理完毕。");
                break;
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
        info!("remove reply {} success", i.rpid);
        remove_notify(cl, i.notify_id, csrf, i.tp.to_string()).await;
        Message::CommentDeleted { rpid: i.rpid }
    } else {
        error!("Can't remove comment. Response json: {}", json_res);
        Message::CommentDeleteError(json_res["code"].as_i64().unwrap())
    }
}

async fn remove_notify(cl: Arc<Client>, id: usize, csrf: String, tp: String) {
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
