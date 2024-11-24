use crate::Main;
use bilibili_comment_cleaning::types::{ChannelMsg, Comment, Message};
use iced::{
    widget::{button, center, checkbox, column, row, scrollable, text, text_input, tooltip, Space},
    Alignment, Element, Length, Task,
};
use std::sync::Arc;
use tokio::spawn;
use tokio::sync::Mutex;

pub fn view<'a>(
    comments: &Option<Arc<Mutex<Vec<Comment>>>>,
    select_state: bool,
    sleep_seconds: &str,
    is_deleting: bool,
) -> Element<'a, Message> {
    if let Some(comments) = comments {
        let a = comments.blocking_lock().clone();

        let head = text(format!("There are currently {} comments", a.len()));
        let cl = column(a.into_iter().map(|i| {
            checkbox(i.content, i.remove_state)
                .text_shaping(text::Shaping::Advanced)
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
            row![
                tooltip(
                    text_input("0", sleep_seconds)
                        .align_x(Alignment::Center)
                        .on_input(Message::SecondsInputChanged)
                        .on_submit(Message::DeleteComment)
                        .width(Length::Fixed(33.0)),
                    "Sleep seconds",
                    tooltip::Position::FollowCursor
                ),
                text("s"),
                if is_deleting {
                    button("stop").on_press(Message::StopDeleteComment)
                } else {
                    button("remove").on_press(Message::DeleteComment)
                }
            ]
            .spacing(5)
            .align_y(Alignment::Center)
        ]
        // let log=;
        .height(Length::Shrink);
        center(
            column![head, comments.width(Length::FillPortion(3)), controls]
                .align_x(Alignment::Center)
                .spacing(10),
        )
        .padding([5, 20])
        .into()
    } else {
        center(text("任何邪恶，终将绳之以法😭").shaping(text::Shaping::Advanced)).into()
    }
}

pub fn update(main: &mut Main, msg: Message) -> Task<Message> {
    match msg {
        Message::ChangeCommentRemoveState(rpid, b) => {
            let a = Arc::clone(main.comments.as_ref().unwrap());
            return Task::perform(
                async move {
                    for i in a.lock().await.iter_mut() {
                        if i.rpid == rpid {
                            i.remove_state = b;
                        }
                    }
                },
                Message::RefreshUI,
            );
        }
        Message::CommentsSelectAll => {
            let a = Arc::clone(main.comments.as_ref().unwrap());
            main.select_state = false;
            return Task::perform(
                async move {
                    for i in a.lock().await.iter_mut() {
                        i.remove_state = true;
                    }
                },
                Message::RefreshUI,
            );
        }
        Message::CommentsDeselectAll => {
            let a = Arc::clone(main.comments.as_ref().unwrap());
            main.select_state = true;
            return Task::perform(
                async move {
                    for i in a.lock().await.iter_mut() {
                        i.remove_state = false;
                    }
                },
                Message::RefreshUI,
            );
        }
        Message::DeleteComment => {
            let sender = main.sender.as_ref().unwrap().clone();
            let cl = Arc::clone(&main.client);
            let csrf = Arc::clone(main.csrf.as_ref().unwrap());
            let seconds = main.sleep_seconds.parse::<f32>().unwrap_or(0.0);
            let comments = Arc::clone(main.comments.as_ref().unwrap());
            main.is_deleting = true;
            spawn(async move {
                sender
                    .send(ChannelMsg::DeleteComment(cl, csrf, comments, seconds))
                    .await
                    .unwrap();
            });
        }
        Message::CommentDeleted { rpid } => {
            let a = Arc::clone(main.comments.as_ref().unwrap());
            return Task::perform(
                async move {
                    a.lock().await.retain(|e| e.rpid != rpid);
                },
                Message::RefreshUI,
            );
        }
        Message::SecondsInputChanged(v) => {
            main.sleep_seconds = v;
        }
        Message::StopDeleteComment => {
            let sender = main.sender.as_ref().unwrap().clone();
            spawn(async move {
                sender.send(ChannelMsg::StopDelete).await.unwrap();
            });
        }
        Message::AllCommentDeleted => {
            main.is_deleting = false;
        }
        _ => {}
    }
    Task::none()
}
