use iced::futures::channel::mpsc::Sender;
use iced::futures::SinkExt;
use iced::{stream, Subscription};
use indicatif::ProgressBar;
use reqwest::{Client, IntoUrl};
use std::fmt::{Display, Formatter};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::spawn;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tracing::{error, info};

pub mod http;
pub mod screens;
pub mod types;
pub use crate::screens::main::comment_viewer::CvMsg as cvmsg;
pub use crate::screens::main::danmu_viewer::DvMsg as dvmsg;
pub use crate::screens::main::notify_viewer::NvMsg as nvmsg;

use crate::screens::{main, qrcode};
use crate::types::{ChannelMsg, Message, RemoveAble};

// pub fn main_subscription() -> Subscription<Message> {
//     Subscription::run(|| {
//         stream::channel(100, |mut output| async move {
//             let (sender, mut receiver) = mpsc::channel(100);
//             output
//                 .send(Message::ChannelConnected(sender))
//                 .await
//                 .unwrap();
//             let qrcode_refresh_flag = Arc::new(AtomicBool::new(false));
//
//             let comment_delete_flag = Arc::new(AtomicBool::new(true));
//             let mut comment_delete_task: Option<JoinHandle<()>> = None;
//
//             let notify_delete_flag = Arc::new(AtomicBool::new(true));
//             let mut notify_delete_task: Option<JoinHandle<()>> = None;
//
//             let danmu_delete_flag = Arc::new(AtomicBool::new(true));
//             let mut danmu_delete_task: Option<JoinHandle<()>> = None;
//
//             loop {
//                 // 处理消息接收
//                 if let Some(msg) = receiver.recv().await {
//                     match msg {
//                         ChannelMsg::DeleteComment(cl, csrf, c, seconds) => {
//                             comment_delete_flag.store(true, Ordering::SeqCst);
//
//                             let comments = c
//                                 .lock()
//                                 .await
//                                 .iter()
//                                 .filter(|e| e.1.is_selected)
//                                 .map(|(a, b)| (*a, b.clone()))
//                                 .collect::<Vec<(_, _)>>();
//
//                             if comments.is_empty() {
//                                 continue;
//                             }
//
//                             // 如果已有删除任务正在执行，检查任务是否完成
//                             if let Some(handle) = comment_delete_task.take() {
//                                 if !handle.is_finished() {
//                                     handle.abort();
//                                     info!("已有删除任务正在进行，已中止");
//                                 }
//                             }
//
//                             // 启动新的删除任务
//                             let delete_flag_clone = Arc::clone(&comment_delete_flag);
//                             let mut output_clone = output.clone();
//                             comment_delete_task = Some(spawn(async move {
//                                 let len = comments.len();
//                                 let pb = ProgressBar::new(len as u64);
//                                 pb.set_style(
//                                     indicatif::ProgressStyle::with_template(
//                                         "{wide_bar} {pos}/{len} {msg}",
//                                     )
//                                     .unwrap(),
//                                 );
//
//                                 for (index, comment) in comments.into_iter().enumerate() {
//                                     let (rpid, comment) = comment;
//                                     if !delete_flag_clone.load(Ordering::SeqCst) {
//                                         output_clone
//                                             .send(Message::Main(main::Message::CommentMsg(
//                                                 cvmsg::AllCommentDeleted,
//                                             )))
//                                             .await
//                                             .unwrap();
//                                         info!("删除操作已中止");
//                                         break;
//                                     }
//
//                                     let cl_clone = Arc::clone(&cl);
//                                     let csrf_clone = Arc::clone(&csrf);
//                                     match comment.remove(rpid, cl_clone, csrf_clone).await {
//                                         Ok(rpid) => {
//                                             output_clone
//                                                 .send(Message::Main(main::Message::CommentMsg(
//                                                     cvmsg::CommentDeleted { rpid },
//                                                 )))
//                                                 .await
//                                                 .unwrap();
//                                             pb.set_message(format!("已删除评论：{}", rpid));
//                                             pb.inc(1);
//                                         }
//                                         Err(err) => {
//                                             error!("{}", err);
//                                         }
//                                     }
//
//                                     if index + 1 == len {
//                                         output_clone
//                                             .send(Message::Main(main::Message::CommentMsg(
//                                                 cvmsg::AllCommentDeleted,
//                                             )))
//                                             .await
//                                             .unwrap();
//                                         pb.finish_with_message("删除完成");
//                                     }
//
//                                     sleep(Duration::from_secs_f32(seconds)).await;
//                                 }
//                             }));
//                         }
//                         ChannelMsg::StopDeleteComment => {
//                             comment_delete_flag.store(false, Ordering::SeqCst);
//                             info!("停止删除评论");
//                         }
//                         ChannelMsg::DeleteNotify(cl, csrf, c, seconds) => {
//                             notify_delete_flag.store(true, Ordering::SeqCst);
//
//                             let notify = c
//                                 .lock()
//                                 .await
//                                 .iter()
//                                 .filter(|e| e.1.is_selected)
//                                 .map(|(a, b)| (*a, b.clone()))
//                                 .collect::<Vec<(_, _)>>();
//
//                             if notify.is_empty() {
//                                 continue;
//                             }
//
//                             // 如果已有删除任务正在执行，检查任务是否完成
//                             if let Some(handle) = notify_delete_task.take() {
//                                 if !handle.is_finished() {
//                                     handle.abort();
//                                     info!("已有删除任务正在进行，已中止");
//                                 }
//                             }
//
//                             // 启动新的删除任务
//                             let delete_flag_clone = Arc::clone(&notify_delete_flag);
//                             let mut output_clone = output.clone();
//                             notify_delete_task = Some(spawn(async move {
//                                 let len = notify.len();
//                                 let pb = ProgressBar::new(len as u64);
//                                 pb.set_style(
//                                     indicatif::ProgressStyle::with_template(
//                                         "{wide_bar} {pos}/{len} {msg}",
//                                     )
//                                     .unwrap(),
//                                 );
//
//                                 for (index, notify) in notify.into_iter().enumerate() {
//                                     let (id, notify) = notify;
//                                     if !delete_flag_clone.load(Ordering::SeqCst) {
//                                         output_clone
//                                             .send(Message::Main(main::Message::NotifyMsg(
//                                                 nvmsg::AllNotifyDeleted,
//                                             )))
//                                             .await
//                                             .unwrap();
//                                         info!("删除操作已中止");
//                                         break;
//                                     }
//
//                                     let cl_clone = Arc::clone(&cl);
//                                     let csrf_clone = Arc::clone(&csrf);
//                                     match notify.remove(id, cl_clone, csrf_clone).await {
//                                         Ok(id) => {
//                                             output_clone
//                                                 .send(Message::Main(main::Message::NotifyMsg(
//                                                     nvmsg::NotifyDeleted { id },
//                                                 )))
//                                                 .await
//                                                 .unwrap();
//                                             pb.set_message(format!("已删除通知：{}", id));
//                                             pb.inc(1);
//                                         }
//                                         Err(err) => {
//                                             error!("{}", err);
//                                         }
//                                     }
//
//                                     if index + 1 == len {
//                                         output_clone
//                                             .send(Message::Main(main::Message::NotifyMsg(
//                                                 nvmsg::AllNotifyDeleted,
//                                             )))
//                                             .await
//                                             .unwrap();
//                                         pb.finish_with_message("删除完成");
//                                     }
//
//                                     sleep(Duration::from_secs_f32(seconds)).await;
//                                 }
//                             }));
//                         }
//                         ChannelMsg::StopDeleteNotify => {
//                             notify_delete_flag.store(false, Ordering::SeqCst);
//                             info!("停止删除通知");
//                         }
//
//                         ChannelMsg::DeleteDanmu(cl, csrf, c, seconds) => {
//                             danmu_delete_flag.store(true, Ordering::SeqCst);
//
//                             let danmu = c
//                                 .lock()
//                                 .await
//                                 .iter()
//                                 .filter(|e| e.1.is_selected)
//                                 .map(|(a, b)| (*a, b.clone()))
//                                 .collect::<Vec<(_, _)>>();
//
//                             if danmu.is_empty() {
//                                 continue;
//                             }
//
//                             // 如果已有删除任务正在执行，检查任务是否完成
//                             if let Some(handle) = danmu_delete_task.take() {
//                                 if !handle.is_finished() {
//                                     handle.abort();
//                                     info!("已有删除任务正在进行，已中止");
//                                 }
//                             }
//
//                             // 启动新的删除任务
//                             let delete_flag_clone = Arc::clone(&danmu_delete_flag);
//                             let mut output_clone = output.clone();
//                             danmu_delete_task = Some(spawn(async move {
//                                 let len = danmu.len();
//                                 let pb = ProgressBar::new(len as u64);
//                                 pb.set_style(
//                                     indicatif::ProgressStyle::with_template(
//                                         "{wide_bar} {pos}/{len} {msg}",
//                                     )
//                                     .unwrap(),
//                                 );
//
//                                 for (index, danmu) in danmu.into_iter().enumerate() {
//                                     let (id, danmu) = danmu;
//                                     if !delete_flag_clone.load(Ordering::SeqCst) {
//                                         output_clone
//                                             .send(Message::Main(main::Message::DanmuMsg(
//                                                 dvmsg::AllDanmuDeleted,
//                                             )))
//                                             .await
//                                             .unwrap();
//                                         info!("删除操作已中止");
//                                         break;
//                                     }
//
//                                     let cl_clone = Arc::clone(&cl);
//                                     let csrf_clone = Arc::clone(&csrf);
//                                     match danmu.remove(id, cl_clone, csrf_clone).await {
//                                         Ok(id) => {
//                                             output_clone
//                                                 .send(Message::Main(main::Message::DanmuMsg(
//                                                     dvmsg::DanmuDeleted { id },
//                                                 )))
//                                                 .await
//                                                 .unwrap();
//                                             pb.set_message(format!("已删除弹幕：{}", id));
//                                             pb.inc(1);
//                                         }
//                                         Err(err) => {
//                                             error!("{}", err);
//                                         }
//                                     }
//
//                                     if index + 1 == len {
//                                         output_clone
//                                             .send(Message::Main(main::Message::DanmuMsg(
//                                                 dvmsg::AllDanmuDeleted,
//                                             )))
//                                             .await
//                                             .unwrap();
//                                         pb.finish_with_message("删除完成");
//                                     }
//
//                                     sleep(Duration::from_secs_f32(seconds)).await;
//                                 }
//                             }));
//                         }
//                         ChannelMsg::StopDeleteDanmu => {
//                             danmu_delete_flag.store(false, Ordering::SeqCst);
//                             info!("停止删除通知");
//                         }
//
//                         ChannelMsg::StartRefreshQRcodeState => {
//                             qrcode_refresh_flag.store(true, Ordering::SeqCst);
//                             let qrcode_refresh_flag_clone = Arc::clone(&qrcode_refresh_flag);
//                             let mut output_clone = output.clone();
//                             spawn(async move {
//                                 while qrcode_refresh_flag_clone.load(Ordering::SeqCst) {
//                                     output_clone
//                                         .send(Message::QRCode(qrcode::Message::QRcodeRefresh))
//                                         .await
//                                         .unwrap();
//                                     sleep(Duration::from_secs(1)).await;
//                                 }
//                             });
//                         }
//                         ChannelMsg::StopRefreshQRcodeState => {
//                             qrcode_refresh_flag.store(false, Ordering::SeqCst);
//                         }
//                     }
//                 } else {
//                     panic!("Channel is closed");
//                 }
//             }
//         })
//     })
// }

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
    client: Arc<Client>,
    csrf: Arc<String>,
) where
    T: RemoveAble,
{
    if items.is_empty() {
        return;
    }

    let len = items.len();
    let pb = ProgressBar::new(len as u64);
    pb.set_style(indicatif::ProgressStyle::default_bar());

    let msg_done: Message = match tp {
        Type::Comment => cvmsg::AllCommentDeleted.into(),
        Type::Danmu => dvmsg::AllDanmuDeleted.into(),
        Type::Notify => nvmsg::AllNotifyDeleted.into(),
    };

    for (index, item) in items.into_iter().enumerate() {
        let (id, data) = item;
        if !delete_flag.load(Ordering::SeqCst) {
            output.send(msg_done.clone().into()).await.unwrap();
            break;
        }
        match data.remove(id, client.clone(), csrf.clone()).await {
            Ok(id) => {
                output
                    .send(match tp {
                        Type::Comment => {
                            Message::Main(main::Message::CommentMsg(cvmsg::CommentDeleted {
                                rpid: id,
                            }))
                        }
                        Type::Danmu => {
                            Message::Main(main::Message::DanmuMsg(dvmsg::DanmuDeleted { id }))
                        }
                        Type::Notify => {
                            Message::Main(main::Message::NotifyMsg(nvmsg::NotifyDeleted { id }))
                        }
                    })
                    .await
                    .unwrap();
                pb.set_message(format!("已删除{}：{}", tp, id));
                pb.inc(1);
                pb.inc(1);
            }
            Err(err) => {
                eprintln!("Error: {}", err);
            }
        }

        if index + 1 == len {
            output.send(msg_done.clone().into()).await.unwrap();
            pb.finish_with_message("删除完成");
        }
        sleep(std::time::Duration::from_secs_f32(seconds)).await;
    }
}

