use crate::http::utility::create_client;
use crate::types::Result;
use iced::{
    widget::{button, center, column, row, text_input, toggler, Space},
    Element, Length, Task,
};
use reqwest::Client;
use tracing::error;

#[derive(Debug, Default)]
pub struct Cookie {
    cookie: String,
    aicu_state: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    CookieSubmited(String),
    CookieInputChanged(String),
    ClientCreated(Result<(Client, String)>),
    AicuToggled(bool),
    EntertoQRcodeScan,
}

pub enum Action {
    Run(Task<Message>),
    Boot {
        client: Client,
        csrf: String,
        aicu_state: bool,
    },
    EnterQRCode,
    None,
}

impl Cookie {
    pub fn new() -> Self {
        Cookie::default()
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
                toggler(self.aicu_state)
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
                return Action::Run(Task::perform(create_client(s), Message::ClientCreated));
            }
            Message::CookieInputChanged(s) => {
                self.cookie = s;
            }
            Message::ClientCreated(Ok((client, csrf))) => {
                return Action::Boot {
                    client,
                    csrf,
                    aicu_state: self.aicu_state,
                };
            }
            Message::AicuToggled(b) => {
                self.aicu_state = b;
            }
            Message::EntertoQRcodeScan => {
                return Action::EnterQRCode;
            }
            Message::ClientCreated(Err(e)) => {
                error!("Client creation failed: {:?}", e);
            }
        }
        Action::None
    }
}
