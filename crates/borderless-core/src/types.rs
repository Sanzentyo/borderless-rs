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
pub struct PhysicalPx(pub i32);

pub use PhysicalPx as Pixels;

impl std::ops::Add for PhysicalPx {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub for PhysicalPx {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Dip(pub f64);

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ScaleFactor(pub f64);

impl Default for ScaleFactor {
    fn default() -> Self {
        Self(1.0)
    }
}

impl ScaleFactor {
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        value
            .is_finite()
            .then_some(value)
            .filter(|value| *value > 0.0)
            .map(Self)
    }

    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhysicalRect {
    pub left: PhysicalPx,
    pub top: PhysicalPx,
    pub right: PhysicalPx,
    pub bottom: PhysicalPx,
}

pub use PhysicalRect as Rect;

impl PhysicalRect {
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
            left: PhysicalPx(left),
            top: PhysicalPx(top),
            right: PhysicalPx(right),
            bottom: PhysicalPx(bottom),
        })
    }

    #[must_use]
    pub fn width(self) -> PhysicalPx {
        self.right - self.left
    }

    #[must_use]
    pub fn height(self) -> PhysicalPx {
        self.bottom - self.top
    }

    #[must_use]
    pub fn contains_point(self, x: PhysicalPx, y: PhysicalPx) -> bool {
        self.left <= x && self.top <= y && self.right > x && self.bottom > y
    }

    #[must_use]
    pub fn intersection_area(self, other: Self) -> i64 {
        let left = self.left.max(other.left).0;
        let top = self.top.max(other.top).0;
        let right = self.right.min(other.right).0;
        let bottom = self.bottom.min(other.bottom).0;
        if right <= left || bottom <= top {
            0
        } else {
            i64::from(right - left) * i64::from(bottom - top)
        }
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DipRect {
    pub left: Dip,
    pub top: Dip,
    pub right: Dip,
    pub bottom: Dip,
}

impl DipRect {
    #[must_use]
    pub const fn new(left: f64, top: f64, right: f64, bottom: f64) -> Self {
        Self {
            left: Dip(left),
            top: Dip(top),
            right: Dip(right),
            bottom: Dip(bottom),
        }
    }

    #[must_use]
    pub fn from_physical(rect: PhysicalRect, scale: ScaleFactor) -> Self {
        let scale = scale.value();
        Self::new(
            f64::from(rect.left.0) / scale,
            f64::from(rect.top.0) / scale,
            f64::from(rect.right.0) / scale,
            f64::from(rect.bottom.0) / scale,
        )
    }

    #[must_use]
    pub fn to_physical(self, scale: ScaleFactor) -> Option<PhysicalRect> {
        let scale = scale.value();
        PhysicalRect::new(
            round_dip_to_i32(self.left, scale)?,
            round_dip_to_i32(self.top, scale)?,
            round_dip_to_i32(self.right, scale)?,
            round_dip_to_i32(self.bottom, scale)?,
        )
        .ok()
    }
}

fn round_dip_to_i32(value: Dip, scale: f64) -> Option<i32> {
    let scaled = value.0 * scale;
    scaled
        .is_finite()
        .then_some(scaled.round())
        .filter(|value| *value >= f64::from(i32::MIN) && *value <= f64::from(i32::MAX))
        .and_then(|value| format!("{value:.0}").parse::<i32>().ok())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dip_rect_round_trips_with_scale_factor() {
        let physical = PhysicalRect::new(100, 200, 500, 800).unwrap();
        let scale = ScaleFactor::new(2.0).unwrap();

        let dip = DipRect::from_physical(physical, scale);

        assert_eq!(dip, DipRect::new(50.0, 100.0, 250.0, 400.0));
        assert_eq!(dip.to_physical(scale), Some(physical));
    }

    #[test]
    fn invalid_scale_factor_is_rejected() {
        assert_eq!(ScaleFactor::new(0.0), None);
        assert_eq!(ScaleFactor::new(f64::NAN), None);
    }
}
