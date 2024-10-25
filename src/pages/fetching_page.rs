use crate::types::Message;
use iced::{
    widget::{center, column, horizontal_rule, image, progress_bar, text},
    Alignment, Element, Length,
};

pub fn view<'a>(
    img: &'static [u8],
    aicu_progress: &'a Option<(f32, f32)>,
    offcial_msg: &'a Option<String>,
) -> Element<'a, Message> {
    center(
        column![
            image(image::Handle::from_bytes(img)).height(Length::FillPortion(2)),
            text("Fetching").height(Length::FillPortion(1))
        ]
        .push_maybe(
            aicu_progress.map(|e| column!["Fetching from aicu.cc:", progress_bar(0.0..=e.1, e.0),]),
        )
        .push_maybe(offcial_msg.clone().map(|e| {
            column![
                horizontal_rule(0.5),
                text("Fetching from official:"),
                text(e).shaping(text::Shaping::Advanced)
            ]
        }))
        .padding(20)
        .spacing(10)
        .align_x(Alignment::Center),
    )
    .into()
}
