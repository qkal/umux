// SPDX-License-Identifier: GPL-3.0-or-later

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalKey {
    Character(char),
    Enter,
    Backspace,
    Escape,
    Tab,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalKeyEvent {
    pub key: TerminalKey,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub selection_present: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalInputRoute {
    CopySelection,
    PasteClipboard,
    WriteBytes(Vec<u8>),
    Ignore,
}

pub struct TerminalInputRouter;

impl TerminalInputRouter {
    pub fn route_key(event: TerminalKeyEvent) -> TerminalInputRoute {
        match event {
            TerminalKeyEvent {
                key: TerminalKey::Character('c' | 'C'),
                ctrl: true,
                shift: true,
                selection_present: true,
                ..
            } => TerminalInputRoute::CopySelection,
            TerminalKeyEvent {
                key: TerminalKey::Character('v' | 'V'),
                ctrl: true,
                shift: true,
                ..
            } => TerminalInputRoute::PasteClipboard,
            TerminalKeyEvent {
                key: TerminalKey::Character(ch),
                ctrl: false,
                alt: false,
                ..
            } => {
                let mut bytes = [0; 4];
                TerminalInputRoute::WriteBytes(ch.encode_utf8(&mut bytes).as_bytes().to_vec())
            }
            TerminalKeyEvent {
                key: TerminalKey::Enter,
                ..
            } => TerminalInputRoute::WriteBytes(b"\r".to_vec()),
            TerminalKeyEvent {
                key: TerminalKey::Tab,
                ..
            } => TerminalInputRoute::WriteBytes(b"\t".to_vec()),
            TerminalKeyEvent {
                key: TerminalKey::Backspace,
                ..
            } => TerminalInputRoute::WriteBytes(vec![0x7f]),
            TerminalKeyEvent {
                key: TerminalKey::Escape,
                ..
            } => TerminalInputRoute::WriteBytes(vec![0x1b]),
            _ => TerminalInputRoute::Ignore,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_shift_c_copies_when_selection_exists() {
        let route = TerminalInputRouter::route_key(TerminalKeyEvent {
            key: TerminalKey::Character('c'),
            ctrl: true,
            shift: true,
            alt: false,
            selection_present: true,
        });

        assert_eq!(route, TerminalInputRoute::CopySelection);
    }

    #[test]
    fn ordinary_character_writes_utf8() {
        let route = TerminalInputRouter::route_key(TerminalKeyEvent {
            key: TerminalKey::Character('x'),
            ctrl: false,
            shift: false,
            alt: false,
            selection_present: false,
        });

        assert_eq!(route, TerminalInputRoute::WriteBytes(vec![b'x']));
    }

    #[test]
    fn ctrl_shift_v_pastes_clipboard() {
        let route = TerminalInputRouter::route_key(TerminalKeyEvent {
            key: TerminalKey::Character('v'),
            ctrl: true,
            shift: true,
            alt: false,
            selection_present: false,
        });

        assert_eq!(route, TerminalInputRoute::PasteClipboard);
    }

    #[test]
    fn ctrl_shift_c_without_selection_ignores() {
        let route = TerminalInputRouter::route_key(TerminalKeyEvent {
            key: TerminalKey::Character('c'),
            ctrl: true,
            shift: true,
            alt: false,
            selection_present: false,
        });

        assert_eq!(route, TerminalInputRoute::Ignore);
    }

    #[test]
    fn alt_modified_clipboard_shortcuts_follow_plan_routes() {
        let copy_route = TerminalInputRouter::route_key(TerminalKeyEvent {
            key: TerminalKey::Character('c'),
            ctrl: true,
            shift: true,
            alt: true,
            selection_present: true,
        });
        let paste_route = TerminalInputRouter::route_key(TerminalKeyEvent {
            key: TerminalKey::Character('v'),
            ctrl: true,
            shift: true,
            alt: true,
            selection_present: false,
        });

        assert_eq!(copy_route, TerminalInputRoute::CopySelection);
        assert_eq!(paste_route, TerminalInputRoute::PasteClipboard);
    }

    #[test]
    fn special_keys_write_expected_bytes() {
        let cases = [
            (
                TerminalKeyEvent {
                    key: TerminalKey::Enter,
                    ctrl: false,
                    shift: false,
                    alt: false,
                    selection_present: false,
                },
                b"\r".to_vec(),
            ),
            (
                TerminalKeyEvent {
                    key: TerminalKey::Tab,
                    ctrl: false,
                    shift: false,
                    alt: false,
                    selection_present: false,
                },
                b"\t".to_vec(),
            ),
            (
                TerminalKeyEvent {
                    key: TerminalKey::Backspace,
                    ctrl: false,
                    shift: false,
                    alt: false,
                    selection_present: false,
                },
                vec![0x7f],
            ),
            (
                TerminalKeyEvent {
                    key: TerminalKey::Escape,
                    ctrl: false,
                    shift: false,
                    alt: false,
                    selection_present: false,
                },
                vec![0x1b],
            ),
            (
                TerminalKeyEvent {
                    key: TerminalKey::Enter,
                    ctrl: true,
                    shift: false,
                    alt: false,
                    selection_present: false,
                },
                b"\r".to_vec(),
            ),
            (
                TerminalKeyEvent {
                    key: TerminalKey::Tab,
                    ctrl: false,
                    shift: true,
                    alt: false,
                    selection_present: false,
                },
                b"\t".to_vec(),
            ),
            (
                TerminalKeyEvent {
                    key: TerminalKey::Backspace,
                    ctrl: false,
                    shift: false,
                    alt: true,
                    selection_present: false,
                },
                vec![0x7f],
            ),
            (
                TerminalKeyEvent {
                    key: TerminalKey::Escape,
                    ctrl: true,
                    shift: true,
                    alt: true,
                    selection_present: false,
                },
                vec![0x1b],
            ),
        ];

        for (event, bytes) in cases {
            let route = TerminalInputRouter::route_key(event);
            assert_eq!(route, TerminalInputRoute::WriteBytes(bytes));
        }
    }
}
