pub mod comment_viewer;
pub mod danmu_viewer;
pub mod notify_viewer;

use crate::http::comment::Comment;
use crate::http::danmu::Danmu;
use crate::http::notify::Notify;
use crate::screens::main::danmu_viewer::DanmuViewer;
use crate::screens::main::notify_viewer::NotifyViewer;
use crate::types::{ChannelMsg, Result};
use comment_viewer::CommentViewer;
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
    cv: CommentViewer,
    nv: NotifyViewer,
    dv: DanmuViewer,
    /// Retrying the fetch requires
    pub aicu_state: bool,
}
impl fmt::Debug for Main {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Main")
            .field("panes", &"<HIDDEN>")
            .field("focus", &self.focus)
            // todo: other fields
            .finish()
    }
}

enum Pane {
    CommentViewer,
    DmViewer,
    NotifyViewer,
}

#[derive(Debug, Clone)]
pub enum Message {
    PaneDragged(pane_grid::DragEvent),
    PaneResized(pane_grid::ResizeEvent),
    PaneMaximize(pane_grid::Pane),
    PaneRestore,
    PaneClicked(pane_grid::Pane),

    CommentMsg(comment_viewer::Msg),
    NotifyMsg(notify_viewer::Msg),
    DanmuMsg(danmu_viewer::Msg),

    RefreshUI(()),
}

pub enum Action {
    Run(Task<Message>),

    DeleteComment {
        comments: Arc<Mutex<HashMap<u64, Comment>>>,
        sleep_seconds: f32,
    },
    RetryFetchComment,

    DeleteNotify {
        notify: Arc<Mutex<HashMap<u64, Notify>>>,
        sleep_seconds: f32,
    },
    RetryFetchNotify,

    DeleteDanmu {
        danmu: Arc<Mutex<HashMap<u64, Danmu>>>,
        sleep_seconds: f32,
    },
    RetryFetchDanmu,

    SendtoChannel(ChannelMsg),
    None,
}
impl Main {
    pub fn new(aicu_state: bool) -> Self {
        let pane_comment = pane_grid::Configuration::Pane(Pane::CommentViewer);
        let pane_dm = pane_grid::Configuration::Pane(Pane::DmViewer);
        let pane_notify = pane_grid::Configuration::Pane(Pane::NotifyViewer);
        let pane_left_side = pane_grid::Configuration::Split {
            axis: pane_grid::Axis::Vertical,
            ratio: 0.5,
            a: Box::new(pane_comment),
            b: Box::new(pane_dm),
        };
        let cfg = pane_grid::Configuration::Split {
            axis: pane_grid::Axis::Vertical,
            ratio: 2. / 3.,
            a: Box::new(pane_left_side),
            b: Box::new(pane_notify),
        };
        Main {
            panes: pane_grid::State::with_configuration(cfg),
            focus: None,
            cv: CommentViewer::new(),
            nv: NotifyViewer::new(),
            dv: DanmuViewer::new(),
            aicu_state,
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

            Message::CommentMsg(m) => return self.cv.update(m),
            Message::NotifyMsg(m) => return self.nv.update(m),
            Message::DanmuMsg(m) => return self.dv.update(m),

            Message::RefreshUI(_) => {}
        }
        Action::None
    }
    pub fn view(&self) -> Element<'_, Message> {
        let focus = self.focus;
        let pane_grid = pane_grid(&self.panes, |pane, state, is_maximized| {
            let is_focused = focus == Some(pane);
            let title = match state {
                Pane::CommentViewer => "comment",
                Pane::DmViewer => "danmu",
                Pane::NotifyViewer => "notify",
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
                Pane::CommentViewer => self.cv.view().map(Message::CommentMsg),
                Pane::DmViewer => self.dv.view().map(Message::DanmuMsg),
                Pane::NotifyViewer => self.nv.view().map(Message::NotifyMsg),
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
