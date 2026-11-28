//! Static widget composition and the two-pass layout.
//!
//! Widget *values* are cheap descriptors that may be rebuilt every frame; the
//! retained part is their `State`, which lives as long as the screen state and
//! remembers the arranged rectangles.

use crate::dirty::DirtyList;
use crate::event::{Event, EventOut};
use crate::geom::{Rect, Size};
use crate::paint::PaintCx;
use embedded_graphics::pixelcolor::Rgb565;

pub mod highlight;
pub mod marquee;
pub mod slider;
pub mod toast;
pub mod toggle;

pub use highlight::{Highlight, HighlightState};
pub use marquee::{Marquee, MarqueeState};
pub use slider::{Slider, SliderState};
pub use toast::{Toast, ToastState};
pub use toggle::{Toggle, ToggleState};

/// Constraints handed to [`Widget::measure`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MeasureCx {
    /// Maximum space available to the widget.
    pub max: Size,
}

impl MeasureCx {
    /// Creates constraints from a maximum size.
    pub const fn new(max: Size) -> Self {
        Self { max }
    }
}

/// A retained widget with per-screen state.
pub trait Widget {
    /// Retained state; must be constructible by default.
    type State: Default;

    /// Preferred size given the constraints.
    fn measure(&self, cx: &MeasureCx, state: &Self::State) -> Size;

    /// Writes absolute coordinates into `state`.
    fn arrange(&self, bounds: Rect, state: &mut Self::State);

    /// Handles an input event.
    fn event(&self, event: &Event, state: &mut Self::State, out: &mut EventOut, now_us: u64) {
        let _ = (event, state, out, now_us);
    }

    /// Advances animations; returns whether a frame is still needed.
    ///
    /// Animated widgets mark every rectangle whose pixels they changed, so the
    /// renderer can send just that band.
    fn tick(&self, state: &mut Self::State, now_us: u64, dirty: &mut DirtyList) -> bool {
        let _ = (state, now_us, dirty);
        false
    }

    /// Paints into the current band.
    fn paint(&self, cx: &mut PaintCx<'_>, state: &Self::State, now_us: u64);

    /// Whether the widget wants the leftover main-axis space.
    fn flexible(&self) -> bool {
        false
    }
}

/// Main-axis alignment inside a stack.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MainAlign {
    /// Children packed at the start.
    #[default]
    Start,
    /// Children centred in the leftover space.
    Center,
    /// Children packed at the end.
    End,
}

/// Cross-axis alignment inside a stack.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CrossAlign {
    /// Children keep their measured cross size at the start.
    Start,
    /// Children keep their measured cross size, centred.
    Center,
    /// Children stretch to the full cross size.
    #[default]
    Stretch,
}

/// Two widgets stacked vertically.
#[derive(Clone, Copy, Debug)]
pub struct VStack<A, B> {
    /// Top widget.
    pub top: A,
    /// Bottom widget.
    pub bottom: B,
    /// Gap between them in pixels.
    pub gap: u16,
    /// Main-axis alignment.
    pub main: MainAlign,
    /// Cross-axis alignment.
    pub cross: CrossAlign,
}

impl<A, B> VStack<A, B> {
    /// Creates a stack with no gap.
    pub const fn new(top: A, bottom: B) -> Self {
        Self {
            top,
            bottom,
            gap: 0,
            main: MainAlign::Start,
            cross: CrossAlign::Stretch,
        }
    }

    /// Sets the gap.
    pub const fn gap(mut self, gap: u16) -> Self {
        self.gap = gap;
        self
    }

    /// Sets the main-axis alignment.
    pub const fn main(mut self, align: MainAlign) -> Self {
        self.main = align;
        self
    }

    /// Sets the cross-axis alignment.
    pub const fn cross(mut self, align: CrossAlign) -> Self {
        self.cross = align;
        self
    }
}

/// Retained state of a [`VStack`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VStackState<A, B> {
    /// State of the top widget.
    pub top: A,
    /// State of the bottom widget.
    pub bottom: B,
    /// Arranged bounds of the top widget.
    pub top_bounds: Rect,
    /// Arranged bounds of the bottom widget.
    pub bottom_bounds: Rect,
}

impl<A: Default, B: Default> VStackState<A, B> {
    /// Creates default child states.
    pub fn new() -> Self {
        Self {
            top: A::default(),
            bottom: B::default(),
            top_bounds: Rect::EMPTY,
            bottom_bounds: Rect::EMPTY,
        }
    }
}

