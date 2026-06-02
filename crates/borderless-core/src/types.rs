use crate::error::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use std::num::NonZeroU32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Pid(NonZeroU32);

impl Pid {
    pub fn new(value: u32) -> Option<Self> {
        NonZeroU32::new(value).map(Self)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

impl Display for Pid {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.get().fmt(f)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Hwnd(pub isize);

impl Hwnd {
    #[must_use]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }
}

impl Display for Hwnd {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:016X}", usize::from_ne_bytes(self.0.to_ne_bytes()))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MonitorId(pub isize);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Pixels(pub i32);

impl std::ops::Add for Pixels {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub for Pixels {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub left: Pixels,
    pub top: Pixels,
    pub right: Pixels,
    pub bottom: Pixels,
}

impl Rect {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> CoreResult<Self> {
        if right <= left || bottom <= top {
            return Err(CoreError::InvalidRect {
                left,
                top,
                right,
                bottom,
            });
        }
        Ok(Self {
            left: Pixels(left),
            top: Pixels(top),
            right: Pixels(right),
            bottom: Pixels(bottom),
        })
    }

    #[must_use]
    pub fn width(self) -> Pixels {
        self.right - self.left
    }

    #[must_use]
    pub fn height(self) -> Pixels {
        self.bottom - self.top
    }

    #[must_use]
    pub fn contains_point(self, x: Pixels, y: Pixels) -> bool {
        self.left <= x && self.top <= y && self.right > x && self.bottom > y
    }

    #[must_use]
    pub fn with_offsets(self, offsets: crate::action::EdgeOffsets) -> Self {
        Self {
            left: self.left + offsets.left,
            top: self.top + offsets.top,
            right: self.right + offsets.right,
            bottom: self.bottom + offsets.bottom,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProcessName(String);

impl ProcessName {
    pub fn new(value: impl Into<String>) -> CoreResult<Self> {
        let value = value.into().trim().to_owned();
        if value.is_empty() {
            Err(CoreError::Empty("process name"))
        } else {
            Ok(Self(value))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ProcessName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WindowTitle(String);

impl WindowTitle {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for WindowTitle {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FavoriteId(String);

impl FavoriteId {
    pub fn new(value: impl Into<String>) -> CoreResult<Self> {
        let value = value.into().trim().to_owned();
        if value.is_empty() {
            Err(CoreError::Empty("favorite id"))
        } else {
            Ok(Self(value))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for FavoriteId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
