use bilibili_comment_cleaning::http::{
    api_service::ApiService, comment, danmu, notify, qr_code::QRdata,
};
use bilibili_comment_cleaning::{
    main_subscription,
    screens::{cookie, main, qrcode, Screen},
    types::*,
};
use iced::{time, Element, Subscription, Task};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
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

    iced::application(App::new, App::update, App::view)
        .window(iced::window::Settings {
            icon: Some(icon),
            size: (900.0, 500.0).into(),
            ..Default::default()
        })
        .subscription(App::subscription)
        .title("BilibiliCommentCleaning")
        .run()
}

#[derive(Debug)]
struct App {
    api: Arc<ApiService>,
    screen: Screen,
    sender: Option<Sender<ChannelMsg>>,
    aicu_state: Arc<AtomicBool>,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let aicu_state = Arc::new(AtomicBool::new(true));
        let app = App {
            api: Arc::new(ApiService::default()),
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
                        cookie::Action::Boot { api, aicu_state } => {
                            self.api = Arc::new(api);
                            self.screen = Screen::Main(main::Main::new(aicu_state));

                            fetch_task(self.api.clone(), aicu_state)
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
                            self.api = Arc::new(ApiService::new_with_fields(
                                self.api.client().clone(),
                                csrf,
                            ));
                            self.screen = Screen::Main(main::Main::new(aicu_state));

                            fetch_task(self.api.clone(), aicu_state)
                        }
                        qrcode::Action::EnterCookie => {
                            self.screen = Screen::WaitingForInputCookie(cookie::Cookie::new(
                                self.aicu_state.clone(),
                            ));
                            Task::none()
                        }
                        qrcode::Action::GetState(v) => {
                            let api = self.api.clone();
                            Task::perform(
                                async move {
                                    let v = v.lock().await;
                                    v.get_state(api).await
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
                            self.send_to_channel(ChannelMsg::DeleteComment(
                                self.api.clone(),
                                comments,
                                sleep_seconds,
                            ));
                            Task::none()
                        }
                        main::Action::RetryFetchComment => comment::fetch_via_aicu_state(
                            self.api.clone(),
                            self.aicu_state.load(Ordering::SeqCst),
                        ),
                        main::Action::DeleteNotify {
                            notify,
                            sleep_seconds,
                        } => {
                            self.send_to_channel(ChannelMsg::DeleteNotify(
                                self.api.clone(),
                                notify,
                                sleep_seconds,
                            ));
                            Task::none()
                        }
                        main::Action::RetryFetchNotify => notify::fetch_task(self.api.clone()),
                        main::Action::DeleteDanmu {
                            danmu,
                            sleep_seconds,
                        } => {
                            self.send_to_channel(ChannelMsg::DeleteDanmu(
                                self.api.clone(),
                                danmu,
                                sleep_seconds,
                            ));
                            Task::none()
                        }
                        main::Action::RetryFetchDanmu => danmu::fetch_via_aicu_state(
                            self.api.clone(),
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
        if let Screen::WaitScanQRcode(_) = &self.screen {
            return Subscription::batch([
                time::every(Duration::from_secs(1))
                    .map(move |_| Message::QRCode(qrcode::Message::QRcodeRefresh)),
                main_subscription(),
            ]);
        }
        main_subscription()
    }

    fn send_to_channel(&self, m: ChannelMsg) {
        let sender = self.sender.as_ref().unwrap().clone();
        spawn(async move { sender.send(m).await });
    }
}

fn fetch_task(api: Arc<ApiService>, aicu_state: bool) -> Task<Message> {
    Task::batch([
        notify::fetch_task(api.clone()),
        comment::fetch_via_aicu_state(api.clone(), aicu_state)
            .chain(danmu::fetch_via_aicu_state(api.clone(), aicu_state)),
    ])
}