impl<A: Widget, B: Widget> Widget for VStack<A, B> {
    type State = VStackState<A::State, B::State>;

    fn measure(&self, cx: &MeasureCx, state: &Self::State) -> Size {
        let top = self.top.measure(cx, &state.top);
        let bottom = self.bottom.measure(cx, &state.bottom);
        Size::new(
            top.w.max(bottom.w),
            top.h.saturating_add(self.gap).saturating_add(bottom.h),
        )
    }

    fn arrange(&self, bounds: Rect, state: &mut Self::State) {
        let cx = MeasureCx::new(Size::new(bounds.w, bounds.h));
        let mut top = self.top.measure(&cx, &state.top);
        let mut bottom = self.bottom.measure(&cx, &state.bottom);

        let gap = self.gap;
        let available = bounds.h.saturating_sub(gap);
        let flexible_top = self.top.flexible();
        let flexible_bottom = self.bottom.flexible();

        match (flexible_top, flexible_bottom) {
            (true, false) => {
                top.h = available.saturating_sub(bottom.h);
            }
            (false, true) => {
                bottom.h = available.saturating_sub(top.h);
            }
            (true, true) => {
                let half = available / 2;
                top.h = half;
                bottom.h = available - half;
            }
            (false, false) => {}
        }

        let total = top.h.saturating_add(gap).saturating_add(bottom.h);
        let leftover = bounds.h.saturating_sub(total);
        let offset = match self.main {
            MainAlign::Start => 0,
            MainAlign::Center => leftover / 2,
            MainAlign::End => leftover,
        };

        let top_rect = Rect::new(
            bounds.x,
            bounds.y.saturating_add(offset as i16),
            top.w,
            top.h,
        );
        let top_rect = self.apply_cross(bounds, top_rect);
        state.top_bounds = top_rect;

        let bottom_y = top_rect
            .y
            .saturating_add(top_rect.h as i16)
            .saturating_add(gap as i16);
        let bottom_rect = Rect::new(bounds.x, bottom_y, bottom.w, bottom.h);
        let bottom_rect = self.apply_cross(bounds, bottom_rect);
        state.bottom_bounds = bottom_rect;

        self.top.arrange(top_rect, &mut state.top);
        self.bottom.arrange(bottom_rect, &mut state.bottom);
    }

    fn event(&self, event: &Event, state: &mut Self::State, out: &mut EventOut, now_us: u64) {
        if out.consumed {
            return;
        }
        self.top.event(event, &mut state.top, out, now_us);
        if out.consumed {
            return;
        }
        self.bottom.event(event, &mut state.bottom, out, now_us);
    }

    fn paint(&self, cx: &mut PaintCx<'_>, state: &Self::State, now_us: u64) {
        self.top.paint(cx, &state.top, now_us);
        self.bottom.paint(cx, &state.bottom, now_us);
    }
}

impl<A, B> VStack<A, B> {
    fn apply_cross(&self, bounds: Rect, rect: Rect) -> Rect {
        match self.cross {
            CrossAlign::Stretch => Rect::new(bounds.x, rect.y, bounds.w, rect.h),
            CrossAlign::Start => rect,
            CrossAlign::Center => Rect::new(
                bounds.x + (bounds.w.saturating_sub(rect.w) / 2) as i16,
                rect.y,
                rect.w,
                rect.h,
            ),
        }
    }
}

/// Two widgets laid out horizontally.
#[derive(Clone, Copy, Debug)]
pub struct HStack<A, B> {
    /// Left widget.
    pub left: A,
    /// Right widget.
    pub right: B,
    /// Gap between them in pixels.
    pub gap: u16,
    /// Main-axis alignment.
    pub main: MainAlign,
    /// Cross-axis alignment.
    pub cross: CrossAlign,
}

impl<A, B> HStack<A, B> {
    /// Creates a horizontal stack with no gap.
    pub const fn new(left: A, right: B) -> Self {
        Self {
            left,
            right,
            gap: 0,
            main: MainAlign::Start,
            cross: CrossAlign::Stretch,
        }
    }

    /// Sets the gap.
    pub const fn gap(mut self, gap: u16) -> Self {
        self.gap = gap;
        self
    }
}

/// Retained state of an [`HStack`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HStackState<A, B> {
    /// State of the left widget.
    pub left: A,
    /// State of the right widget.
    pub right: B,
    /// Arranged bounds of the left widget.
    pub left_bounds: Rect,
    /// Arranged bounds of the right widget.
    pub right_bounds: Rect,
}

