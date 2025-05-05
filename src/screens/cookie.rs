use crate::http::api_service::ApiService;
use iced::{
    widget::{button, center, column, row, text_input, toggler, Space},
    Element, Length, Task,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug)]
pub struct Cookie {
    cookie: String,
    aicu_state: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub enum Message {
    CookieSubmited(String),
    CookieInputChanged(String),
    AicuToggled(bool),
    EntertoQRcodeScan,
}

pub enum Action {
    Run(Task<Message>),
    Boot { api: ApiService, aicu_state: bool },
    EnterQRCode,
    None,
}

impl Cookie {
    pub fn new(aicu_state: Arc<AtomicBool>) -> Self {
        Cookie {
            cookie: String::new(),
            aicu_state,
        }
    }
    pub fn view(&self) -> Element<Message> {
        let cookie = &self.cookie;
        center(
            column![
                row![
                    text_input("Input cookie here", cookie)
                        .on_input(Message::CookieInputChanged)
                        .on_submit(Message::CookieSubmited(cookie.clone())),
                    button("enter").on_press(Message::CookieSubmited(cookie.clone())),
                ]
                .spacing(5),
                toggler(self.aicu_state.load(Ordering::SeqCst))
                    .on_toggle(Message::AicuToggled)
                    .label("Also fetch comments from aicu.cc"),
                row![
                    Space::with_width(Length::Fill),
                    button("Change to scan QR code").on_press(Message::EntertoQRcodeScan)
                ]
            ]
            .spacing(5),
        )
        .padding(20)
        .into()
    }

    pub fn update(&mut self, msg: Message) -> Action {
        match msg {
            Message::CookieSubmited(s) => {
                let api = ApiService::new(s);
                return Action::Boot {
                    api,
                    aicu_state: self.aicu_state.load(Ordering::SeqCst),
                };
            }
            Message::CookieInputChanged(s) => {
                self.cookie = s;
            }
            Message::AicuToggled(b) => {
                self.aicu_state.store(b, Ordering::SeqCst);
            }
            Message::EntertoQRcodeScan => {
                return Action::EnterQRCode;
            }
        }
        Action::None
    }
}
