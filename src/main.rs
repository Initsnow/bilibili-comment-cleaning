use bilibili_comment_cleaning::{
    get_json,
    notify::{
        fetch_remove_ated_notify, fetch_remove_liked_notify, fetch_remove_replyed_notify,
        fetch_remove_system_notify, remove_notify,
    },
};
use clap::{arg, command, Parser, Subcommand};
use iced::{
    futures::{SinkExt, Stream, StreamExt},
    stream,
    widget::qr_code,
    Element, Subscription, Task,
};
use indicatif::ProgressBar;
use regex::Regex;
use reqwest::{header, Client, Url};
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    spawn,
    sync::{
        mpsc::{self, Sender},
        Mutex,
    },
    task::JoinHandle,
    time::sleep,
};
use tracing::{error, info};
mod pages;
mod types;
use pages::{cookie_page, fetched_page, fetching_page, qrcode_page};
use types::{ChannelMsg, Comment, Message, QRcode};

static SOYO0: &[u8] = include_bytes!("assets/soyo0.png");
static TAFFY: &[u8] = include_bytes!("assets/taffy.png");

#[derive(Parser, Debug)]
#[command(author,version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}
#[derive(Subcommand, Debug)]
enum Commands {
    /// 删除通知
    RemoveNotify {
        cookie: String,
        /// 被点赞的评论通知
        #[arg(short, long)]
        liked_notify: bool,
        /// 被评论的评论通知
        #[arg(short, long)]
        replyed_notify: bool,
        /// 被At的评论通知
        #[arg(short, long)]
        ated_notify: bool,
        /// 系统通知
        #[arg(short, long)]
        system_notify: bool,
    },
}

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async move {
        if let Some(Commands::RemoveNotify {
            cookie,
            liked_notify,
            replyed_notify,
            ated_notify,
            system_notify,
        }) = cli.command
        {
            if !liked_notify & !replyed_notify & !ated_notify & !system_notify {
                info!("There's nothing to do...");
                std::process::exit(0);
            }
            let (cl, csrf) = create_client(cookie).await;
            let cl = Arc::new(cl);
            let csrf = Arc::new(csrf);

            if liked_notify {
                fetch_remove_liked_notify(cl.clone(), csrf.clone()).await;
            }
            if replyed_notify {
                fetch_remove_replyed_notify(cl.clone(), csrf.clone()).await;
            }
            if ated_notify {
                fetch_remove_ated_notify(cl.clone(), csrf.clone()).await;
            }
            if system_notify {
                fetch_remove_system_notify(cl.clone(), csrf.clone()).await;
            }
            std::process::exit(0);
        }
    });

    let icon = iced::window::icon::from_file_data(TAFFY, None).unwrap();
    iced::application("BilibiliCommentCleaning", Main::update, Main::view)
        .window(iced::window::Settings {
            icon: Some(icon),
            size: (820.0, 500.0).into(),
            ..Default::default()
        })
        .subscription(Main::subscription)
        .run_with(Main::new)
}

