// SPDX-License-Identifier: GPL-3.0-or-later

pub mod protocol;

pub use protocol::{ErrorFrame, Method, ProtocolError, RequestFrame, ResponseFrame};

pub const CRATE_NAME: &str = "umux-ipc";
