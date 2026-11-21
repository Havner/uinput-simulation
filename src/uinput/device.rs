use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, EventType, InputEvent, KeyCode, RelativeAxisCode};
use strum::IntoEnumIterator;

use super::types::{Button, Coordinate, Direction, Key, ScrollAxis, Token};

// from libc/compat, not sure why evdev doesn't reexport that
const KEY_MAX: u16 = 0x2ff;

enum DeviceType {
    Raw,
    Keyboard,
    Mouse,
}

pub(super) struct Uinput {
    raw: VirtualDevice,
    keyboard: VirtualDevice,
    mouse: VirtualDevice,
}

impl Uinput {
    pub(super) fn new() -> Result<Self, anyhow::Error> {
        let mut codes = AttributeSet::<KeyCode>::new();
        for code in 0u16..=KEY_MAX {
            codes.insert(KeyCode::new(code));
        }

        let raw = VirtualDevice::builder()?
            .name("uinput-simulation virtual raw")
            .with_keys(&codes)?
            .build()?;

        let mut keys = AttributeSet::<KeyCode>::new();
        for key in Key::iter() {
            keys.insert(KeyCode::new(key.code()));
        }

        let keyboard = VirtualDevice::builder()?
            .name("uinput-simulation virtual keyboard")
            .with_keys(&keys)?
            .build()?;

        let mut rel = AttributeSet::<RelativeAxisCode>::new();
        rel.insert(RelativeAxisCode::REL_X);
        rel.insert(RelativeAxisCode::REL_Y);
        rel.insert(RelativeAxisCode::REL_WHEEL);
        rel.insert(RelativeAxisCode::REL_HWHEEL);

        let mut buttons = AttributeSet::<KeyCode>::new();
        for btn in Button::iter() {
            buttons.insert(KeyCode::new(btn.code()));
        }

        let mouse = VirtualDevice::builder()?
            .name("uinput-simulation virtual mouse")
            .with_keys(&buttons)?
            .with_relative_axes(&rel)?
            .build()?;


        Ok(Self { raw, keyboard, mouse })
    }

    pub(super) fn execute(&mut self, token: Token) -> anyhow::Result<()> {
        match token {
            Token::Key(k, d) => self.key(k, d),
            Token::KeyCode(kc, d) => self.key_code(kc, d),
            Token::Raw(c, d) => self.raw(c, d, DeviceType::Raw),
            Token::Button(b, d) => self.button(b, d),
            Token::MoveMouse(x, y, c) => self.move_mouse(x, y, c),
            Token::Scroll(l, a) => self.scroll(l, a),
            Token::Delay(ms) => {
                std::thread::sleep(std::time::Duration::from_millis(ms));
                Ok(())
            },
        }
    }

    fn key(&mut self, key: Key, direction: Direction) -> anyhow::Result<()> {
        self.raw(key.code(), direction, DeviceType::Keyboard)
    }

    fn key_code(&mut self, key_code: KeyCode, direction: Direction) -> anyhow::Result<()> {
        self.raw(key_code.0, direction, DeviceType::Raw)
    }

    fn raw(&mut self, keycode: u16, direction: Direction, dev: DeviceType) -> anyhow::Result<()> {
        let events: &[InputEvent] = match direction {
            Direction::Click => &[
                InputEvent::new(EventType::KEY.0, keycode, 1),
                InputEvent::new(EventType::KEY.0, keycode, 0),
            ],
            Direction::Press => &[InputEvent::new(EventType::KEY.0, keycode, 1)],
            Direction::Release => &[InputEvent::new(EventType::KEY.0, keycode, 0)],
        };

        match dev {
            DeviceType::Raw => self.raw.emit(events)?,
            DeviceType::Keyboard => self.keyboard.emit(events)?,
            DeviceType::Mouse => self.mouse.emit(events)?,
        }
        Ok(())
    }

    fn button(&mut self, button: Button, direction: Direction) -> anyhow::Result<()> {
        let keycode = button.code();
        match direction {
            // for some strange reason click doesn't work for mouse in one emit
            Direction::Click => {
                self.raw(keycode, Direction::Press, DeviceType::Mouse)?;
                self.raw(keycode, Direction::Release, DeviceType::Mouse)
            }
            _ => self.raw(keycode, direction, DeviceType::Mouse),
        }
    }

    fn move_mouse(&mut self, x: i32, y: i32, coordinate: Coordinate) -> anyhow::Result<()> {
        // uinput doesn't know about screen coordinates, simulate absolute
        if coordinate == Coordinate::Absolute {
            let top_left_corner: &[InputEvent] = &[
                InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_X.0, i32::MIN),
                InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_Y.0, i32::MIN),
            ];
            self.mouse.emit(top_left_corner)?;
        }

        let events: &[InputEvent] = &[
            InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_X.0, x),
            InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_Y.0, y),
        ];
        self.mouse.emit(events)?;

        Ok(())
    }

    fn scroll(&mut self, length: i32, axis: ScrollAxis) -> anyhow::Result<()> {
        let axis = match axis {
            ScrollAxis::Vertical => RelativeAxisCode::REL_WHEEL.0,
            ScrollAxis::Horizontal => RelativeAxisCode::REL_HWHEEL.0,
        };

        let events: &[InputEvent] = &[
            InputEvent::new(EventType::RELATIVE.0, axis, length),
            InputEvent::new(EventType::RELATIVE.0, axis, length),
        ];
        self.mouse.emit(events)?;

        Ok(())
    }
}
