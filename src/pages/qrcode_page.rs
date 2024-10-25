use crate::types::Message;
use iced::{
    widget::{button, center, column, qr_code, row, text, toggler, Space},
    Alignment, Element, Length,
};

pub fn view<'a>(
    qr_data: &'a Option<iced::widget::qr_code::Data>,
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
