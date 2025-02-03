use crate::http::comment::Comment;
use crate::types::{ChannelMsg, Result};
use iced::{
    widget::{button, container, pane_grid, row, text},
    Element,
};
use iced::{
    widget::{center, checkbox, column, scrollable, text_input, tooltip, Space},
    Alignment, Length, Task,
};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::error;

pub struct Main {
    panes: pane_grid::State<Pane>,
    focus: Option<pane_grid::Pane>,
    comments: Option<Arc<Mutex<HashMap<u64, Comment>>>>,
    /// 删除请求间隔
    sleep_seconds: String,
    /// 是否正常删除
    is_deleting: bool,
    /// select all | deselect all state
    select_state: bool,
}
impl fmt::Debug for Main {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Main")
            .field("panes", &"<HIDDEN>")
            .field("focus", &self.focus)
            .field("comments", &self.comments)
            .field("sleep_seconds", &self.sleep_seconds)
            .field("is_deleting", &self.is_deleting)
            .field("select_state", &self.select_state)
            .finish()
    }
}

enum Pane {
    DataViewer,
    Control,
    Status,
}

#[derive(Debug, Clone)]
pub enum Message {
    PaneDragged(pane_grid::DragEvent),
    PaneResized(pane_grid::ResizeEvent),
    PaneMaximize(pane_grid::Pane),
    PaneRestore,
    PaneClicked(pane_grid::Pane),
    ChangeCommentRemoveState(u64, bool),
    CommentsSelectAll,
    CommentsDeselectAll,
    SecondsInputChanged(String),
    DeleteComment,
    StopDeleteComment,
    CommentDeleted { rpid: u64 },
    AllCommentDeleted,
    CommentsFetched(Result<Arc<Mutex<HashMap<u64, Comment>>>>),

    RefreshUI(()),
}

pub enum Action {
    Run(Task<Message>),
    DeleteComment {
        comments: Arc<Mutex<HashMap<u64, Comment>>>,
        sleep_seconds: f32,
    },
    SendtoChannel(ChannelMsg),

