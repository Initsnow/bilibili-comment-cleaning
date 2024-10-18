use std::sync::Arc;

use crate::types::{Comment, Message};
use iced::{
    widget::{button, center, checkbox, column, row, scrollable, text, text_input, Space},
    Alignment, Element, Length,
};
use tokio::sync::Mutex;

pub fn view<'a>(
    comments: &Option<Arc<Mutex<Vec<Comment>>>>,
    select_state: bool,
    sleep_seconds: &String,
) -> Element<'a, Message> {
    if let Some(comments) = comments {
        let head = text(format!(
            "There are currently {} comments",
            comments.blocking_lock().len()
        ));
        let a = comments.blocking_lock();
        let cl = column(a.iter().cloned().map(|i| {
            checkbox(i.content, i.remove_state)
                .text_shaping(iced::widget::text::Shaping::Advanced)
                .on_toggle(move |b| Message::ChangeCommentRemoveState(i.rpid, b))
                .into()
        }))
        .padding([0, 15]);
        let comments = center(scrollable(cl).height(Length::Fill));

        let controls = row![
            if select_state {
                button("select all").on_press(Message::CommentsSelectAll)
            } else {
                button("deselect all").on_press(Message::CommentsDeselectAll)
            },
            Space::with_width(Length::Fill),
            button("stop").on_press(Message::StopDeleteComment),
            Space::with_width(Length::Fill),
            row![
                text_input("sleep seconds", sleep_seconds)
                    .on_input(Message::SecondsInputChanged)
                    .on_submit(Message::DeleteComment),
                text("s"),
                button("remove").on_press(Message::DeleteComment)
            ]
            .spacing(5)
            .align_y(Alignment::Center)
        ]
        .height(Length::Shrink)
        .padding([0, 15]);
        center(
            column![head, comments, controls]
                .spacing(10)
                .align_x(Alignment::Center),
        )
        .padding([5, 20])
        .into()
    } else {
        center(text("任何邪恶，终将绳之以法😭").shaping(text::Shaping::Advanced)).into()
    }
}
