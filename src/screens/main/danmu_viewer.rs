use crate::http::danmu::Danmu;
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

pub struct DanmuViewer {
    pub danmu: Option<Arc<Mutex<HashMap<u64, Danmu>>>>,
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
    ChangeDanmuRemoveState(u64, bool),
    DanmusSelectAll,
    DanmusDeselectAll,
    DeleteDanmu,
    StopDeleteDanmu,
    DanmuDeleted { id: u64 },
    AllDanmuDeleted,
    DanmusFetched(Result<Arc<Mutex<HashMap<u64, Danmu>>>>),
    RetryFetchDanmu,
}
impl DanmuViewer {
    pub fn new() -> Self {
        DanmuViewer {
            danmu: None,
            sleep_seconds: "3".to_string(),
            is_deleting: false,
            is_fetching: true,
            select_state: false,
            error: None,
        }
    }

    pub fn view(&self) -> Element<Msg> {
        if let Some(comments) = &self.danmu {
            let a = {
                let guard = comments.blocking_lock();
                guard.clone()
            };

            let head = text(format!(
                "{} selected out of {} total",
                a.values().filter(|e| e.is_selected).count(),
                a.len()
            ));
            let cl = column(a.into_iter().map(|(id, i)| {
                checkbox(i.content.to_string(), i.is_selected)
                    .text_shaping(text::Shaping::Advanced)
                    .on_toggle(move |b| Msg::ChangeDanmuRemoveState(id, b))
                    .into()
            }))
            .padding([0, 15]);
            let comments = center(scrollable(cl).height(Length::Fill));

            let control = row![
                if self.select_state {
                    button("select all").on_press(Msg::DanmusSelectAll)
                } else {
                    button("deselect all").on_press(Msg::DanmusDeselectAll)
                },
                Space::with_width(Length::Fill),
                row![
                    tooltip(
                        text_input("0", &self.sleep_seconds)
                            .align_x(Alignment::Center)
                            .on_input(Msg::SecondsInputChanged)
                            .on_submit(Msg::DeleteDanmu)
                            .width(Length::Fixed(33.0)),
                        "Sleep seconds",
                        tooltip::Position::FollowCursor
                    ),
                    text("s"),
                    if self.is_deleting {
                        button("stop").on_press(Msg::StopDeleteDanmu)
                    } else {
                        button("remove").on_press(Msg::DeleteDanmu)
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
                        .map(|_| button("Retry").on_press(Msg::RetryFetchDanmu)),
                )
                .align_x(Alignment::Center)
                .spacing(4),
            ))
            .into()
        }
    }

    pub fn update(&mut self, msg: Msg) -> Action {
        match msg {
            Msg::ChangeDanmuRemoveState(id, b) => {
                let a = Arc::clone(self.danmu.as_ref().unwrap());
                return Action::Run(Task::perform(
                    async move {
                        if let Some(v) = a.lock().await.get_mut(&id) {
                            v.is_selected = b
                        }
                    },
                    main::Message::RefreshUI,
                ));
            }
            Msg::DanmusSelectAll => {
                let a = Arc::clone(self.danmu.as_ref().unwrap());
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
            Msg::DanmusDeselectAll => {
                let a = Arc::clone(self.danmu.as_ref().unwrap());
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
            Msg::DeleteDanmu => {
                return Action::DeleteDanmu {
                    danmu: self.danmu.as_ref().unwrap().clone(),
                    sleep_seconds: self.sleep_seconds.parse::<f32>().unwrap_or(0.0),
                };
            }
            Msg::DanmuDeleted { id } => {
                let a = Arc::clone(self.danmu.as_ref().unwrap());
                return Action::Run(Task::perform(
                    async move {
                        a.lock().await.remove(&id);
                    },
                    main::Message::RefreshUI,
                ));
            }
            Msg::SecondsInputChanged(v) => {
                self.sleep_seconds = v;
            }
            Msg::StopDeleteDanmu => return Action::SendtoChannel(ChannelMsg::StopDeleteComment),
            Msg::AllDanmuDeleted => {
                self.is_deleting = false;
            }
            Msg::DanmusFetched(Ok(c)) => {
                self.is_fetching = false;
                self.danmu = Some(c);
            }
            Msg::DanmusFetched(Err(e)) => {
                self.is_fetching = false;
                let e = format!("Failed to fetch danmu: {:?}", e);
                error!(e);
                self.error = Some(e);
            }
            Msg::RetryFetchDanmu => {
                self.error = None;
                self.is_fetching = true;
                return Action::RetryFetchDanmu;
            }
        }
        Action::None
    }
}
