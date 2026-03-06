use evdev::KeyCode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub(super) enum Direction {
    #[serde(alias = "P")]
    #[serde(alias = "p")]
    #[serde(alias = "Pressed")]
    #[serde(alias = "pressed")]
    Press,
    #[serde(alias = "R")]
    #[serde(alias = "r")]
    #[serde(alias = "Released")]
    #[serde(alias = "released")]
    Release,
    #[serde(alias = "C")]
    #[serde(alias = "c")]
    #[serde(alias = "Clicked")]
    #[serde(alias = "clicked")]
    #[default]
    Click,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub(super) enum Coordinate {
    #[serde(alias = "A")]
    #[serde(alias = "a")]
    #[default]
    Absolute,
    #[serde(alias = "R")]
    #[serde(alias = "r")]
    Relative,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub(super) enum ScrollAxis {
    #[serde(alias = "H")]
    #[serde(alias = "h")]
    Horizontal,
    #[serde(alias = "V")]
    #[serde(alias = "v")]
    #[default]
    Vertical,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(super) enum Token {
    #[serde(alias = "KC")]
    #[serde(alias = "kc")]
    KeyCode(KeyCode, #[serde(default)] Direction),
    #[serde(alias = "R")]
    #[serde(alias = "r")]
    Raw(u16, #[serde(default)] Direction),
    #[serde(alias = "M")]
    #[serde(alias = "m")]
    MoveMouse(i32, i32, #[serde(default)] Coordinate),
    #[serde(alias = "S")]
    #[serde(alias = "s")]
    Scroll(i32, #[serde(default)] ScrollAxis),
}
