use crate::http::danmu::Danmu;
use crate::main::Action;
use crate::types::{ChannelMsg, Result};
use crate::{main, nvmsg};
use iced::widget::{
    button, center, checkbox, column, row, scrollable, text, text_input, tooltip, Space,
};
use iced::{Alignment, Element, Length, Task};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::error;

#[derive(Debug)]
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
pub enum DvMsg {
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
impl Default for DanmuViewer {
    fn default() -> Self {
        Self::new()
    }
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

    pub fn view(&self) -> Element<DvMsg> {
        if let Some(comments) = &self.danmu {
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
                        Some(move |b| DvMsg::ChangeDanmuRemoveState(id, b))
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
                        .on_press_maybe((!self.is_deleting).then_some(DvMsg::DanmusSelectAll))
                } else {
                    button("deselect all")
                        .on_press_maybe((!self.is_deleting).then_some(DvMsg::DanmusDeselectAll))
                },
                Space::with_width(Length::Fill),
                row![
                    tooltip(
                        text_input("0", &self.sleep_seconds)
                            .align_x(Alignment::Center)
                            .on_input_maybe(
                                (!self.is_deleting).then_some(DvMsg::SecondsInputChanged)
                            )
                            .on_submit_maybe((!self.is_deleting).then_some(DvMsg::DeleteDanmu))
                            .width(Length::Fixed(33.0)),
                        "Sleep seconds",
                        tooltip::Position::FollowCursor
                    ),
                    text("s"),
                    if self.is_deleting {
                        button("stop").on_press(DvMsg::StopDeleteDanmu)
                    } else {
                        button("remove").on_press_maybe(if select_count != 0 {
                            Some(DvMsg::DeleteDanmu)
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
                } else if let Some(e) = &self.error {
                    e
                } else {
                    "None 😭"
                })
                .shaping(text::Shaping::Advanced)]
                .push_maybe(
                    self.error
                        .as_ref()
                        .map(|_| button("Retry").on_press(DvMsg::RetryFetchDanmu)),
                )
                .align_x(Alignment::Center)
                .spacing(4),
            ))
            .into()
        }
    }

    pub fn update(&mut self, msg: DvMsg) -> Action {
        match msg {
            DvMsg::ChangeDanmuRemoveState(id, b) => {
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
            DvMsg::DanmusSelectAll => {
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
            DvMsg::DanmusDeselectAll => {
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
            DvMsg::DeleteDanmu => {
                self.is_deleting = true;
                return Action::DeleteDanmu {
                    danmu: self.danmu.as_ref().unwrap().clone(),
                    sleep_seconds: self.sleep_seconds.parse::<f32>().unwrap_or(0.0),
                };
            }
            DvMsg::DanmuDeleted { id } => {
                let a = Arc::clone(self.danmu.as_ref().unwrap());
                return Action::Run(Task::perform(
                    async move { a.lock().await.remove(&id).unwrap() },
                    |e| {
                        if let Some(i) = e.notify_id {
                            main::Message::NotifyMsg(nvmsg::NotifyDeleted { id: i })
                        } else {
                            main::Message::RefreshUI(())
                        }
                    },
                ));
            }
            DvMsg::SecondsInputChanged(v) => {
                self.sleep_seconds = v;
            }
            DvMsg::StopDeleteDanmu => return Action::SendtoChannel(ChannelMsg::StopDeleteDanmu),
            DvMsg::AllDanmuDeleted => {
                self.is_deleting = false;
            }
            DvMsg::DanmusFetched(Ok(c)) => {
                self.is_fetching = false;
                self.danmu = Some(c);
            }
            DvMsg::DanmusFetched(Err(e)) => {
                self.is_fetching = false;
                let e = format!("Failed to fetch danmu: {:?}", e);
                error!("{:?}",e);
                self.error = Some(e);
            }
            DvMsg::RetryFetchDanmu => {
                self.error = None;
                self.is_fetching = true;
                return Action::RetryFetchDanmu;
            }
        }
        Action::None
    }
}
