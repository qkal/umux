#![cfg(target_os = "windows")]
#![allow(
    clippy::collapsible_if,
    clippy::field_reassign_with_default,
    clippy::let_unit_value,
    clippy::needless_borrow,
    clippy::needless_borrows_for_generic_args,
    clippy::needless_else,
    clippy::needless_return,
    clippy::nonminimal_bool,
    clippy::partialeq_to_none,
    clippy::ptr_arg,
    clippy::redundant_pattern_matching,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::unnecessary_mut_passed,
    clippy::upper_case_acronyms
)]

mod clipboard;
mod destination_list;
mod direct_manipulation;
mod direct_write;
mod directx_atlas;
mod directx_devices;
mod directx_renderer;
mod dispatcher;
mod display;
mod events;
mod keyboard;
mod platform;
mod system_settings;
mod util;
mod vsync;
mod window;
mod wrapper;

pub(crate) use clipboard::*;
pub(crate) use destination_list::*;
pub(crate) use direct_write::*;
pub(crate) use directx_atlas::*;
pub(crate) use directx_devices::*;
pub(crate) use directx_renderer::*;
pub(crate) use dispatcher::*;
pub(crate) use display::*;
pub(crate) use events::*;
pub(crate) use keyboard::*;
pub(crate) use platform::*;
pub(crate) use system_settings::*;
pub(crate) use util::*;
pub(crate) use vsync::*;
pub(crate) use window::*;
pub(crate) use wrapper::*;

pub use platform::WindowsPlatform;

pub(crate) use windows::Win32::Foundation::HWND;
