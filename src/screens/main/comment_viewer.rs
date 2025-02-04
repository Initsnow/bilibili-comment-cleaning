use crate::http::comment::Comment;
use crate::main;
use crate::main::Action;
use crate::types::ChannelMsg;
use iced::widget::{
    button, center, checkbox, column, row, scrollable, text, text_input, tooltip, Space,
};
use iced::{Alignment, Element, Length, Task};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::error;

pub struct CommentViewer {
    pub comments: Option<Arc<Mutex<HashMap<u64, Comment>>>>,
    /// 删除请求间隔
    pub sleep_seconds: String,
    /// 是否正在删除
    pub is_deleting: bool,
    /// 是否正在获取
    /// 默认为true，在Fetched后设置为false
    pub is_fetching: bool,
    /// select all | deselect all state
    pub select_state: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Msg {
    SecondsInputChanged(String),
    ChangeCommentRemoveState(u64, bool),
    CommentsSelectAll,
    CommentsDeselectAll,
    DeleteComment,
    StopDeleteComment,
    CommentDeleted { rpid: u64 },
    AllCommentDeleted,
    CommentsFetched(crate::types::Result<Arc<Mutex<HashMap<u64, Comment>>>>),
    RetryFetchComment,
}
impl CommentViewer {
    pub fn new() -> Self {
        CommentViewer {
            comments: None,
            sleep_seconds: "3".to_string(),
            is_deleting: false,
            is_fetching: true,
            select_state: false,
            error: None,
        }
    }

    pub fn view(&self) -> Element<Msg> {
        if let Some(comments) = &self.comments {
            let a = {
                let guard = comments.blocking_lock();
                guard.clone()
            };

            let head = text(format!(
                "{} selected out of {} total",
                a.values().filter(|e| e.is_selected).count(),
                a.len()
            ));
            let cl = column(a.into_iter().map(|(rpid, i)| {
                checkbox(i.content.to_string(), i.is_selected)
                    .text_shaping(text::Shaping::Advanced)
                    .on_toggle(move |b| Msg::ChangeCommentRemoveState(rpid, b))
                    .into()
            }))
            .padding([0, 15]);
            let comments = center(scrollable(cl).height(Length::Fill));

            let control = row![
                if self.select_state {
                    button("select all").on_press(Msg::CommentsSelectAll)
                } else {
                    button("deselect all").on_press(Msg::CommentsDeselectAll)
                },
                Space::with_width(Length::Fill),
                row![
                    tooltip(
                        text_input("0", &self.sleep_seconds)
                            .align_x(Alignment::Center)
                            .on_input(Msg::SecondsInputChanged)
                            .on_submit(Msg::DeleteComment)
                            .width(Length::Fixed(33.0)),
                        "Sleep seconds",
                        tooltip::Position::FollowCursor
                    ),
                    text("s"),
                    if self.is_deleting {
                        button("stop").on_press(Msg::StopDeleteComment)
                    } else {
                        button("remove").on_press(Msg::DeleteComment)
                    }
                ]
                .spacing(5)
                .align_y(Alignment::Center)
            ]
            .height(Length::Shrink);

            center(
                iced::widget::column![head, comments.width(Length::FillPortion(3)), control]
                    .align_x(Alignment::Center)
                    .spacing(10),
            )
            .padding([5, 20])
            .into()
        } else {
            center(scrollable(
                column![text(if self.is_fetching {
                    "Fetching..."
                } else {
                    if let Some(e) = &self.error {
                        e
                    } else {
                        "None 😭"
                    }
                })
                .shaping(text::Shaping::Advanced)]
                .push_maybe(
                    self.error
                        .as_ref()
                        .map(|_| button("Retry").on_press(Msg::RetryFetchComment)),
                )
                .align_x(Alignment::Center)
                .spacing(4),
            ))
            .into()
        }
    }

    pub fn update(&mut self, msg: Msg) -> Action {
        match msg {
            Msg::ChangeCommentRemoveState(rpid, b) => {
                let a = Arc::clone(self.comments.as_ref().unwrap());
                return Action::Run(Task::perform(
                    async move {
                        if let Some(v) = a.lock().await.get_mut(&rpid) {
                            v.is_selected = b
                        }
                    },
                    main::Message::RefreshUI,
                ));
            }
            Msg::CommentsSelectAll => {
                let a = Arc::clone(self.comments.as_ref().unwrap());
                self.select_state = false;
                return Action::Run(Task::perform(
                    async move {
                        a.lock()
                            .await
                            .values_mut()
                            .for_each(|e| e.is_selected = true);
                    },
                    main::Message::RefreshUI,
                ));
            }
            Msg::CommentsDeselectAll => {
                let a = Arc::clone(self.comments.as_ref().unwrap());
                self.select_state = true;
                return Action::Run(Task::perform(
                    async move {
                        a.lock()
                            .await
                            .values_mut()
                            .for_each(|e| e.is_selected = false);
                    },
                    main::Message::RefreshUI,
                ));
            }
            Msg::DeleteComment => {
                return Action::DeleteComment {
                    comments: self.comments.as_ref().unwrap().clone(),
                    sleep_seconds: self.sleep_seconds.parse::<f32>().unwrap_or(0.0),
                };
            }
            Msg::CommentDeleted { rpid } => {
                let a = Arc::clone(self.comments.as_ref().unwrap());
                return Action::Run(Task::perform(
                    async move {
                        a.lock().await.remove(&rpid);
                    },
                    main::Message::RefreshUI,
                ));
            }
            Msg::SecondsInputChanged(v) => {
                self.sleep_seconds = v;
            }
            Msg::StopDeleteComment => return Action::SendtoChannel(ChannelMsg::StopDeleteComment),
            Msg::AllCommentDeleted => {
                self.is_deleting = false;
            }
            Msg::CommentsFetched(Ok(c)) => {
                self.is_fetching = false;
                self.comments = Some(c);
            }
            Msg::CommentsFetched(Err(e)) => {
                self.is_fetching = false;
                let e = format!("Failed to fetch comments: {:?}", e);
                error!(e);
                self.error = Some(e);
            }
            Msg::RetryFetchComment => {
                self.error = None;
                self.is_fetching = true;
                return Action::RetryFetchComment;
            }
        }
        Action::None
    }
}
