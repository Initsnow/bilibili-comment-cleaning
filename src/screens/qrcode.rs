use crate::http::qr_code::QRdata;
use crate::types::{ChannelMsg, Result};
use iced::{
    widget::{button, center, column, qr_code, row, text, toggler, Space},
    Alignment, Element, Length, Task,
};
use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::error;

#[derive(Debug)]
pub struct QRCode {
    qr_data: Option<qr_code::Data>,
    qr_code: Option<Arc<Mutex<QRdata>>>,
    qr_code_state: Option<u64>,
    aicu_state: Arc<AtomicBool>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    AicuToggled(bool),
    EntertoCookieInput,
    QRcodeGot(Result<QRdata>),
    QRcodeRefresh,
    QRcodeState(Result<(u64, Option<String>)>),
}

pub enum Action {
    Run(Task<Message>),
    SendtoChannel(ChannelMsg),
    GetState(Arc<Mutex<QRdata>>),
    Boot { csrf: String, aicu_state: bool },
    EnterCookie,
    None,
}

impl QRCode {
    pub fn new(aicu_state: Arc<AtomicBool>) -> (Self, Task<Message>) {
        (
            QRCode {
                qr_data: None,
                qr_code: None,
                qr_code_state: None,
                aicu_state,
                error: None,
            },
            Task::perform(QRdata::request_qrcode(), Message::QRcodeGot),
        )
    }

    pub fn view(&self) -> Element<Message> {
        if let Some(v) = &self.qr_data {
            let mut cl = column![qr_code(v)];
            if let Some(c) = self.qr_code_state {
                let resmsg = match c {
                    0 => Cow::Borrowed("扫码登录成功"),
                    86038 => Cow::Borrowed("二维码已失效"),
                    86090 => Cow::Borrowed("已扫码，未确认"),
                    86101 => Cow::Borrowed("未扫码"),
                    _ => Cow::Owned(format!("未知代码：{}", c)),
                };
                cl = cl
                    .push(text(resmsg).shaping(text::Shaping::Advanced))
                    .push(
                        toggler(self.aicu_state.load(Ordering::SeqCst))
                            .on_toggle(Message::AicuToggled)
                            .label("Also fetch comments from aicu.cc"),
                    )
                    .push(row![
                        Space::with_width(Length::Fill),
                        button("Change to input cookie").on_press(Message::EntertoCookieInput)
                    ]);
            }
            if let Some(e) = &self.error {
                cl = column![text(e).shaping(text::Shaping::Advanced)];
            }
            center(cl.spacing(10).align_x(Alignment::Center))
                .padding(20)
                .into()
        } else {
            center(if let Some(e) = &self.error {
                text(e).shaping(text::Shaping::Advanced)
            } else {
                "QRCode is loading...".into()
            })
            .into()
        }
    }

    pub fn update(&mut self, msg: Message) -> Action {
        match msg {
            Message::QRcodeGot(Ok(d)) => {
                self.qr_data = Some(qr_code::Data::new(d.url.clone()).unwrap());
                self.qr_code = Some(Arc::new(Mutex::new(d)));
                return Action::SendtoChannel(ChannelMsg::StartRefreshQRcodeState);
            }
            Message::AicuToggled(b) => {
                self.aicu_state.store(b, Ordering::SeqCst);
            }
            Message::QRcodeRefresh => {
                return Action::GetState(self.qr_code.as_ref().unwrap().clone());
            }
            Message::QRcodeState(v) => match v {
                Ok(v) => {
                    self.qr_code_state = Some(v.0);
                    self.error = None;
                    if v.0 == 0 {
                        return Action::Boot {
                            csrf: v.1.unwrap(),
                            aicu_state: self.aicu_state.load(Ordering::SeqCst),
                        };
                    }
                }
                Err(e) => {
                    self.qr_code_state = None;
                    let e = format!("Failed to get QRCode state: {:#?}", e);
                    error!("{:?}",e);
                    self.error = Some(e);
                }
            },
            Message::EntertoCookieInput => {
                return Action::EnterCookie;
            }
            Message::QRcodeGot(Err(e)) => {
                let e = format!("Failed to fetch QRCode: {:#?}", e);
                error!("{:?}",e);
                self.error = Some(e);
            }
        }
        Action::None
    }
}
