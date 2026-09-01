//! Tiling geometry.
//!
//! Deliberately pure: rectangles in, rectangles out, no Win32 anywhere. Every
//! layout decision is unit-testable, which matters because this is the part
//! that gets fiddled with most and the part whose bugs are most visible.

use serde::{Deserialize, Serialize};

use crate::geometry::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LayoutKind {
    /// One large pane plus a vertical stack of the rest. The default: it maps
    /// onto how editors are actually used — one thing you are working in, a
    /// few things you are watching.
    MainStack,
    /// Equal vertical columns.
    Columns,
    /// Roughly square grid.
    Grid,
    /// One window at a time, full region.
    Monocle,
    /// Hands off: windows keep whatever geometry they had.
    Float,
}

impl LayoutKind {
    pub const CYCLE: [LayoutKind; 4] = [
        LayoutKind::MainStack,
        LayoutKind::Columns,
        LayoutKind::Grid,
        LayoutKind::Monocle,
    ];

    /// Float is not in the cycle: it is a deliberate escape, not a step.
    pub fn next(self) -> Self {
        match Self::CYCLE.iter().position(|k| *k == self) {
            Some(index) => Self::CYCLE[(index + 1) % Self::CYCLE.len()],
            None => LayoutKind::MainStack,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LayoutParams {
    /// Space between tiles, in physical pixels.
    pub gap: i32,
    /// Share of the region the main pane takes in `MainStack`, 0.2–0.8.
    pub main_ratio: f32,
}

impl Default for LayoutParams {
    fn default() -> Self {
        Self {
            gap: 8,
            main_ratio: 0.6,
        }
    }
}

/// Rectangles for `count` windows inside `region`, in stacking order.
///
/// `Float` returns nothing: an empty result means "leave these windows alone",
/// which is different from "give them zero size".
pub fn arrange(region: Rect, count: usize, kind: LayoutKind, params: &LayoutParams) -> Vec<Rect> {
    if count == 0 || kind == LayoutKind::Float {
        return Vec::new();
    }
    if region.width <= 0 || region.height <= 0 {
        return Vec::new();
    }

    match kind {
        LayoutKind::Float => Vec::new(),
        LayoutKind::Monocle => vec![region; count],
        LayoutKind::Columns => split_horizontally(region, count, params.gap),
        LayoutKind::Grid => grid(region, count, params.gap),
        LayoutKind::MainStack => {
            if count == 1 {
                return vec![region];
            }
            let ratio = params.main_ratio.clamp(0.2, 0.8);
            let main_width = ((region.width - params.gap) as f32 * ratio)
                .round()
                .max(1.0) as i32;

            let main = Rect {
                width: main_width,
                ..region
            };
            let stack_region = Rect {
                x: region.x + main_width + params.gap,
                width: (region.width - main_width - params.gap).max(1),
                ..region
            };

            let mut rects = vec![main];
            rects.extend(split_vertically(stack_region, count - 1, params.gap));
            rects
        }
    }
}

/// Divides `total` into `count` parts whose sizes differ by at most one pixel,
/// so tiles fill the region exactly instead of leaving a rounding gap.
fn even_spans(start: i32, total: i32, count: usize, gap: i32) -> Vec<(i32, i32)> {
    let count_i32 = count as i32;
    let usable = (total - gap * (count_i32 - 1)).max(count_i32);
    let base = usable / count_i32;
    let remainder = usable % count_i32;

    let mut spans = Vec::with_capacity(count);
    let mut offset = start;
    for index in 0..count {
        // Spread the remainder over the first tiles rather than dumping it on
        // the last one.
        let size = base + if (index as i32) < remainder { 1 } else { 0 };
        spans.push((offset, size));
        offset += size + gap;
    }
    spans
}

fn split_horizontally(region: Rect, count: usize, gap: i32) -> Vec<Rect> {
    even_spans(region.x, region.width, count, gap)
        .into_iter()
        .map(|(x, width)| Rect { x, width, ..region })
        .collect()
}

fn split_vertically(region: Rect, count: usize, gap: i32) -> Vec<Rect> {
    even_spans(region.y, region.height, count, gap)
        .into_iter()
        .map(|(y, height)| Rect {
            y,
            height,
            ..region
        })
        .collect()
}

fn grid(region: Rect, count: usize, gap: i32) -> Vec<Rect> {
    let columns = (count as f64).sqrt().ceil() as usize;
    let rows = count.div_ceil(columns);

    let mut rects = Vec::with_capacity(count);
    let row_spans = even_spans(region.y, region.height, rows, gap);

    for (row_index, (y, height)) in row_spans.into_iter().enumerate() {
        let remaining = count - row_index * columns;
        if remaining == 0 {
            break;
        }
        // A short final row stretches to fill the width rather than leaving a
        // ragged hole.
        let in_row = remaining.min(columns);
        let row = Rect {
            y,
            height,
            ..region
        };
        rects.extend(split_horizontally(row, in_row, gap));
    }
    rects
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGION: Rect = Rect {
        x: 100,
        y: 50,
        width: 1600,
        height: 900,
    };

    fn params() -> LayoutParams {
        LayoutParams {
            gap: 8,
            main_ratio: 0.6,
        }
    }

    fn overlaps(a: &Rect, b: &Rect) -> bool {
        a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
    }

    fn assert_inside_and_disjoint(rects: &[Rect]) {
        for rect in rects {
            assert!(
                rect.width > 0 && rect.height > 0,
                "degenerate tile {rect:?}"
            );
            assert!(
                rect.x >= REGION.x
                    && rect.y >= REGION.y
                    && rect.right() <= REGION.right()
                    && rect.bottom() <= REGION.bottom(),
                "tile {rect:?} escapes the region"
            );
        }
        for (i, a) in rects.iter().enumerate() {
            for b in &rects[i + 1..] {
                assert!(!overlaps(a, b), "tiles overlap: {a:?} vs {b:?}");
            }
        }
    }

    #[test]
    fn every_layout_tiles_without_overlap_or_overflow() {
        for kind in [LayoutKind::MainStack, LayoutKind::Columns, LayoutKind::Grid] {
            for count in 1..=10 {
                let rects = arrange(REGION, count, kind, &params());
                assert_eq!(rects.len(), count, "{kind:?} with {count} windows");
                assert_inside_and_disjoint(&rects);
            }
        }
    }

    #[test]
    fn columns_fill_the_region_exactly() {
        let rects = arrange(REGION, 3, LayoutKind::Columns, &params());
        assert_eq!(rects[0].x, REGION.x);
        assert_eq!(rects[2].right(), REGION.right());
        // Rounding is spread, never left as a gap at the end.
        let covered: i32 = rects.iter().map(|r| r.width).sum::<i32>() + 8 * 2;
        assert_eq!(covered, REGION.width);
    }

    #[test]
    fn main_stack_gives_the_main_pane_its_ratio() {
        let rects = arrange(REGION, 3, LayoutKind::MainStack, &params());
        let expected = ((REGION.width - 8) as f32 * 0.6).round() as i32;
        assert_eq!(rects[0].width, expected);
        assert_eq!(rects[0].height, REGION.height);
        assert_eq!(rects[1].x, REGION.x + expected + 8);
        assert_eq!(rects[2].bottom(), REGION.bottom());
    }

    #[test]
    fn a_single_window_gets_the_whole_region_in_every_tiling_layout() {
        for kind in [
            LayoutKind::MainStack,
            LayoutKind::Columns,
            LayoutKind::Grid,
            LayoutKind::Monocle,
        ] {
            assert_eq!(
                arrange(REGION, 1, kind, &params()),
                vec![REGION],
                "{kind:?}"
            );
        }
    }

    #[test]
    fn monocle_stacks_every_window_on_the_region() {
        let rects = arrange(REGION, 4, LayoutKind::Monocle, &params());
        assert!(rects.iter().all(|r| *r == REGION));
    }

    #[test]
    fn float_and_empty_sets_produce_no_geometry() {
        assert!(arrange(REGION, 5, LayoutKind::Float, &params()).is_empty());
        assert!(arrange(REGION, 0, LayoutKind::Grid, &params()).is_empty());
    }

    #[test]
    fn grid_stretches_a_short_final_row() {
        // 5 windows: 3 columns, so the second row holds 2 stretched tiles.
        let rects = arrange(REGION, 5, LayoutKind::Grid, &params());
        assert_eq!(rects.len(), 5);
        assert_eq!(rects[3].right() + 8 + rects[4].width, REGION.right());
        assert_inside_and_disjoint(&rects);
    }

    #[test]
    fn degenerate_regions_are_refused_rather_than_producing_junk() {
        let flat = Rect { width: 0, ..REGION };
        assert!(arrange(flat, 3, LayoutKind::Grid, &params()).is_empty());
    }

    #[test]
    fn cycle_skips_float_and_wraps() {
        assert_eq!(LayoutKind::MainStack.next(), LayoutKind::Columns);
        assert_eq!(LayoutKind::Monocle.next(), LayoutKind::MainStack);
        assert_eq!(LayoutKind::Float.next(), LayoutKind::MainStack);
    }
}
