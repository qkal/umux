// SPDX-License-Identifier: GPL-3.0-or-later

use thiserror::Error;

pub const CRATE_NAME: &str = "umux-win32";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWindowHandle(pub isize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformCapabilities {
    pub webview2_runtime_expected: bool,
    pub conpty_expected: bool,
    pub named_pipes_expected: bool,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum Win32Error {
    #[error("window handle must not be null")]
    NullWindowHandle,
}

pub fn platform_capabilities() -> PlatformCapabilities {
    PlatformCapabilities {
        webview2_runtime_expected: true,
        conpty_expected: true,
        named_pipes_expected: true,
    }
}

pub fn require_hwnd(handle: NativeWindowHandle) -> Result<NativeWindowHandle, Win32Error> {
    if handle.0 == 0 {
        Err(Win32Error::NullWindowHandle)
    } else {
        Ok(handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_capabilities_expect_native_foundation_runtime() {
        let capabilities = platform_capabilities();

        assert!(capabilities.webview2_runtime_expected);
        assert!(capabilities.conpty_expected);
        assert!(capabilities.named_pipes_expected);
    }

    #[test]
    fn require_hwnd_rejects_null_handle() {
        assert_eq!(
            require_hwnd(NativeWindowHandle(0)),
            Err(Win32Error::NullWindowHandle)
        );
        assert_eq!(
            require_hwnd(NativeWindowHandle(42)),
            Ok(NativeWindowHandle(42))
        );
    }
}
