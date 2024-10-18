use crate::types::Message;
use iced::{
    widget::{button, center, column, row, text_input, toggler, Space},
    Element, Length,
};

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
