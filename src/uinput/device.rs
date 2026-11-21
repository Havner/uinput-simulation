use evdev::uinput::VirtualDevice;
use evdev::{
    AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, EventType, InputEvent, InputId, KeyCode,
    RelativeAxisCode, UinputAbsSetup,
};
use strum::IntoEnumIterator;

use super::types::{Button, Coordinate, Direction, Key, ScrollAxis, Token};

// from libc/compat, not sure why evdev doesn't reexport that
const KEY_MAX: u16 = 0x2ff;

// direct input maximum
const JOYSTICK_BUTTONS: usize = 128;

// button needs to be pressed for some time so the polling works
const JOYSTICK_CLICK_HOLD: std::time::Duration = std::time::Duration::from_millis(50);

// Key codes used for the joystick buttons, in ascending order, so that SDL, Wine (evdev
// backend) and joydev all number them the same way (they enumerate the key bitmap upwards).
// Constraints, checked against the udev, SDL2/3, Wine/Proton and kernel sources:
// - only codes udev/SDL don't consider keyboard keys (< 0x100, 0x160-0x21f, 0x230-0x2bf are
//   keys and would make the device a keyboard too), so it's ID_INPUT_JOYSTICK only and
//   libinput (GNOME) ignores it;
// - no 0x100-0x11f: mouse buttons, and SDL numbers these after 0x120+ while Wine/joydev
//   number them first;
// - no BTN_SOUTH (0x130): SDL would auto-create a gamepad mapping and Wine would expose the
//   device as an XInput gamepad instead of a DirectInput joystick;
// - no BTN_DIGI (0x140) or BTN_TOUCH (0x14a): joydev refuses such devices;
// - no KEY_MAX (0x2ff): SDL and Wine both stop below it.
// That leaves 126 codes in joystick ranges, the last 2 come from the 0x14x block.
const JOYSTICK_CODE_RANGES: &[(u16, u16)] = &[
    (0x120, 0x12f), // BTN_TRIGGER .. BTN_DEAD
    (0x131, 0x13f), // BTN_EAST .. 0x13f (BTN_GAMEPAD block without BTN_SOUTH)
    (0x14e, 0x14f), // BTN_TOOL_TRIPLETAP, BTN_TOOL_QUADTAP
    (0x150, 0x15f), // BTN_GEAR_DOWN, BTN_GEAR_UP, unnamed
    (0x220, 0x22f), // BTN_DPAD_UP .. unnamed
    (0x2c0, 0x2fe), // BTN_TRIGGER_HAPPY1 .. BTN_TRIGGER_HAPPY63
];

fn joystick_codes() -> Vec<u16> {
    let codes: Vec<u16> = JOYSTICK_CODE_RANGES.iter().flat_map(|&(a, b)| a..=b).collect();
    debug_assert_eq!(codes.len(), JOYSTICK_BUTTONS);
    codes
}

enum DeviceType {
    Raw,
    Keyboard,
    Mouse,
    Joystick,
}

pub(super) struct Uinput {
    raw: VirtualDevice,
    keyboard: VirtualDevice,
    mouse: VirtualDevice,
    joystick: VirtualDevice,
    joystick_codes: Vec<u16>,
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

        let joystick_codes = joystick_codes();
        let mut joy_buttons = AttributeSet::<KeyCode>::new();
        for &code in &joystick_codes {
            joy_buttons.insert(KeyCode::new(code));
        }

        // 1209:0001 is the test PID, meant for private use.
        // Two axes (X/Y) are for detection purposes only, so proton can figure
        // out it's a joystick even without udev access (proton in a container).
        let idle_axis = AbsInfo::new(0, -32767, 32767, 0, 0, 0);
        let joystick = VirtualDevice::builder()?
            .name("uinput-simulation virtual joystick")
            .input_id(InputId::new(BusType::BUS_VIRTUAL, 0x1209, 0x0001, 1))
            .with_keys(&joy_buttons)?
            .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_X, idle_axis))?
            .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_Y, idle_axis))?
            .build()?;

        Ok(Self { raw, keyboard, mouse, joystick, joystick_codes })
    }

    pub(super) fn execute(&mut self, token: Token) -> anyhow::Result<()> {
        match token {
            Token::Key(k, d) => self.key(k, d),
            Token::KeyCode(kc, d) => self.key_code(kc, d),
            Token::Raw(c, d) => self.raw(c, d, DeviceType::Raw),
            Token::Button(b, d) => self.button(b, d),
            Token::MoveMouse(x, y, c) => self.move_mouse(x, y, c),
            Token::Scroll(l, a) => self.scroll(l, a),
            Token::Joystick(j, d) => self.joystick(j, d),
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
            DeviceType::Joystick => self.joystick.emit(events)?,
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

    // joy_button is 1-based (1..=128)
    fn joystick(&mut self, joy_button: u16, direction: Direction) -> anyhow::Result<()> {
        let code = joy_button
            .checked_sub(1)
            .and_then(|i| self.joystick_codes.get(i as usize))
            .copied()
            .ok_or_else(|| anyhow::anyhow!("joystick button {joy_button} out of range 1..={JOYSTICK_BUTTONS}"))?;
        match direction {
            Direction::Click => {
                self.raw(code, Direction::Press, DeviceType::Joystick)?;
                std::thread::sleep(JOYSTICK_CLICK_HOLD);
                self.raw(code, Direction::Release, DeviceType::Joystick)
            }
            _ => self.raw(code, direction, DeviceType::Joystick),
        }
    }
}