impl<A: Default, B: Default> HStackState<A, B> {
    /// Creates default child states.
    pub fn new() -> Self {
        Self {
            left: A::default(),
            right: B::default(),
            left_bounds: Rect::EMPTY,
            right_bounds: Rect::EMPTY,
        }
    }
}

impl<A: Widget, B: Widget> Widget for HStack<A, B> {
    type State = HStackState<A::State, B::State>;

    fn measure(&self, cx: &MeasureCx, state: &Self::State) -> Size {
        let left = self.left.measure(cx, &state.left);
        let right = self.right.measure(cx, &state.right);
        Size::new(
            left.w.saturating_add(self.gap).saturating_add(right.w),
            left.h.max(right.h),
        )
    }

    fn arrange(&self, bounds: Rect, state: &mut Self::State) {
        let cx = MeasureCx::new(Size::new(bounds.w, bounds.h));
        let mut left = self.left.measure(&cx, &state.left);
        let mut right = self.right.measure(&cx, &state.right);

        let available = bounds.w.saturating_sub(self.gap);
        match (self.left.flexible(), self.right.flexible()) {
            (true, false) => left.w = available.saturating_sub(right.w),
            (false, true) => right.w = available.saturating_sub(left.w),
            (true, true) => {
                let half = available / 2;
                left.w = half;
                right.w = available - half;
            }
            (false, false) => {}
        }

        let total = left.w.saturating_add(self.gap).saturating_add(right.w);
        let leftover = bounds.w.saturating_sub(total);
        let offset = match self.main {
            MainAlign::Start => 0,
            MainAlign::Center => leftover / 2,
            MainAlign::End => leftover,
        };

        let left_rect = self.cross(
            bounds,
            Rect::new(
                bounds.x.saturating_add(offset as i16),
                bounds.y,
                left.w,
                left.h,
            ),
        );
        let right_rect = self.cross(
            bounds,
            Rect::new(
                left_rect
                    .x
                    .saturating_add(left_rect.w as i16)
                    .saturating_add(self.gap as i16),
                bounds.y,
                right.w,
                right.h,
            ),
        );
        state.left_bounds = left_rect;
        state.right_bounds = right_rect;
        self.left.arrange(left_rect, &mut state.left);
        self.right.arrange(right_rect, &mut state.right);
    }

    fn event(&self, event: &Event, state: &mut Self::State, out: &mut EventOut, now_us: u64) {
        if out.consumed {
            return;
        }
        self.left.event(event, &mut state.left, out, now_us);
        if out.consumed {
            return;
        }
        self.right.event(event, &mut state.right, out, now_us);
    }

    fn paint(&self, cx: &mut PaintCx<'_>, state: &Self::State, now_us: u64) {
        self.left.paint(cx, &state.left, now_us);
        self.right.paint(cx, &state.right, now_us);
    }
}

impl<A, B> HStack<A, B> {
    fn cross(&self, bounds: Rect, rect: Rect) -> Rect {
        match self.cross {
            CrossAlign::Stretch => Rect::new(rect.x, bounds.y, rect.w, bounds.h),
            CrossAlign::Start => rect,
            CrossAlign::Center => Rect::new(
                rect.x,
                bounds.y + (bounds.h.saturating_sub(rect.h) / 2) as i16,
                rect.w,
                rect.h,
            ),
        }
    }
}

/// Pixel insets on the four edges.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Edges {
    /// Left inset.
    pub left: u16,
    /// Right inset.
    pub right: u16,
    /// Top inset.
    pub top: u16,
    /// Bottom inset.
    pub bottom: u16,
}

impl Edges {
    /// Equal insets on all edges.
    pub const fn all(n: u16) -> Self {
        Self {
            left: n,
            right: n,
            top: n,
            bottom: n,
        }
    }

    /// Horizontal and vertical insets.
    pub const fn symmetric(h: u16, v: u16) -> Self {
        Self {
            left: h,
            right: h,
            top: v,
            bottom: v,
        }
    }
}

/// Adds insets around a widget.
#[derive(Clone, Copy, Debug)]
pub struct Padding<W> {
    /// The wrapped widget.
    pub inner: W,
    /// Insets.
    pub edges: Edges,
}

impl<W> Padding<W> {
    /// Wraps `inner` with `edges`.
    pub const fn new(inner: W, edges: Edges) -> Self {
        Self { inner, edges }
    }
}

/// Retained state of a [`Padding`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PaddingState<W> {
    /// State of the inner widget.
    pub inner: W,
    /// Arranged inner bounds.
    pub inner_bounds: Rect,
}

