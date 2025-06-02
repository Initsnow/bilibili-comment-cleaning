use crate::http::notify::Notify;
use crate::main;
use crate::main::Action;
use crate::types::{ChannelMsg, Result};
use iced::widget::{
    button, center, checkbox, column, row, scrollable, text, text_input, tooltip, Space,
};
use iced::{Alignment, Element, Length, Task};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::error;

#[derive(Debug)]
pub struct NotifyViewer {
    pub notify: Option<Arc<Mutex<HashMap<u64, Notify>>>>,
    /// 删除请求间隔
    pub sleep_seconds: String,
    /// 是否正在删除
    pub is_deleting: bool,
    /// 是否正在获取
    /// 默认为true，在Fetched后设置为false
    pub is_fetching: bool,
    /// select all | deselect all state
    pub select_state: bool,
}

#[derive(Clone, Debug)]
pub enum NvMsg {
    SecondsInputChanged(String),
    ChangeNotifyRemoveState(u64, bool),
    NotifysSelectAll,
    NotifysDeselectAll,
    DeleteNotify,
    StopDeleteNotify,
    NotifyDeleted { id: u64 },
    AllNotifyDeleted,
    NotifysFetched(Arc<Mutex<HashMap<u64, Notify>>>),
}
impl Default for NotifyViewer {
    fn default() -> Self {
        Self::new()
    }
}

impl NotifyViewer {
    pub fn new() -> Self {
        NotifyViewer {
            notify: None,
            sleep_seconds: "3".to_string(),
            is_deleting: false,
            is_fetching: true,
            select_state: false,
        }
    }

    pub fn view(&self) -> Element<NvMsg> {
        if let Some(comments) = &self.notify {
            let a = {
                let guard = comments.blocking_lock();
                guard.clone()
            };
            let select_count = a.values().filter(|e| e.is_selected).count();

            let head = text(format!(
                "{} selected out of {} total",
                select_count,
                a.len()
            ));
            let cl = column(a.into_iter().map(|(id, i)| {
                checkbox(i.content.to_string(), i.is_selected)
                    .text_shaping(text::Shaping::Advanced)
                    .on_toggle_maybe(if !self.is_deleting {
                        Some(move |b| NvMsg::ChangeNotifyRemoveState(id, b))
                    } else {
                        None
                    })
                    .into()
            }))
            .padding([0, 15]);
            let comments = center(scrollable(cl).height(Length::Fill).width(Length::Fill));

            let control = row![
                if self.select_state {
                    button("select all")
                        .on_press_maybe((!self.is_deleting).then_some(NvMsg::NotifysSelectAll))
                } else {
                    button("deselect all")
                        .on_press_maybe((!self.is_deleting).then_some(NvMsg::NotifysDeselectAll))
                },
                Space::with_width(Length::Fill),
                row![
                    tooltip(
                        text_input("0", &self.sleep_seconds)
                            .align_x(Alignment::Center)
                            .on_input_maybe(
                                (!self.is_deleting).then_some(NvMsg::SecondsInputChanged)
                            )
                            .on_submit_maybe((!self.is_deleting).then_some(NvMsg::DeleteNotify))
                            .width(Length::Fixed(33.0)),
                        "Sleep seconds",
                        tooltip::Position::FollowCursor
                    ),
                    text("s"),
                    if self.is_deleting {
                        button("stop").on_press(NvMsg::StopDeleteNotify)
                    } else {
                        button("remove").on_press_maybe(if select_count != 0 {
                            Some(NvMsg::DeleteNotify)
                        } else {
                            None
                        })
                    }
                ]
                .spacing(5)
                .align_y(Alignment::Center)
            ]
            .height(Length::Shrink);

            center(
                iced::widget::column![head, comments, control]
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
                    "None 😭"
                })
                .shaping(text::Shaping::Advanced)]
                .align_x(Alignment::Center)
                .spacing(4),
            ))
            .into()
        }
    }

    pub fn update(&mut self, msg: NvMsg) -> Action {
        match msg {
            NvMsg::ChangeNotifyRemoveState(id, b) => {
                let a = Arc::clone(self.notify.as_ref().unwrap());
                return Action::Run(Task::perform(
                    async move {
                        if let Some(v) = a.lock().await.get_mut(&id) {
                            v.is_selected = b
                        }
                    },
                    main::Message::RefreshUI,
                ));
            }
            NvMsg::NotifysSelectAll => {
                let a = Arc::clone(self.notify.as_ref().unwrap());
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
            NvMsg::NotifysDeselectAll => {
                let a = Arc::clone(self.notify.as_ref().unwrap());
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
            NvMsg::DeleteNotify => {
                self.is_deleting = true;
                return Action::DeleteNotify {
                    notify: self.notify.as_ref().unwrap().clone(),
                    sleep_seconds: self.sleep_seconds.parse::<f32>().unwrap_or(0.0),
                };
            }
            NvMsg::NotifyDeleted { id } => {
                let a = Arc::clone(self.notify.as_ref().unwrap());
                return Action::Run(Task::perform(
                    async move {
                        a.lock().await.remove(&id);
                    },
                    main::Message::RefreshUI,
                ));
            }
            NvMsg::SecondsInputChanged(v) => {
                self.sleep_seconds = v;
            }
            NvMsg::StopDeleteNotify => return Action::SendtoChannel(ChannelMsg::StopDeleteNotify),
            NvMsg::AllNotifyDeleted => {
                self.is_deleting = false;
            }
            NvMsg::NotifysFetched(c) => {
                self.is_fetching = false;
                self.notify = Some(c);
            }
        }
        Action::None
    }
}