pub fn main_subscription() -> Subscription<Message> {
    Subscription::run(|| {
        stream::channel(10, |mut output| async move {
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
                    ChannelMsg::DeleteComment(cl, csrf, c, seconds) => {
                        let comments = c
                            .lock()
                            .await
                            .iter()
                            .filter(|e| e.1.is_selected)
                            .map(|(a, b)| (*a, b.clone()))
                            .collect::<Vec<_>>();

                        let flag = Arc::clone(&flags.1);
                        let output_clone = output.clone();
                        let task = tokio::spawn(handle_delete(
                            flag,
                            output_clone,
                            comments,
                            Type::Comment,
                            seconds,
                            cl.clone(),
                            csrf.clone(),
                        ));
                        tasks.0 = Some(task);
                    }
                    ChannelMsg::StopDeleteComment => {
                        flags.1.store(false, Ordering::SeqCst);
                    }
                    ChannelMsg::DeleteNotify(cl, csrf, c, seconds) => {
                        let notify = c
                            .lock()
                            .await
                            .iter()
                            .filter(|e| e.1.is_selected)
                            .map(|(a, b)| (*a, b.clone()))
                            .collect::<Vec<_>>();

                        let flag = Arc::clone(&flags.2);
                        let output_clone = output.clone();
                        let task = tokio::spawn(handle_delete(
                            flag,
                            output_clone,
                            notify,
                            Type::Notify,
                            seconds,
                            cl.clone(),
                            csrf.clone(),
                        ));
                        tasks.1 = Some(task);
                    }
                    ChannelMsg::StopDeleteNotify => {
                        flags.2.store(false, Ordering::SeqCst);
                    }
                    ChannelMsg::DeleteDanmu(cl, csrf, c, seconds) => {
                        let danmu = c
                            .lock()
                            .await
                            .iter()
                            .filter(|e| e.1.is_selected)
                            .map(|(a, b)| (*a, b.clone()))
                            .collect::<Vec<_>>();

                        let flag = Arc::clone(&flags.3);
                        let output_clone = output.clone();
                        let task = tokio::spawn(handle_delete(
                            flag,
                            output_clone,
                            danmu,
                            Type::Danmu,
                            seconds,
                            cl.clone(),
                            csrf.clone(),
                        ));
                        tasks.2 = Some(task);
                    }
                    ChannelMsg::StopDeleteDanmu => {
                        flags.3.store(false, Ordering::SeqCst);
                    }
                    ChannelMsg::StartRefreshQRcodeState => {
                        flags.0.store(true, Ordering::SeqCst);
                        let mut output_clone = output.clone();
                        let flag = Arc::clone(&flags.0);
                        tokio::spawn(async move {
                            while flag.load(Ordering::SeqCst) {
                                output_clone
                                    .send(Message::QRCode(qrcode::Message::QRcodeRefresh))
                                    .await
                                    .unwrap();
                                sleep(std::time::Duration::from_secs(1)).await;
                            }
                        });
                    }
                    ChannelMsg::StopRefreshQRcodeState => {
                        flags.0.store(false, Ordering::SeqCst);
                    }
                }
            }
            error!("Channel is closed");
        })
    })
}
