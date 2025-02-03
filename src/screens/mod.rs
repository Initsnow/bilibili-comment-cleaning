use crate::screens::cookie::Cookie;
use crate::screens::main::Main;
use crate::screens::qrcode::QRCode;
use std::fmt::{Debug, Formatter};

pub mod cookie;
pub mod fetching;
pub mod main;
pub mod qrcode;

#[derive(Debug)]
pub enum Screen {
    WaitScanQRcode(QRCode),
    WaitingForInputCookie(Cookie),
    Main(Main),
}

impl Default for Screen {
    fn default() -> Self {
        Screen::WaitScanQRcode(QRCode::default())
    }
    // fn default() -> Self {
    //     Screen::Main(Main::new())
    // }
}
