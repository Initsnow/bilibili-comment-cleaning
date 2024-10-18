use crate::types::Message;
use iced::{
    widget::{center, column, image, text},
    Alignment, Element, Length,
};

pub fn view(img: &'static [u8]) -> Element<Message> {
    center(
        column![
            image(image::Handle::from_bytes(img)).height(Length::FillPortion(2)),
            text("Fetching").height(Length::FillPortion(1))
        ]
        .padding(20)
        .spacing(10)
        .align_x(Alignment::Center),
    )
    .into()
}
