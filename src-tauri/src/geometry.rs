use serde::{Deserialize, Serialize};

/// A rectangle in virtual-desktop **physical** pixels (top-left origin).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub const ZERO: Rect = Rect {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
    };

    pub fn right(&self) -> i32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.height
    }

    /// Shrinks the rect by `insets`, never past zero size.
    pub fn inset(self, insets: Insets) -> Rect {
        Rect {
            x: self.x + insets.left,
            y: self.y + insets.top,
            width: (self.width - insets.left - insets.right).max(0),
            height: (self.height - insets.top - insets.bottom).max(0),
        }
    }
}

/// Edge margins in **logical** pixels (i.e. CSS pixels, DPI-independent).
///
/// The HUD draws its chrome inside these margins; the window manager
/// (milestone 4) tiles app windows into what is left. Convert with
/// [`Insets::to_physical`] before handing them to Win32.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Insets {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

impl Insets {
    pub fn to_physical(self, scale_factor: f64) -> Insets {
        let s = |v: i32| (v as f64 * scale_factor).round() as i32;
        Insets {
            top: s(self.top),
            right: s(self.right),
            bottom: s(self.bottom),
            left: s(self.left),
        }
    }
}
