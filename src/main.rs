use crate::screens::{main, Screen};
use bilibili_comment_cleaning::http::notify;
use bilibili_comment_cleaning::http::qr_code::QRdata;
use bilibili_comment_cleaning::http::{comment, danmu};
use bilibili_comment_cleaning::types::*;
use bilibili_comment_cleaning::*;
use iced::{Element, Subscription, Task};
use reqwest::{header, Client};
use screens::{cookie, qrcode};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::spawn;
use tokio::sync::mpsc::Sender;
use tracing_subscriber::fmt::time::LocalTime;

static TAFFY: &[u8] = include_bytes!("assets/taffy.png");

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .compact()
        .with_target(false)
        .with_timer(LocalTime::rfc_3339())
        .init();

    let icon = iced::window::icon::from_file_data(TAFFY, None).unwrap();
    iced::application("BilibiliCommentCleaning", App::update, App::view)
        .window(iced::window::Settings {
            icon: Some(icon),
            size: (900.0, 500.0).into(),
            ..Default::default()
        })
        .subscription(App::subscription)
        .run_with(App::new)
}

#[derive(Debug)]
struct App {
    client: Arc<Client>,
    csrf: Option<Arc<String>>,
    screen: Screen,
    sender: Option<Sender<ChannelMsg>>,
    aicu_state: Arc<AtomicBool>,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let aicu_state = Arc::new(AtomicBool::new(true));
        let app=        App {
            client: Arc::new(Client::builder().default_headers({
                let mut headers = header::HeaderMap::new();
                headers.insert("User-Agent", header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36 Edg/127.0.2651.86"));
                headers
            }).cookie_store(true).build().unwrap()),
            csrf: None,
            screen: Screen::new(aicu_state.clone()),
            sender: None,
            aicu_state,
        };
        (
            app,
            Task::perform(QRdata::request_qrcode(), |a| {
                Message::QRCode(qrcode::Message::QRcodeGot(a))
            }),
        )
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Cookie(msg) => {
                if let Screen::WaitingForInputCookie(c) = &mut self.screen {
                    match c.update(msg) {
                        cookie::Action::Run(t) => t.map(Message::Cookie),
                        cookie::Action::EnterQRCode => {
                            let (s, t) = qrcode::QRCode::new(self.aicu_state.clone());
                            self.screen = Screen::WaitScanQRcode(s);
                            t.map(Message::QRCode)
                        }
                        cookie::Action::Boot {
                            client,
                            csrf,
                            aicu_state,
                        } => {
                            self.client = Arc::new(client);
                            self.csrf = Some(Arc::new(csrf));
                            self.screen = Screen::Main(main::Main::new(aicu_state));

                            let sender_clone = self.sender.as_ref().unwrap().clone();
                            let fetch_task = fetch_task(
                                self.client.clone(),
                                self.csrf.as_ref().unwrap().clone(),
                                aicu_state,
                            );

                            Task::batch([
                                fetch_task,
                                Task::perform(
                                    async move {
                                        sender_clone.send(ChannelMsg::StopRefreshQRcodeState).await
                                    },
                                    |_| Message::QRCode(qrcode::Message::QRcodeRefresh),
                                ),
                            ])
                        }
                        cookie::Action::None => Task::none(),
                    }
                } else {
                    Task::none()
                }
            }
            Message::QRCode(msg) => {
                if let Screen::WaitScanQRcode(q) = &mut self.screen {
                    match q.update(msg) {
                        qrcode::Action::Run(t) => t.map(Message::QRCode),
                        qrcode::Action::SendtoChannel(m) => {
                            self.send_to_channel(m);
                            Task::none()
                        }
                        qrcode::Action::Boot { csrf, aicu_state } => {
                            self.csrf = Some(Arc::new(csrf));
                            self.screen = Screen::Main(main::Main::new(aicu_state));

                            if let Some(sender) = self.sender.clone() {
                                let fetch_task = fetch_task(
                                    self.client.clone(),
                                    self.csrf.as_ref().unwrap().clone(),
                                    aicu_state,
                                );
                                Task::batch([
                                    fetch_task,
                                    Task::perform(
                                        async move {
                                            sender.send(ChannelMsg::StopRefreshQRcodeState).await
                                        },
                                        |_| Message::QRCode(qrcode::Message::QRcodeRefresh),
                                    ),
                                ])
                            } else {
                                Task::none()
                            }
                        }
                        qrcode::Action::EnterCookie => {
                            self.screen = Screen::WaitingForInputCookie(cookie::Cookie::new(
                                self.aicu_state.clone(),
                            ));
                            Task::none()
                        }
                        qrcode::Action::GetState(v) => {
                            let cl = Arc::clone(&self.client);
                            Task::perform(
                                async move {
                                    let v = v.lock().await;
                                    v.get_state(cl).await
                                },
                                |a| Message::QRCode(qrcode::Message::QRcodeState(a)),
                            )
                        }
                        qrcode::Action::None => Task::none(),
                    }
                } else {
                    Task::none()
                }
            }
            Message::Main(msg) => {
                if let Screen::Main(m) = &mut self.screen {
                    match m.update(msg) {
                        main::Action::Run(t) => t.map(Message::Main),
                        main::Action::SendtoChannel(m) => {
                            self.send_to_channel(m);
                            Task::none()
                        }
                        main::Action::DeleteComment {
                            comments,
                            sleep_seconds,
                        } => {
                            let cl = Arc::clone(&self.client);
                            let csrf = Arc::clone(self.csrf.as_ref().unwrap());
                            self.send_to_channel(ChannelMsg::DeleteComment(
                                cl,
                                csrf,
                                comments,
                                sleep_seconds,
                            ));
                            Task::none()
                        }
                        main::Action::RetryFetchComment => comment::fetch_via_aicu_state(
                            self.client.clone(),
                            self.aicu_state.load(Ordering::SeqCst),
                        ),
                        main::Action::DeleteNotify {
                            notify,
                            sleep_seconds,
                        } => {
                            let cl = Arc::clone(&self.client);
                            let csrf = Arc::clone(self.csrf.as_ref().unwrap());
                            self.send_to_channel(ChannelMsg::DeleteNotify(
                                cl,
                                csrf,
                                notify,
                                sleep_seconds,
                            ));
                            Task::none()
                        }
                        main::Action::RetryFetchNotify => notify::fetch_task(
                            self.client.clone(),
                            self.csrf.as_ref().unwrap().clone(),
                        ),
                        main::Action::DeleteDanmu {
                            danmu,
                            sleep_seconds,
                        } => {
                            let cl = Arc::clone(&self.client);
                            let csrf = Arc::clone(self.csrf.as_ref().unwrap());
                            self.send_to_channel(ChannelMsg::DeleteDanmu(
                                cl,
                                csrf,
                                danmu,
                                sleep_seconds,
                            ));
                            Task::none()
                        }
                        main::Action::RetryFetchDanmu => danmu::fetch_via_aicu_state(
                            self.client.clone(),
                            self.aicu_state.load(Ordering::SeqCst),
                        ),
                        main::Action::None => Task::none(),
                    }
                } else {
                    Task::none()
                }
            }
            Message::ChannelConnected(sender) => {
                self.sender = Some(sender);
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn view(&self) -> Element<Message> {
        match &self.screen {
            Screen::WaitingForInputCookie(c) => c.view().map(Message::Cookie),
            Screen::WaitScanQRcode(q) => q.view().map(Message::QRCode),
            Screen::Main(m) => m.view().map(Message::Main),
        }
    }
    fn subscription(&self) -> Subscription<Message> {
        main_subscription()
    }

    fn send_to_channel(&self, m: ChannelMsg) {
        let sender = self.sender.as_ref().unwrap().clone();
        spawn(async move { sender.send(m).await });
    }
}

fn fetch_task(cl: Arc<Client>, csrf: Arc<String>, aicu_state: bool) -> Task<Message> {
    notify::fetch_task(cl.clone(), csrf.clone()).chain(
        comment::fetch_via_aicu_state(cl.clone(), aicu_state)
            .chain(danmu::fetch_via_aicu_state(cl.clone(), aicu_state)),
    )
}
