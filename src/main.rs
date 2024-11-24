use bilibili_comment_cleaning::types::*;
use bilibili_comment_cleaning::{create_client, main_subscription, notify::*};
use clap::{arg, command, Parser, Subcommand};
use iced::{widget::qr_code, Element, Subscription, Task};
use reqwest::{header, Client};
use std::sync::Arc;
use tokio::sync::{mpsc::Sender, Mutex};
use tracing::info;

mod pages;
use pages::{cookie_page, fetched_page, fetching_page, qrcode_page};

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
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
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
        });
    }

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
    Fetching {
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
            State::WaitScanQRcode { .. } => qrcode_page::update(self, msg),
            State::WaitingForInputCookie => cookie_page::update(self, msg),
            State::Fetching { .. } => fetching_page::update(self, msg),
            State::CommentsFetched => fetched_page::update(self, msg),
        }
    }

    fn view(&self) -> Element<Message> {
        match &self.state {
            State::WaitScanQRcode {
                ref qr_data,
                ref qr_code_state,
                ..
            } => qrcode_page::view(qr_data, qr_code_state, self.aicu_state),
            State::WaitingForInputCookie => cookie_page::view(&self.cookie, self.aicu_state),
            State::Fetching {
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
    fn subscription(&self) -> Subscription<Message> {
        main_subscription()
    }
}