impl<W: Default> PaddingState<W> {
    /// Creates a default child state.
    pub fn new() -> Self {
        Self {
            inner: W::default(),
            inner_bounds: Rect::EMPTY,
        }
    }
}

impl<W: Widget> Widget for Padding<W> {
    type State = PaddingState<W::State>;

    fn measure(&self, cx: &MeasureCx, state: &Self::State) -> Size {
        let inner = self.inner.measure(cx, &state.inner);
        Size::new(
            inner
                .w
                .saturating_add(self.edges.left)
                .saturating_add(self.edges.right),
            inner
                .h
                .saturating_add(self.edges.top)
                .saturating_add(self.edges.bottom),
        )
    }

    fn arrange(&self, bounds: Rect, state: &mut Self::State) {
        let inner = Rect::new(
            bounds.x.saturating_add(self.edges.left as i16),
            bounds.y.saturating_add(self.edges.top as i16),
            bounds
                .w
                .saturating_sub(self.edges.left)
                .saturating_sub(self.edges.right),
            bounds
                .h
                .saturating_sub(self.edges.top)
                .saturating_sub(self.edges.bottom),
        );
        state.inner_bounds = inner;
        self.inner.arrange(inner, &mut state.inner);
    }

    fn event(&self, event: &Event, state: &mut Self::State, out: &mut EventOut, now_us: u64) {
        self.inner.event(event, &mut state.inner, out, now_us);
    }

    fn paint(&self, cx: &mut PaintCx<'_>, state: &Self::State, now_us: u64) {
        self.inner.paint(cx, &state.inner, now_us);
    }

    fn flexible(&self) -> bool {
        self.inner.flexible()
    }
}

/// Fills the background of the wrapped widget.
#[derive(Clone, Copy, Debug)]
pub struct Background<W> {
    /// The wrapped widget.
    pub inner: W,
    /// Fill colour.
    pub color: Rgb565,
}

impl<W> Background<W> {
    /// Wraps `inner` with a background fill.
    pub const fn new(inner: W, color: Rgb565) -> Self {
        Self { inner, color }
    }
}

/// Retained state of a [`Background`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BackgroundState<W> {
    /// State of the inner widget.
    pub inner: W,
    /// Arranged bounds of the background.
    pub bounds: Rect,
}

impl<W: Default> BackgroundState<W> {
    /// Creates a default child state.
    pub fn new() -> Self {
        Self {
            inner: W::default(),
            bounds: Rect::EMPTY,
        }
    }
}

impl<W: Widget> Widget for Background<W> {
    type State = BackgroundState<W::State>;

    fn measure(&self, cx: &MeasureCx, state: &Self::State) -> Size {
        self.inner.measure(cx, &state.inner)
    }

    fn arrange(&self, bounds: Rect, state: &mut Self::State) {
        state.bounds = bounds;
        self.inner.arrange(bounds, &mut state.inner);
    }

    fn event(&self, event: &Event, state: &mut Self::State, out: &mut EventOut, now_us: u64) {
        self.inner.event(event, &mut state.inner, out, now_us);
    }

    fn paint(&self, cx: &mut PaintCx<'_>, state: &Self::State, now_us: u64) {
        cx.fill_rect(state.bounds, self.color);
        self.inner.paint(cx, &state.inner, now_us);
    }

    fn flexible(&self) -> bool {
        self.inner.flexible()
    }
}

/// A leaf with a fixed size and no content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fixed {
    /// The fixed size.
    pub size: Size,
}

impl Fixed {
    /// Creates a fixed-size leaf.
    pub const fn new(w: u16, h: u16) -> Self {
        Self {
            size: Size::new(w, h),
        }
    }
}

impl Widget for Fixed {
    type State = ();

    fn measure(&self, _cx: &MeasureCx, _state: &Self::State) -> Size {
        self.size
    }

    fn arrange(&self, _bounds: Rect, _state: &mut Self::State) {}

    fn paint(&self, _cx: &mut PaintCx<'_>, _state: &Self::State, _now_us: u64) {}
}

/// A flexible spacer filling the leftover main-axis space.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Expanded<W> {
    /// The wrapped widget.
    pub inner: W,
}

impl<W> Expanded<W> {
    /// Marks `inner` as flexible.
    pub const fn new(inner: W) -> Self {
        Self { inner }
    }
}

impl<W: Widget> Widget for Expanded<W> {
    type State = W::State;

