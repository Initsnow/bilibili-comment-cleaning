use crate::{Main, State};
use bilibili_comment_cleaning::comment::{fetch_comment, fetch_comment_both};
use bilibili_comment_cleaning::create_client;
use bilibili_comment_cleaning::types::{ChannelMsg, Message, QRcode};
use iced::{
    widget::{button, center, column, row, text_input, toggler, Space},
    Element, Length, Task,
};
use std::sync::Arc;

pub fn view<'a>(cookie: &String, aicu_state: bool) -> Element<'a, Message> {
    center(
        column![
            row![
                text_input("Input cookie here", cookie)
                    .on_input(Message::CookieInputChanged)
                    .on_submit(Message::CookieSubmited(cookie.to_owned())),
                button("enter").on_press(Message::CookieSubmited(cookie.to_owned())),
            ]
            .spacing(5),
            toggler(aicu_state)
                .on_toggle(Message::AicuToggle)
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

pub fn update(main: &mut Main, msg: Message) -> Task<Message> {
    match msg {
        Message::CookieSubmited(s) => {
            return Task::perform(create_client(s), move |e| Message::ClientCreated {
                client: e.0,
                csrf: e.1,
            });
        }
        Message::CookieInputChanged(s) => {
            main.cookie = s;
        }
        Message::ClientCreated { client, csrf } => {
            main.client = Arc::new(client);
            main.csrf = Some(Arc::new(csrf));
            main.state = State::Fetching {
                aicu_progress: None,
                offcial_msg: None,
            };
            let sender_clone = main.sender.as_ref().unwrap().clone();
            if main.aicu_state {
                return Task::batch([
                    Task::stream(fetch_comment_both(Arc::clone(&main.client))),
                    Task::perform(
                        async move { sender_clone.send(ChannelMsg::StopRefreshQRcodeState).await },
                        |_| Message::QRcodeRefresh,
                    ),
                ]);
            }
            return Task::batch([
                Task::stream(fetch_comment(Arc::clone(&main.client))),
                Task::perform(
                    async move { sender_clone.send(ChannelMsg::StopRefreshQRcodeState).await },
                    |_| Message::QRcodeRefresh,
                ),
            ]);
        }
        Message::AicuToggle(b) => {
            main.aicu_state = b;
        }
        Message::EntertoQRcodeScan => {
            main.state = State::WaitScanQRcode {
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
