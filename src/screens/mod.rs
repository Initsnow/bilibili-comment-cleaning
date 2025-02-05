use crate::screens::cookie::Cookie;
use crate::screens::main::Main;
use crate::screens::qrcode::QRCode;
use std::fmt::Debug;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub mod cookie;
pub mod main;
pub mod qrcode;

#[derive(Debug)]
pub enum Screen {
    WaitScanQRcode(QRCode),
    WaitingForInputCookie(Cookie),
    Main(Main),
}

impl Screen {
    pub fn new(aicu_state: Arc<AtomicBool>) -> Self {
        Screen::WaitScanQRcode(QRCode::new(aicu_state).0)
    }
}
