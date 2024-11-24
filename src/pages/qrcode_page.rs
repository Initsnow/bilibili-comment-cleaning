use crate::{Main, State};
use bilibili_comment_cleaning::comment::{fetch_comment, fetch_comment_both};
use bilibili_comment_cleaning::types::{ChannelMsg, Message};
use iced::{
    widget::{button, center, column, qr_code, row, text, toggler, Space},
    Alignment, Element, Length, Task,
};
use std::sync::Arc;
use tokio::sync::Mutex;

pub fn view<'a>(
    qr_data: &'a Option<qr_code::Data>,
    qr_code_state: &'a Option<u64>,
    aicu_state: bool,
) -> Element<'a, Message> {
    if let Some(v) = qr_data {
        let mut cl = column![qr_code(v)];
        if let Some(c) = qr_code_state {
            let resmsg = match c {
                0 => "扫码登录成功".to_string(),
                86038 => "二维码已失效".to_string(),
                86090 => "已扫码，未确认".to_string(),
                86101 => "未扫码".to_string(),
                _ => format!("未知代码：{}", c),
            };
            cl = cl
                .push(text(resmsg).shaping(text::Shaping::Advanced))
                .push(
                    toggler(aicu_state)
                        .on_toggle(Message::AicuToggle)
                        .label("Also fetch comments from aicu.cc"),
                )
                .push(row![
                    Space::with_width(Length::Fill),
                    button("Change to input cookie").on_press(Message::EntertoCookieInput)
                ]);
        }
        center(cl.spacing(10).align_x(Alignment::Center))
            .padding(20)
            .into()
    } else {
        center("QRCode is loading...").into()
    }
}

pub fn update(main: &mut Main, msg: Message) -> Task<Message> {
    match msg {
        Message::QRcodeGot(d) => {
            if let State::WaitScanQRcode {
                ref mut qr_code,
                ref mut qr_data,
                ..
            } = main.state
            {
                *qr_data = Some(qr_code::Data::new(d.url.clone()).unwrap());
                *qr_code = Some(Arc::new(Mutex::new(d)));
                if let Some(sender) = main.sender.clone() {
                    return Task::perform(
                        async move { sender.send(ChannelMsg::StartRefreshQRcodeState).await },
                        |_| Message::QRcodeRefresh,
                    );
                }
            }
        }
        Message::AicuToggle(b) => {
            main.aicu_state = b;
        }
        Message::ChannelConnected(s) => {
            main.sender = Some(s);
        }
        Message::QRcodeRefresh => {
            if let State::WaitScanQRcode {
                qr_code: Some(ref v),
                ..
            } = main.state
            {
                let v = Arc::clone(v);
                let cl = Arc::clone(&main.client);
                return Task::perform(
                    async move {
                        let v = v.lock().await;
                        v.get_state(cl).await
                    },
                    Message::QRcodeState,
                );
            }
        }
        Message::QRcodeState(v) => {
            if let State::WaitScanQRcode {
                ref mut qr_code_state,
                ..
            } = main.state
            {
                *qr_code_state = Some(v.0);
            }
            if v.0 == 0 {
                main.csrf = Some(Arc::new(v.1.unwrap()));
                main.state = State::Fetching {
                    aicu_progress: None,
                    offcial_msg: None,
                };
                if let Some(sender) = main.sender.clone() {
                    return Task::batch([
                        if main.aicu_state {
                            Task::stream(fetch_comment_both(Arc::clone(&main.client)))
                        } else {
                            Task::stream(fetch_comment(Arc::clone(&main.client)))
                        },
                        Task::perform(
                            async move { sender.send(ChannelMsg::StopRefreshQRcodeState).await },
                            |_| Message::QRcodeRefresh,
                        ),
                    ]);
                }
            }
        }
        Message::EntertoCookieInput => {
            main.state = State::WaitingForInputCookie;
        }
        _ => {}
    }
    Task::none()
}
