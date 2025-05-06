use http::api_service::ApiService;
use iced::futures::channel::mpsc::Sender;
use iced::futures::SinkExt;
use iced::{stream, Subscription};
use indicatif::{ProgressBar, ProgressStyle};
use std::fmt::{Display, Formatter};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::spawn;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tracing::error;

pub mod http;
pub mod screens;
pub mod types;
pub use crate::screens::main::comment_viewer::CvMsg as cvmsg;
pub use crate::screens::main::danmu_viewer::DvMsg as dvmsg;
pub use crate::screens::main::notify_viewer::NvMsg as nvmsg;

use crate::screens::main;
use crate::types::{ChannelMsg, Message, RemoveAble};

const UA:&str="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36 Edg/127.0.2651.86";

enum Type {
    Comment,
    Danmu,
    Notify,
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Comment => write!(f, "评论"),
            Type::Danmu => write!(f, "弹幕"),
            Type::Notify => write!(f, "通知"),
        }
    }
}

async fn handle_delete<T>(
    delete_flag: Arc<AtomicBool>,
    mut output: Sender<Message>,
    items: Vec<(u64, T)>,
    tp: Type,
    seconds: f32,
    api: Arc<ApiService>,
) where
    T: RemoveAble,
{
    if items.is_empty() {
        return;
    }

    let len = items.len();
    let pb = ProgressBar::new(len as u64);
    pb.set_style(ProgressStyle::with_template("{wide_bar} {pos}/{len} {msg}").unwrap());

    let msg_done: Message = match tp {
        Type::Comment => cvmsg::AllCommentDeleted.into(),
        Type::Danmu => dvmsg::AllDanmuDeleted.into(),
        Type::Notify => nvmsg::AllNotifyDeleted.into(),
    };

    for (index, item) in items.into_iter().enumerate() {
        let (id, data) = item;
        if !delete_flag.load(Ordering::SeqCst) {
            delete_flag.store(true, Ordering::SeqCst);
            output.send(msg_done.clone()).await.unwrap();
            break;
        }
        match data.remove(id, api.clone()).await {
            Ok(id) => {
                output
                    .send(match tp {
                        Type::Comment => cvmsg::CommentDeleted { rpid: id }.into(),
                        Type::Danmu => dvmsg::DanmuDeleted { id }.into(),
                        Type::Notify => nvmsg::NotifyDeleted { id }.into(),
                    })
                    .await
                    .unwrap();
                pb.set_message(format!("已删除{}：{}", tp, id));
                pb.inc(1);
            }
            Err(err) => {
                error!("{}", err);
            }
        }

        if index + 1 == len {
            output.send(msg_done.clone()).await.unwrap();
            pb.finish_with_message("删除完成");
        }
        sleep(Duration::from_secs_f32(seconds)).await;
    }
}

pub fn main_subscription() -> Subscription<Message> {
    Subscription::run(|| {
        stream::channel(10, |mut output: Sender<Message>| async move {
            let (sender, mut receiver) = mpsc::channel(100);
            output
                .send(Message::ChannelConnected(sender))
                .await
                .unwrap();

            let flags = (
                Arc::new(AtomicBool::new(false)), // QR code
                Arc::new(AtomicBool::new(true)),  // Comment
                Arc::new(AtomicBool::new(true)),  // Notify
                Arc::new(AtomicBool::new(true)),  // Danmu
            );

            let mut tasks: (
                Option<JoinHandle<()>>,
                Option<JoinHandle<()>>,
                Option<JoinHandle<()>>,
            ) = (None, None, None);

            while let Some(msg) = receiver.recv().await {
                match msg {
                    ChannelMsg::DeleteComment(api, c, seconds) => {
                        let comments = c
                            .lock()
                            .await
                            .iter()
                            .filter(|e| e.1.is_selected)
                            .map(|(a, b)| (*a, b.clone()))
                            .collect::<Vec<_>>();

                        let flag = Arc::clone(&flags.1);
                        let output_clone = output.clone();
                        let task = spawn(handle_delete(
                            flag,
                            output_clone,
                            comments,
                            Type::Comment,
                            seconds,
                            api.clone(),
                        ));
                        tasks.0 = Some(task);
                    }
                    ChannelMsg::StopDeleteComment => {
                        flags.1.store(false, Ordering::SeqCst);
                    }
                    ChannelMsg::DeleteNotify(api, c, seconds) => {
                        let notify = c
                            .lock()
                            .await
                            .iter()
                            .filter(|e| e.1.is_selected)
                            .map(|(a, b)| (*a, b.clone()))
                            .collect::<Vec<_>>();

                        let flag = Arc::clone(&flags.2);
                        let output_clone = output.clone();
                        let task = spawn(handle_delete(
                            flag,
                            output_clone,
                            notify,
                            Type::Notify,
                            seconds,
                            api.clone(),
                        ));
                        tasks.1 = Some(task);
                    }
                    ChannelMsg::StopDeleteNotify => {
                        flags.2.store(false, Ordering::SeqCst);
                    }
                    ChannelMsg::DeleteDanmu(api, c, seconds) => {
                        let danmu = c
                            .lock()
                            .await
                            .iter()
                            .filter(|e| e.1.is_selected)
                            .map(|(a, b)| (*a, b.clone()))
                            .collect::<Vec<_>>();

                        let flag = Arc::clone(&flags.3);
                        let output_clone = output.clone();
                        let task = spawn(handle_delete(
                            flag,
                            output_clone,
                            danmu,
                            Type::Danmu,
                            seconds,
                            api.clone(),
                        ));
                        tasks.2 = Some(task);
                    }
                    ChannelMsg::StopDeleteDanmu => {
                        flags.3.store(false, Ordering::SeqCst);
                    }
                }
            }
        })
    })
}