#[derive(Debug)]
struct Main {
    cookie: String,
    client: Arc<Client>,
    csrf: Option<Arc<String>>,
    state: State,
    comments: Option<Arc<Mutex<Vec<Comment>>>>,
    select_state: bool,
    aicu_state: bool,
    sender: Option<Sender<ChannelMsg>>,
    sleep_seconds: String,
    is_deleting: bool,
}
impl Default for Main {
    fn default() -> Self {
        Main {
            cookie: String::default(),
            client: Arc::new(Client::builder().default_headers({
                let mut headers = header::HeaderMap::new();
    headers.insert("User-Agent", header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36 Edg/127.0.2651.86"));
    headers
            }).cookie_store(true).build().unwrap()),
            csrf: None,
            state: State::default(),
            comments: None,
            select_state: false,
            aicu_state: true,
            sender: None,
            sleep_seconds: String::default(),
            is_deleting:false,
        }
    }
}

#[derive(Debug)]
enum State {
    WaitScanQRcode {
        qr_data: Option<qr_code::Data>,
        qr_code: Option<Arc<Mutex<QRcode>>>,
        qr_code_state: Option<u64>,
    },
    WaitingForInputCookie,
    LoginSuccess {
        aicu_progress: Option<(f32, f32)>,
        offcial_msg: Option<String>,
    },
    CommentsFetched,
}

impl Default for State {
    fn default() -> Self {
        State::WaitScanQRcode {
            qr_data: None,
            qr_code: None,
            qr_code_state: None,
        }
    }
}

impl Main {
    fn new() -> (Self, Task<Message>) {
        (
            Self::default(),
            Task::perform(QRcode::request_qrcode(), Message::QRcodeGot),
        )
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match self.state {
            State::WaitScanQRcode {
                ref mut qr_code,
                ref mut qr_data,
                ..
            } => {
                match msg {
                    Message::QRcodeGot(d) => {
                        *qr_data = Some(qr_code::Data::new(d.url.clone()).unwrap());
                        *qr_code = Some(Arc::new(Mutex::new(d)));
                        let sender_clone = self.sender.as_ref().unwrap().clone();
                        return Task::perform(
                            async move { sender_clone.send(ChannelMsg::StartRefreshQRcodeState).await },
                            |_| Message::QRcodeRefresh,
                        );
                    }
                    Message::AicuToggle(b) => {
                        self.aicu_state = b;
                    }
                    Message::ChannelConnected(s) => {
                        self.sender = Some(s);
                    }
                    Message::QRcodeRefresh => {
                        if let State::WaitScanQRcode { ref qr_code, .. } = self.state {
                            if let Some(v) = qr_code {
                                let v = Arc::clone(v);
                                let cl = Arc::clone(&self.client);
                                return Task::perform(
                                    async move {
                                        let v = v.lock().await;
                                        v.get_state(cl).await
                                    },
                                    Message::QRcodeState,
                                );
                            }
                        }
                    }
                    Message::QRcodeState(v) => {
                        if let State::WaitScanQRcode {
                            ref mut qr_code_state,
                            ..
                        } = self.state
                        {
                            *qr_code_state = Some(v.0)
                        }
                        if v.0 == 0 {
                            self.csrf = Some(Arc::new(v.1.unwrap()));
                            self.state = State::LoginSuccess {
                                aicu_progress: None,
                                offcial_msg: None,
                            };
                            let sender_clone = self.sender.as_ref().unwrap().clone();
                            return Task::batch([
                                if self.aicu_state {
                                    Task::stream(fetch_comment_both(Arc::clone(&self.client)))
                                } else {
                                    Task::stream(fetch_comment(Arc::clone(&self.client)))
                                },
                                Task::perform(
                                    async move {
                                        sender_clone.send(ChannelMsg::StopRefreshQRcodeState).await
                                    },
                                    |_| Message::QRcodeRefresh,
                                ),
                            ]);
                        }
                    }
                    Message::EntertoCookieInput => {
                        self.state = State::WaitingForInputCookie;
                    }
                    _ => {}
                }
                Task::none()
            }
            State::WaitingForInputCookie => {
                match msg {
                    Message::CookieSubmited(s) => {
                        return Task::perform(create_client(s), move |e| Message::ClientCreated {
                            client: e.0,
                            csrf: e.1,
                        });
                    }
                    Message::CookieInputChanged(s) => {
                        self.cookie = s;
                    }
                    Message::ClientCreated { client, csrf } => {
                        self.client = Arc::new(client);
                        self.csrf = Some(Arc::new(csrf));
                        self.state = State::LoginSuccess {
                            aicu_progress: None,
                            offcial_msg: None,
                        };
                        let sender_clone = self.sender.as_ref().unwrap().clone();
                        if self.aicu_state {
                            return Task::batch([
                                Task::stream(fetch_comment_both(Arc::clone(&self.client))),
                                Task::perform(
                                    async move {
                                        sender_clone.send(ChannelMsg::StopRefreshQRcodeState).await
                                    },
                                    |_| Message::QRcodeRefresh,
                                ),
                            ]);
                        }
                        return Task::batch([
                            Task::stream(fetch_comment(Arc::clone(&self.client))),
                            Task::perform(
                                async move {
                                    sender_clone.send(ChannelMsg::StopRefreshQRcodeState).await
                                },
                                |_| Message::QRcodeRefresh,
                            ),
                        ]);
                    }
                    Message::AicuToggle(b) => {
                        self.aicu_state = b;
                    }
                    Message::EntertoQRcodeScan => {
                        self.state = State::WaitScanQRcode {
                            qr_code: None,
                            qr_data: None,
                            qr_code_state: None,
                        };
                        return Task::perform(QRcode::request_qrcode(), Message::QRcodeGot);
                    }
                    _ => {}
                }
                Task::none()
            }

            State::LoginSuccess {
                ref mut aicu_progress,
                ref mut offcial_msg,
            } => {
                match msg {
                    Message::CommentsFetched(comments) => {
                        self.comments = Some(comments);
                        self.state = State::CommentsFetched;
                    }
                    Message::AicuFetchingState { now, max } => {
                        *aicu_progress = Some((now, max));
                    }
                    Message::OfficialFetchingState(s) => {
                        *offcial_msg = Some(s);
                    }
                    _ => {}
                }

                Task::none()
            }
            State::CommentsFetched => {
                match msg {
                    Message::ChangeCommentRemoveState(rpid, b) => {
                        let a = Arc::clone(self.comments.as_ref().unwrap());
                        return Task::perform(
                            async move {
                                for i in a.lock().await.iter_mut() {
                                    if i.rpid == rpid {
                                        i.remove_state = b;
                                    }
                                }
                            },
                            Message::RefreshUI,
                        );
                    }
                    Message::CommentsSelectAll => {
                        let a = Arc::clone(self.comments.as_ref().unwrap());
                        self.select_state = false;
                        return Task::perform(
                            async move {
                                for i in a.lock().await.iter_mut() {
                                    i.remove_state = true;
                                }
                            },
                            Message::RefreshUI,
                        );
                    }
                    Message::CommentsDeselectAll => {
                        let a = Arc::clone(self.comments.as_ref().unwrap());
                        self.select_state = true;
                        return Task::perform(
                            async move {
                                for i in a.lock().await.iter_mut() {
                                    i.remove_state = false;
                                }
                            },
                            Message::RefreshUI,
                        );
                    }
                    Message::DeleteComment => {
                        let sender = self.sender.as_ref().unwrap().clone();
                        let cl = Arc::clone(&self.client);
                        let csrf = Arc::clone(self.csrf.as_ref().unwrap());
                        let seconds = self.sleep_seconds.parse::<f32>().unwrap_or(0.0);
                        let comments = Arc::clone(self.comments.as_ref().unwrap());
                        self.is_deleting = true;
                        spawn(async move {
                            sender
                                .send(ChannelMsg::DeleteComment(cl, csrf, comments, seconds))
                                .await
                                .unwrap();
                        });
                    }
                    Message::CommentDeleted { rpid } => {
                        let a = Arc::clone(self.comments.as_ref().unwrap());
                        return Task::perform(
                            async move {
                                a.lock().await.retain(|e| e.rpid != rpid);
                            },
                            Message::RefreshUI,
                        );
                    }
                    Message::SecondsInputChanged(v) => {
                        self.sleep_seconds = v;
                    }
                    Message::StopDeleteComment => {
                        let sender = self.sender.as_ref().unwrap().clone();
                        spawn(async move {
                            sender.send(ChannelMsg::StopDelete).await.unwrap();
                        });
                    }
                    Message::AllCommentDeleted => {
                        self.is_deleting = false;
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
                let qrcode_refresh_flag = Arc::new(AtomicBool::new(false));
                let delete_flag = Arc::new(AtomicBool::new(true));
                let mut delete_task: Option<JoinHandle<()>> = None;

                loop {
                    // 处理消息接收
                    if let Some(msg) = receiver.recv().await {
                        match msg {
                            ChannelMsg::DeleteComment(cl, csrf, c, seconds) => {
                                delete_flag.store(true, Ordering::SeqCst);

                                let comments = c
                                    .lock()
                                    .await
                                    .iter()
                                    .filter(|e| e.remove_state)
                                    .cloned()
                                    .collect::<Vec<_>>();

                                if comments.is_empty() {
                                    continue;
                                }

                                // 如果已有删除任务正在执行，检查任务是否完成
                                if let Some(handle) = delete_task.take() {
                                    if !handle.is_finished() {
                                        handle.abort();
                                        info!("已有删除任务正在进行，已中止");
                                    }
                                }

                                // 启动新的删除任务
                                let delete_flag_clone = Arc::clone(&delete_flag);
                                let mut output_clone = output.clone();
                                delete_task = Some(spawn(async move {
                                    let len = comments.len();
                                    let pb = ProgressBar::new(len as u64);
                                    pb.set_style(
                                        indicatif::ProgressStyle::with_template(
                                            "{wide_bar} {pos}/{len} {msg}",
                                        )
                                        .unwrap(),
                                    );

                                    for (index, comment) in comments.iter().enumerate() {
                                        if !delete_flag_clone.load(Ordering::SeqCst) {
                                            output_clone
                                                .send(Message::AllCommentDeleted)
                                                .await
                                                .unwrap();
                                            info!("删除操作已中止");
                                            break;
                                        }

                                        let cl_clone = Arc::clone(&cl);
                                        let csrf_clone = Arc::clone(&csrf);
                                        match remove_comment(cl_clone, csrf_clone, comment).await {
                                            Ok(rpid) => {
                                                output_clone
                                                    .send(Message::CommentDeleted { rpid })
                                                    .await
                                                    .unwrap();
                                                pb.set_message(format!("已删除评论：{}", rpid));
                                                pb.inc(1);
                                            }
                                            Err(err) => {
                                                error!("{}", err);
                                            }
                                        }

                                        if index + 1 == len {
                                            output_clone
                                                .send(Message::AllCommentDeleted)
                                                .await
                                                .unwrap();
                                            pb.finish_with_message("删除完成");
                                        }

                                        sleep(Duration::from_secs_f32(seconds)).await;
                                    }
                                }));
                            }
                            ChannelMsg::StopDelete => {
                                delete_flag.store(false, Ordering::SeqCst);
                                info!("停止删除评论");
                            }
                            ChannelMsg::StartRefreshQRcodeState => {
                                qrcode_refresh_flag.store(true, Ordering::SeqCst);
                                let qrcode_refresh_flag_clone = Arc::clone(&qrcode_refresh_flag);
                                let mut output_clone = output.clone();
                                spawn(async move {
                                    while qrcode_refresh_flag_clone.load(Ordering::SeqCst) {
                                        output_clone.send(Message::QRcodeRefresh).await.unwrap();
                                        sleep(Duration::from_secs(1)).await;
                                    }
                                });
                            }
                            ChannelMsg::StopRefreshQRcodeState => {
                                qrcode_refresh_flag.store(false, Ordering::SeqCst);
                            }
                        }
                    } else {
                        panic!("Channel is closed");
                    }
                }
            })
        })
    }

