// use crate::{App, State};
// use bilibili_comment_cleaning::types::Message;
// use iced::{
//     widget::{center, column, horizontal_rule, image, progress_bar, row, text},
//     Alignment, Element, Length, Task,
// };
//
// pub fn view<'a>(
//     img: &'static [u8],
//     aicu_progress: &'a Option<(f32, f32)>,
//     offcial_msg: &'a Option<String>,
// ) -> Element<'a, Message> {
//     center(
//         column![
//             image(image::Handle::from_bytes(img)).height(Length::FillPortion(2)),
//             text("Fetching").height(Length::FillPortion(1))
//         ]
//         .push_maybe(aicu_progress.map(|e| {
//             column![
//                 "Fetching from aicu.cc:",
//                 row![
//                     progress_bar(0.0..=e.1, e.0),
//                     text(format!("({}/{})", e.0, e.1))
//                 ]
//                 .spacing(5)
//                 .align_y(Alignment::Center)
//             ]
//         }))
//         .push_maybe(offcial_msg.clone().map(|e| {
//             column![
//                 horizontal_rule(0.5),
//                 text("Fetching from official:"),
//                 text(e).shaping(text::Shaping::Advanced)
//             ]
//         }))
//         .padding(20)
//         .spacing(10)
//         .align_x(Alignment::Center),
//     )
//     .into()
// }
//
// pub fn update(main: &mut App, msg: Message) -> Task<Message> {
//     match msg {
//         Message::CommentsFetched(comments) => {
//             main.comments = Some(comments);
//             main.state = State::CommentsFetched;
//         }
//         Message::AicuFetchingState { now, max } => {
//             if let State::Fetching {
//                 ref mut aicu_progress,
//                 ..
//             } = main.state
//             {
//                 *aicu_progress = Some((now, max));
//             }
//         }
//         Message::OfficialFetchingState(s) => {
//             if let State::Fetching {
//                 ref mut offcial_msg,
//                 ..
//             } = main.state
//             {
//                 *offcial_msg = Some(s);
//             }
//         }
//         _ => {}
//     }
//
//     Task::none()
// }