    fn measure(&self, cx: &MeasureCx, state: &Self::State) -> Size {
        self.inner.measure(cx, state)
    }

    fn arrange(&self, bounds: Rect, state: &mut Self::State) {
        self.inner.arrange(bounds, state);
    }

    fn event(&self, event: &Event, state: &mut Self::State, out: &mut EventOut, now_us: u64) {
        self.inner.event(event, state, out, now_us);
    }

    fn paint(&self, cx: &mut PaintCx<'_>, state: &Self::State, now_us: u64) {
        self.inner.paint(cx, state, now_us);
    }

    fn flexible(&self) -> bool {
        true
    }
}

/// Convenience: a screen with a fixed row height and a gap.
pub fn rows(count: u8, row_h: u16, gap: u16) -> Size {
    Size::new(
        0,
        (count as u16)
            .saturating_mul(row_h)
            .saturating_add((count.saturating_sub(1) as u16).saturating_mul(gap)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedWidget(Size);
    impl Widget for FixedWidget {
        type State = ();
        fn measure(&self, _cx: &MeasureCx, _s: &()) -> Size {
            self.0
        }
        fn arrange(&self, _b: Rect, _s: &mut ()) {}
        fn paint(&self, _cx: &mut PaintCx<'_>, _s: &(), _n: u64) {}
    }

    #[test]
    fn vstack_measure_sums_heights() {
        let v = VStack::new(
            FixedWidget(Size::new(100, 10)),
            FixedWidget(Size::new(80, 20)),
        )
        .gap(4);
        let state = VStackState::<(), ()>::new();
        assert_eq!(
            v.measure(&MeasureCx::new(Size::new(200, 200)), &state),
            Size::new(100, 34)
        );
    }

    #[test]
    fn vstack_arrange_stacks_and_stretches() {
        let v = VStack::new(
            FixedWidget(Size::new(100, 10)),
            FixedWidget(Size::new(80, 20)),
        );
        let mut state = VStackState::<(), ()>::new();
        v.arrange(Rect::new(5, 7, 120, 60), &mut state);
        assert_eq!(state.top_bounds, Rect::new(5, 7, 120, 10));
        assert_eq!(state.bottom_bounds, Rect::new(5, 17, 120, 20));
    }

    #[test]
    fn vstack_center_alignment() {
        let v = VStack::new(
            FixedWidget(Size::new(10, 10)),
            FixedWidget(Size::new(10, 10)),
        )
        .gap(0)
        .main(MainAlign::Center);
        let mut state = VStackState::<(), ()>::new();
        v.arrange(Rect::new(0, 0, 100, 50), &mut state);
        assert_eq!(state.top_bounds.y, 15);
        assert_eq!(state.bottom_bounds.y, 25);
    }

    #[test]
    fn expanded_takes_remaining_space() {
        let v = VStack::new(
            FixedWidget(Size::new(100, 10)),
            Expanded::new(FixedWidget(Size::new(100, 0))),
        );
        let mut state = VStackState::<(), ()>::new();
        v.arrange(Rect::new(0, 0, 100, 100), &mut state);
        assert_eq!(state.top_bounds.h, 10);
        assert_eq!(state.bottom_bounds.h, 90);
    }

    #[test]
    fn padding_insets_inner() {
        let p = Padding::new(FixedWidget(Size::new(10, 10)), Edges::symmetric(4, 2));
        let mut state = PaddingState::<()>::new();
        p.arrange(Rect::new(10, 20, 100, 40), &mut state);
        assert_eq!(state.inner_bounds, Rect::new(14, 22, 92, 36));
        assert_eq!(
            p.measure(
                &MeasureCx::new(Size::new(200, 200)),
                &PaddingState::<()>::new()
            ),
            Size::new(18, 14)
        );
    }

    #[test]
    fn hstack_lays_out_left_to_right() {
        let h = HStack::new(
            FixedWidget(Size::new(20, 10)),
            FixedWidget(Size::new(30, 10)),
        )
        .gap(5);
        let mut state = HStackState::<(), ()>::new();
        h.arrange(Rect::new(0, 0, 200, 50), &mut state);
        assert_eq!(state.left_bounds, Rect::new(0, 0, 20, 50));
        assert_eq!(state.right_bounds, Rect::new(25, 0, 30, 50));
    }

    #[test]
    fn rows_helper() {
        assert_eq!(rows(3, 28, 4), Size::new(0, 92));
        assert_eq!(rows(0, 28, 4), Size::new(0, 0));
    }
}