    fn view(&self) -> Element<Message> {
        match &self.state {
            State::WaitScanQRcode {
                ref qr_data,
                ref qr_code_state,
                ..
            } => qrcode_page::view(qr_data, qr_code_state, self.aicu_state),
            State::WaitingForInputCookie => cookie_page::view(&self.cookie, self.aicu_state),
            State::LoginSuccess {
                offcial_msg,
                aicu_progress,
            } => fetching_page::view(SOYO0, aicu_progress, offcial_msg),
            State::CommentsFetched => fetched_page::view(
                &self.comments,
                self.select_state,
                &self.sleep_seconds,
                self.is_deleting,
            ),
        }
    }
}

async fn create_client(ck: String) -> (Client, String) {
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

    (cl, csrf.to_string())
}

enum MsgType {
    Like,
    Reply,
    At,
}

fn fetch_comment(cl: Arc<Client>) -> impl Stream<Item = Message> {
    stream::channel(10, |mut output| async move {
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
                json = get_json(
                Arc::clone(&cl),
                match msgtype {
                    MsgType::Like => {
                        "https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web"
                    }
                    MsgType::Reply => {
                        "https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web"
                    }
                    MsgType::At => "https://api.bilibili.com/x/msgfeed/at?build=0&mobi_app=web",
                },
            )
            .await;

                match msgtype {
                    MsgType::Like => {
                        notifys = &json["data"]["total"]["items"];
                        if notifys.as_array().unwrap().is_empty() {
                            msgtype = MsgType::Reply;
                            info!("没有收到赞的评论。");
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
                            info!("没有收到评论的评论。");
                            continue;
                        }
                        last_time =
                            notifys.as_array().unwrap().last().unwrap()["reply_time"].as_u64();
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
                        json = get_json(Arc::clone(&cl), url).await;
                        notifys = &json["data"]["total"]["items"];
                        last_time =
                            notifys.as_array().unwrap().last().unwrap()["like_time"].as_u64();
                        queryid = json["data"]["total"]["cursor"]["id"].as_u64();
                    }
                    MsgType::Reply => {
                        url.query_pairs_mut()
                            .append_pair("id", &queryid.unwrap().to_string())
                            .append_pair("reply_time", &last_time.unwrap().to_string());
                        json = get_json(Arc::clone(&cl), url).await;
                        notifys = &json["data"]["items"];
                        last_time =
                            notifys.as_array().unwrap().last().unwrap()["reply_time"].as_u64();
                        queryid = json["data"]["cursor"]["id"].as_u64();
                    }
                    MsgType::At => {
                        url.query_pairs_mut()
                            .append_pair("id", &queryid.unwrap().to_string())
                            .append_pair("at_time", &last_time.unwrap().to_string());
                        json = get_json(Arc::clone(&cl), url).await;
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
                                pb.set_message(format!("Duplicate Comment: {rpid}"));
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
                        info!("Undefined URI:{}\nSkip this comment: {}", uri, rpid);
                        continue;
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
                    let msg = format!("Push Comment: {}, Vec counts now: {}", rpid, v.len());
                    pb.set_message(msg.clone());
                    output
                        .send(Message::OfficialFetchingState(msg))
                        .await
                        .unwrap();
                    pb.tick();
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
        output
            .send(Message::CommentsFetched(Arc::new(Mutex::new(v))))
            .await
            .unwrap();
    })
}

async fn remove_comment(cl: Arc<Client>, csrf: Arc<String>, i: &Comment) -> Result<u64, String> {
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
                ("csrf", csrf.to_string()),
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
        // 如果is_some则删除通知
        if let Some(notify_id) = i.notify_id {
            remove_notify(cl, notify_id, csrf, i.tp.unwrap().to_string()).await;
        }
        Ok(i.rpid)
    } else {
        error!("Can't remove comment. Response json: {}", json_res);
        Err(format!("Can't remove comment. Response json: {}", json_res))
    }
}

async fn get_uid(cl: Arc<Client>) -> u64 {
    let json_res = get_json(cl, "https://api.bilibili.com/x/member/web/account").await;
    json_res["data"]["mid"]
        .as_u64()
        .expect("Can't get uid. Please check your cookie data")
}

fn fetch_comment_from_aicu(cl: Arc<Client>) -> impl Stream<Item = types::Message> {
    stream::channel(10, |mut output| async move {
        let uid = get_uid(Arc::clone(&cl)).await;
        let mut page = 1;
        let mut v = Vec::new();

        // get counts & init progress bar
        let total_replies = get_json(
            Arc::clone(&cl),
            format!(
                "https://api.aicu.cc/api/v3/search/getreply?uid={}&pn=1&ps=0&mode=0&keyword=",
                uid
            ),
        )
        .await["data"]["cursor"]["all_count"]
            .as_u64()
            .unwrap();
        let pb = ProgressBar::new(total_replies);
        let max = total_replies as f32;
        let mut count = 0.0;
        output
            .send(Message::AicuFetchingState { now: count, max })
            .await
            .unwrap();
        println!("正在从aicu.cc获取数据...");
        loop {
            let res = get_json(
                Arc::clone(&cl),
                format!(
                "https://api.aicu.cc/api/v3/search/getreply?uid={}&pn={}&ps=500&mode=0&keyword=",
                uid, page
            ),
            )
            .await;
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
                count += 1.0;
                output
                    .send(Message::AicuFetchingState { now: count, max })
                    .await
                    .unwrap();
            }
            page += 1;
            if res["data"]["cursor"]["is_end"].as_bool().unwrap() {
                pb.finish_with_message("Fetched successful from aicu.cc");
                break;
            }
        }
        output
            .send(Message::CommentsFetched(Arc::new(Mutex::new(v))))
            .await
            .unwrap();
    })
}

fn fetch_comment_both(cl: Arc<Client>) -> impl Stream<Item = Message> {
    stream::channel(10, |mut output| async move {
        let mut a = Box::pin(fetch_comment_from_aicu(Arc::clone(&cl)));
        let mut v1 = None;
        while let Some(v) = a.next().await {
            match v {
                Message::CommentsFetched(v) => v1 = Some(v),
                _ => output.send(v).await.unwrap(),
            }
        }
        let mut b = Box::pin(fetch_comment(Arc::clone(&cl)));
        let mut v2 = None;
        while let Some(v) = b.next().await {
            match v {
                Message::CommentsFetched(v) => v2 = Some(v),
                _ => output.send(v).await.unwrap(),
            }
        }

        let mut seen_ids = HashSet::new();
        {
            let mut v1_locked = v1.as_ref().unwrap().lock().await;
            v1_locked.retain(|e| seen_ids.insert(e.rpid));

            let v2_locked = v2.as_ref().unwrap().lock().await;
            v2_locked.iter().for_each(|item| {
                if seen_ids.insert(item.rpid) {
                    v1_locked.push(item.clone());
                }
            });
        }
        output
            .send(Message::CommentsFetched(v1.unwrap()))
            .await
            .unwrap();
    })
}