    None,
}
impl Main {
    pub fn new() -> Self {
        let pane_data = pane_grid::Configuration::Pane(Pane::DataViewer);
        let pane_control = pane_grid::Configuration::Pane(Pane::Control);
        let pane_log = pane_grid::Configuration::Pane(Pane::Status);
        let pane_right_side = pane_grid::Configuration::Split {
            axis: pane_grid::Axis::Horizontal,
            ratio: 0.3,
            a: Box::new(pane_control),
            b: Box::new(pane_log),
        };
        let cfg = pane_grid::Configuration::Split {
            axis: pane_grid::Axis::Vertical,
            ratio: 0.5,
            a: Box::new(pane_data),
            b: Box::new(pane_right_side),
        };
        Main {
            panes: pane_grid::State::with_configuration(cfg),
            focus: None,
            comments: None,
            sleep_seconds: String::new(),
            is_deleting: false,
            select_state: false,
        }
    }
    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
            }
            Message::PaneDragged(pane_grid::DragEvent::Dropped { pane, target }) => {
                self.panes.drop(pane, target);
            }
            Message::PaneDragged(_) => {}
            Message::PaneMaximize(pane) => self.panes.maximize(pane),
            Message::PaneRestore => {
                self.panes.restore();
            }
            Message::PaneClicked(pane) => {
                self.focus = Some(pane);
            }

            Message::ChangeCommentRemoveState(rpid, b) => {
                let a = Arc::clone(self.comments.as_ref().unwrap());
                return Action::Run(Task::perform(
                    async move {
                        if let Some(v) = a.lock().await.get_mut(&rpid) {
                            v.is_selected = b
                        }
                    },
                    Message::RefreshUI,
                ));
            }
            Message::CommentsSelectAll => {
                let a = Arc::clone(self.comments.as_ref().unwrap());
                self.select_state = false;
                return Action::Run(Task::perform(
                    async move {
                        a.lock()
                            .await
                            .values_mut()
                            .for_each(|e| e.is_selected = true);
                    },
                    Message::RefreshUI,
                ));
            }
            Message::CommentsDeselectAll => {
                let a = Arc::clone(self.comments.as_ref().unwrap());
                self.select_state = true;
                return Action::Run(Task::perform(
                    async move {
                        a.lock()
                            .await
                            .values_mut()
                            .for_each(|e| e.is_selected = false);
                    },
                    Message::RefreshUI,
                ));
            }
            Message::DeleteComment => {
                return Action::DeleteComment {
                    comments: self.comments.as_ref().unwrap().clone(),
                    sleep_seconds: self.sleep_seconds.parse::<f32>().unwrap_or(0.0),
                };
            }
            Message::CommentDeleted { rpid } => {
                let a = Arc::clone(self.comments.as_ref().unwrap());
                return Action::Run(Task::perform(
                    async move {
                        a.lock().await.remove(&rpid);
                    },
                    Message::RefreshUI,
                ));
            }
            Message::SecondsInputChanged(v) => {
                self.sleep_seconds = v;
            }
            Message::StopDeleteComment => return Action::SendtoChannel(ChannelMsg::StopDelete),
            Message::AllCommentDeleted => {
                self.is_deleting = false;
            }
            Message::CommentsFetched(Ok(c)) => {
                self.comments = Some(c);
            }
            Message::CommentsFetched(Err(e)) => {
                error!("Failed to fetch comments: {}", e);
                //todo Retry
            }
            Message::RefreshUI(_) => {}
        }
        Action::None
    }
    pub fn view(&self) -> Element<'_, Message> {
        let focus = self.focus;
        let pane_grid = pane_grid(&self.panes, |pane, state, is_maximized| {
            let is_focused = focus == Some(pane);
            let title = match state {
                Pane::DataViewer => "tmp: comment",
                Pane::Control => "Control",
                Pane::Status => "Status",
            };
            let titlebar = pane_grid::TitleBar::new(text(title))
                .controls(pane_grid::Controls::new(view_controls(pane, is_maximized)))
                .padding(3)
                .style(if is_focused {
                    style::title_bar_focused
                } else {
                    style::title_bar_active
                });

            pane_grid::Content::new(match state {
                Pane::DataViewer => self.comment_viewer(),
                Pane::Control => self.controls(),
                Pane::Status => "tmp: Status".into(),
            })
            .title_bar(titlebar)
            .style(if is_focused {
                style::pane_focused
            } else {
                style::pane_active
            })
        })
        .on_drag(Message::PaneDragged)
        .on_resize(10, Message::PaneResized)
        .on_click(Message::PaneClicked)
        .spacing(5);
        container(pane_grid).padding(5).into()
    }

    fn comment_viewer(&self) -> Element<Message> {
        if let Some(comments) = &self.comments {
            let a = {
                let guard = comments.blocking_lock();
                guard.clone()
            };

            let head = text(format!("There are currently {} comments", a.len()));
            let cl = column(a.into_iter().map(|(rpid, i)| {
                checkbox(i.content.to_string(), i.is_selected)
                    .text_shaping(text::Shaping::Advanced)
                    .on_toggle(move |b| Message::ChangeCommentRemoveState(rpid, b))
                    .into()
            }))
            .padding([0, 15]);
            let comments = center(scrollable(cl).height(Length::Fill));

            center(
                column![head, comments.width(Length::FillPortion(3))]
                    .align_x(Alignment::Center)
                    .spacing(10),
            )
            .padding([5, 20])
            .into()
        } else {
            center(text("None 😭").shaping(text::Shaping::Advanced)).into()
        }
    }
    fn controls(&self) -> Element<Message> {
        row![
            if self.select_state {
                button("select all").on_press(Message::CommentsSelectAll)
            } else {
                button("deselect all").on_press(Message::CommentsDeselectAll)
            },
            Space::with_width(Length::Fill),
            row![
                tooltip(
                    text_input("0", &self.sleep_seconds)
                        .align_x(Alignment::Center)
                        .on_input(Message::SecondsInputChanged)
                        .on_submit(Message::DeleteComment)
                        .width(Length::Fixed(33.0)),
                    "Sleep seconds",
                    tooltip::Position::FollowCursor
                ),
                text("s"),
                if self.is_deleting {
                    button("stop").on_press(Message::StopDeleteComment)
                } else {
                    button("remove").on_press(Message::DeleteComment)
                }
            ]
            .spacing(5)
            .align_y(Alignment::Center)
        ]
        .height(Length::Shrink)
        .into()
    }
}

fn view_controls<'a>(pane: pane_grid::Pane, is_maximized: bool) -> Element<'a, Message> {
    let (content, message) = if is_maximized {
        ("Restore", Message::PaneRestore)
    } else {
        ("Maximize", Message::PaneMaximize(pane))
    };

    let row = row![button(text(content).size(14))
        .style(button::secondary)
        .padding(3)
        .on_press(message),];

    row.into()
}

mod style {
    use iced::widget::container;
    use iced::{Border, Theme};

    pub fn title_bar_active(theme: &Theme) -> container::Style {
        let palette = theme.extended_palette();

        container::Style {
            text_color: Some(palette.background.strong.text),
            background: Some(palette.background.strong.color.into()),
            ..Default::default()
        }
    }

    pub fn title_bar_focused(theme: &Theme) -> container::Style {
        let palette = theme.extended_palette();

        container::Style {
            text_color: Some(palette.primary.strong.text),
            background: Some(palette.primary.strong.color.into()),
            ..Default::default()
        }
    }

    pub fn pane_active(theme: &Theme) -> container::Style {
        let palette = theme.extended_palette();

        container::Style {
            background: Some(palette.background.weak.color.into()),
            border: Border {
                width: 2.0,
                color: palette.background.strong.color,
                radius: 3.0.into(),
                ..Border::default()
            },
            ..Default::default()
        }
    }

    pub fn pane_focused(theme: &Theme) -> container::Style {
        let palette = theme.extended_palette();

        container::Style {
            background: Some(palette.background.weak.color.into()),
            border: Border {
                width: 2.0,
                color: palette.primary.strong.color,
                radius: 3.0.into(),
                ..Border::default()
            },
            ..Default::default()
        }
    }
}
