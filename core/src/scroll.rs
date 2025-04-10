use std::f64::INFINITY;
use std::time::Duration;

use druid::{
    kurbo::{Affine, Point, Rect, Size, Vec2},
    Insets, WidgetId,
};
use druid::{
    theme, BoxConstraints, Data, Env, Event, EventCtx, LayoutCtx, LifeCycle,
    LifeCycleCtx, PaintCtx, RenderContext, TimerToken, UpdateCtx, Widget, WidgetPod,
};

use crate::command::{LapceUICommand, LAPCE_UI_COMMAND};

/// Represents the size and position of a rectangular "viewport" into a larger area.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Viewport {
    /// The size of the area that we have a viewport into.
    pub content_size: Size,
    /// The view rectangle.
    pub rect: Rect,
}

impl Viewport {
    /// Tries to find a position for the view rectangle that is contained in the content rectangle.
    ///
    /// If the supplied origin is good, returns it; if it isn't, we try to return the nearest
    /// origin that would make the view rectangle contained in the content rectangle. (This will
    /// fail if the content is smaller than the view, and we return `0.0` in each dimension where
    /// the content is smaller.)
    pub fn clamp_view_origin(&self, origin: Point) -> Point {
        let x = origin
            .x
            .min(self.content_size.width - self.rect.width())
            .max(0.0);
        let y = origin
            .y
            .min(self.content_size.height - self.rect.height())
            .max(0.0);
        Point::new(x, y)
    }

    /// Changes the viewport offset by `delta`, while trying to keep the view rectangle inside the
    /// content rectangle.
    ///
    /// Returns true if the offset actually changed. Even if `delta` is non-zero, the offset might
    /// not change. For example, if you try to move the viewport down but it is already at the
    /// bottom of the child widget, then the offset will not change and this function will return
    /// false.
    pub fn pan_by(&mut self, delta: Vec2) -> bool {
        self.pan_to(self.rect.origin() + delta)
    }

    /// Sets the viewport origin to `pos`, while trying to keep the view rectangle inside the
    /// content rectangle.
    ///
    /// Returns true if the position changed. Note that the valid values for the viewport origin
    /// are constrained by the size of the child, and so the origin might not get set to exactly
    /// `pos`.
    pub fn pan_to(&mut self, origin: Point) -> bool {
        let new_origin = self.clamp_view_origin(origin);
        if (new_origin - self.rect.origin()).hypot2() > 1e-12 {
            self.rect = self.rect.with_origin(new_origin);
            true
        } else {
            false
        }
    }

    pub fn force_pan_to(&mut self, origin: Point) -> bool {
        if (origin - self.rect.origin()).hypot2() > 1e-12 {
            self.rect = self.rect.with_origin(origin);
            true
        } else {
            false
        }
    }
}

/// A widget exposing a rectangular view into its child, which can be used as a building block for
/// widgets that scroll their child.
pub struct ClipBox<T, W> {
    child: WidgetPod<T, W>,
    port: Viewport,
    constrain_horizontal: bool,
    constrain_vertical: bool,
}

impl<T, W: Widget<T>> ClipBox<T, W> {
    /// Creates a new `ClipBox` wrapping `child`.
    pub fn new(child: W) -> Self {
        ClipBox {
            child: WidgetPod::new(child),
            port: Default::default(),
            constrain_horizontal: false,
            constrain_vertical: false,
        }
    }

    /// Returns a reference to the child widget.
    pub fn child(&self) -> &W {
        self.child.widget()
    }

    /// Returns a mutable reference to the child widget.
    pub fn child_mut(&mut self) -> &mut W {
        self.child.widget_mut()
    }

    /// Returns a the viewport describing this `ClipBox`'s position.
    pub fn viewport(&self) -> Viewport {
        self.port
    }

    /// Returns the size of the rectangular viewport into the child widget.
    /// To get the position of the viewport, see [`viewport_origin`].
    ///
    /// [`viewport_origin`]: struct.ClipBox.html#method.viewport_origin
    pub fn viewport_size(&self) -> Size {
        self.port.rect.size()
    }

    /// Returns the size of the child widget.
    pub fn content_size(&self) -> Size {
        self.port.content_size
    }

    /// Builder-style method for deciding whether to constrain the child horizontally. The default
    /// is `false`. See [`constrain_vertical`] for more details.
    ///
    /// [`constrain_vertical`]: struct.ClipBox.html#constrain_vertical
    pub fn constrain_horizontal(mut self, constrain: bool) -> Self {
        self.constrain_horizontal = constrain;
        self
    }

    /// Determine whether to constrain the child horizontally.
    ///
    /// See [`constrain_vertical`] for more details.
    ///
    /// [`constrain_vertical`]: struct.ClipBox.html#constrain_vertical
    pub fn set_constrain_horizontal(&mut self, constrain: bool) {
        self.constrain_horizontal = constrain;
    }

    /// Builder-style method for deciding whether to constrain the child vertically. The default
    /// is `false`.
    ///
    /// This setting affects how a `ClipBox` lays out its child.
    ///
    /// - When it is `false` (the default), the child does receive any upper bound on its height:
    ///   the idea is that the child can be as tall as it wants, and the viewport will somehow get
    ///   moved around to see all of it.
    /// - When it is `true`, the viewport's maximum height will be passed down as an upper bound on
    ///   the height of the child, and the viewport will set its own height to be the same as its
    ///   child's height.
    pub fn constrain_vertical(mut self, constrain: bool) -> Self {
        self.constrain_vertical = constrain;
        self
    }

    /// Determine whether to constrain the child vertically.
    ///
    /// See [`constrain_vertical`] for more details.
    ///
    /// [`constrain_vertical`]: struct.ClipBox.html#constrain_vertical
    pub fn set_constrain_vertical(&mut self, constrain: bool) {
        self.constrain_vertical = constrain;
    }

    /// Changes the viewport offset by `delta`.
    ///
    /// Returns true if the offset actually changed. Even if `delta` is non-zero, the offset might
    /// not change. For example, if you try to move the viewport down but it is already at the
    /// bottom of the child widget, then the offset will not change and this function will return
    /// false.
    pub fn pan_by(&mut self, delta: Vec2) -> bool {
        self.pan_to(self.viewport_origin() + delta)
    }

    /// Sets the viewport origin to `pos`.
    ///
    /// Returns true if the position changed. Note that the valid values for the viewport origin
    /// are constrained by the size of the child, and so the origin might not get set to exactly
    /// `pos`.
    pub fn pan_to(&mut self, origin: Point) -> bool {
        if self.port.pan_to(origin) {
            self.child
                .set_viewport_offset(self.viewport_origin().to_vec2());
            true
        } else {
            false
        }
    }

    pub fn force_pan_to(&mut self, origin: Point) -> bool {
        if self.port.force_pan_to(origin) {
            self.child
                .set_viewport_offset(self.viewport_origin().to_vec2());
            true
        } else {
            false
        }
    }

    /// Returns the origin of the viewport rectangle.
    pub fn viewport_origin(&self) -> Point {
        self.port.rect.origin()
    }

    /// Allows this `ClipBox`'s viewport rectangle to be modified. The provided callback function
    /// can modify its argument, and when it is done then this `ClipBox` will be modified to have
    /// the new viewport rectangle.
    pub fn with_port<F: FnOnce(&mut Viewport)>(&mut self, f: F) {
        f(&mut self.port);
        self.child
            .set_viewport_offset(self.viewport_origin().to_vec2());
    }
}

impl<T: Data, W: Widget<T>> Widget<T> for ClipBox<T, W> {
    fn event(&mut self, ctx: &mut EventCtx, ev: &Event, data: &mut T, env: &Env) {
        let viewport = ctx.size().to_rect();
        let force_event = self.child.is_hot() || self.child.is_active();
        if let Some(child_event) = ev.transform_scroll(
            self.viewport_origin().to_vec2(),
            viewport,
            force_event,
        ) {
            self.child.event(ctx, &child_event, data, env);
        }
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        ev: &LifeCycle,
        data: &T,
        env: &Env,
    ) {
        self.child.lifecycle(ctx, ev, data, env);
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &T, data: &T, env: &Env) {
        self.child.update(ctx, data, env);
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &T,
        env: &Env,
    ) -> Size {
        bc.debug_check("ClipBox");

        let max_child_width = if self.constrain_horizontal {
            bc.max().width
        } else {
            f64::INFINITY
        };
        let max_child_height = if self.constrain_vertical {
            bc.max().height
        } else {
            f64::INFINITY
        };
        let child_bc = BoxConstraints::new(
            bc.max(),
            Size::new(max_child_width, max_child_height),
        );

        let content_size = self.child.layout(ctx, &child_bc, data, env);
        self.port.content_size = content_size;
        self.child
            .set_layout_rect(ctx, data, env, content_size.to_rect());

        self.port.rect = self.port.rect.with_size(bc.constrain(content_size));
        let new_offset = self.port.clamp_view_origin(self.viewport_origin());
        self.pan_to(new_offset);
        self.viewport_size()
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        let viewport = ctx.size().to_rect();
        let offset = self.viewport_origin().to_vec2();
        ctx.with_save(|ctx| {
            ctx.clip(viewport);
            ctx.transform(Affine::translate(-offset));

            let mut visible = ctx.region().clone();
            visible += offset;
            ctx.with_child_ctx(visible, |ctx| self.child.paint_raw(ctx, data, env));
        });
    }
}

#[derive(Debug, Clone)]
enum ScrollDirection {
    Bidirectional,
    Vertical,
    Horizontal,
}

/// A container that scrolls its contents.
///
/// This container holds a single child, and uses the wheel to scroll it
/// when the child's bounds are larger than the viewport.
///
/// The child is laid out with completely unconstrained layout bounds by
/// default. Restrict to a specific axis with [`vertical`] or [`horizontal`].
/// When restricted to scrolling on a specific axis the child's size is
/// locked on the opposite axis.
///
/// [`vertical`]: struct.Scroll.html#method.vertical
/// [`horizontal`]: struct.Scroll.html#method.horizontal
pub struct LapceScroll<T, W> {
    clip: ClipBox<T, W>,
    scroll_component: ScrollComponent,
}

impl<T: Data, W: Widget<T>> LapceScroll<T, W> {
    /// Create a new scroll container.
    ///
    /// This method will allow scrolling in all directions if child's bounds
    /// are larger than the viewport. Use [vertical](#method.vertical) and
    /// [horizontal](#method.horizontal) methods to limit scrolling to a specific axis.
    pub fn new(child: W) -> LapceScroll<T, W> {
        LapceScroll {
            clip: ClipBox::new(child),
            scroll_component: ScrollComponent::new(),
            //direction: ScrollDirection::Bidirectional,
            //content_size: Size::ZERO,
            //scroll_offset: Vec2::ZERO,
        }
    }

    /// Restrict scrolling to the vertical axis while locking child width.
    pub fn vertical(mut self) -> Self {
        self.clip.set_constrain_vertical(false);
        self.clip.set_constrain_horizontal(true);
        self
    }

    /// Restrict scrolling to the horizontal axis while locking child height.
    pub fn horizontal(mut self) -> Self {
        self.clip.set_constrain_vertical(true);
        self.clip.set_constrain_horizontal(false);
        self
    }

    /// Returns a reference to the child widget.
    pub fn child(&self) -> &W {
        self.clip.child()
    }

    /// Returns a mutable reference to the child widget.
    pub fn child_mut(&mut self) -> &mut W {
        self.clip.child_mut()
    }

    /// Returns the size of the child widget.
    pub fn child_size(&self) -> Size {
        self.clip.content_size()
    }

    /// Returns the current scroll offset.
    pub fn offset(&self) -> Vec2 {
        self.clip.viewport_origin().to_vec2()
    }

    pub fn scroll(&mut self, x: f64, y: f64) {
        self.clip.pan_by(Vec2::new(x, y));
    }

    pub fn scroll_to(&mut self, x: f64, y: f64) {
        self.clip.pan_to(Point::new(x, y));
    }

    pub fn force_scroll_to(&mut self, x: f64, y: f64) {
        self.clip.force_pan_to(Point::new(x, y));
    }

    pub fn ensure_visible(
        &mut self,
        scroll_size: Size,
        rect: &Rect,
        margin: &(f64, f64),
    ) -> bool {
        let mut new_offset = self.offset();
        let content_size = self.child_size();

        let (x_margin, y_margin) = margin;

        new_offset.x = if new_offset.x < rect.x1 + x_margin - scroll_size.width {
            (rect.x1 + x_margin - scroll_size.width)
                .min(content_size.width - scroll_size.width)
        } else if new_offset.x > rect.x0 - x_margin {
            (rect.x0 - x_margin).max(0.0)
        } else {
            new_offset.x
        };

        new_offset.y = if new_offset.y < rect.y1 + y_margin - scroll_size.height {
            (rect.y1 + y_margin - scroll_size.height)
                .min(content_size.height - scroll_size.height)
        } else if new_offset.y > rect.y0 - y_margin {
            (rect.y0 - y_margin).max(0.0)
        } else {
            new_offset.y
        };

        if new_offset == self.offset() {
            return false;
        }

        self.clip.pan_to(Point::new(new_offset.x, new_offset.y));
        true
    }
}

impl<T: Data, W: Widget<T>> Widget<T> for LapceScroll<T, W> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        match event {
            Event::Internal(_) => {
                self.clip.event(ctx, event, data, env);
            }
            Event::Command(cmd) => match cmd {
                _ if cmd.is(LAPCE_UI_COMMAND) => {
                    let command = cmd.get_unchecked(LAPCE_UI_COMMAND);
                    match command {
                        LapceUICommand::RequestLayout => {
                            println!("scroll request layout");
                            ctx.request_layout();
                        }
                        LapceUICommand::RequestPaint => {
                            println!("scroll request paint");
                            ctx.request_paint();
                        }
                        LapceUICommand::EnsureVisible((rect, margin, position)) => {
                            if self.ensure_visible(ctx.size(), rect, margin) {
                                ctx.request_paint();
                            }
                            return;
                        }
                        LapceUICommand::ScrollTo((x, y)) => {
                            self.scroll_to(*x, *y);
                            return;
                        }
                        LapceUICommand::Scroll((x, y)) => {
                            self.scroll(*x, *y);
                            ctx.request_paint();
                            return;
                        }
                        _ => println!("scroll unprocessed ui command {:?}", command),
                    }
                }
                _ => (),
            },
            _ => (),
        };
        // self.scroll_component.event(ctx, event, env);
        if !ctx.is_handled() {
            self.clip.event(ctx, event, data, env);
        }

        // self.scroll_component.handle_scroll(
        //     self.child.viewport_offset(),
        //     ctx,
        //     event,
        //     env,
        // );
        // In order to ensure that invalidation regions are correctly propagated up the tree,
        // we need to set the viewport offset on our child whenever we change our scroll offset.
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &T,
        env: &Env,
    ) {
        self.clip.lifecycle(ctx, event, data, env);
        self.scroll_component.lifecycle(ctx, event, env);
    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &T, data: &T, env: &Env) {
        self.clip.update(ctx, old_data, data, env);
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &T,
        env: &Env,
    ) -> Size {
        bc.debug_check("Scroll");

        let old_size = self.clip.viewport().rect.size();
        let child_size = self.clip.layout(ctx, &bc, data, env);

        let self_size = bc.constrain(child_size);
        self_size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        self.clip.paint(ctx, data, env);
        self.scroll_component
            .draw_bars(ctx, &self.clip.viewport(), env);
    }
}

/// Minimum length for any scrollbar to be when measured on that
/// scrollbar's primary axis.
pub const SCROLLBAR_MIN_SIZE: f64 = 45.0;

/// Denotes which scrollbar, if any, is currently being hovered over
/// by the mouse.
#[derive(Debug, Copy, Clone)]
pub enum BarHoveredState {
    /// Neither scrollbar is being hovered by the mouse.
    None,
    /// The vertical scrollbar is being hovered by the mouse.
    Vertical,
    /// The horizontal scrollbar is being hovered by the mouse.
    Horizontal,
}

impl BarHoveredState {
    /// Determines if any scrollbar is currently being hovered by the mouse.
    pub fn is_hovered(self) -> bool {
        matches!(
            self,
            BarHoveredState::Vertical | BarHoveredState::Horizontal
        )
    }
}

/// Denotes which scrollbar, if any, is currently being dragged.
#[derive(Debug, Copy, Clone)]
pub enum BarHeldState {
    /// Neither scrollbar is being dragged.
    None,
    /// Vertical scrollbar is being dragged. Contains an `f64` with
    /// the initial y-offset of the dragging input.
    Vertical(f64),
    /// Horizontal scrollbar is being dragged. Contains an `f64` with
    /// the initial x-offset of the dragging input.
    Horizontal(f64),
}

#[derive(Debug, Copy, Clone)]
pub struct ScrollComponent {
    /// Current opacity for both scrollbars
    pub opacity: f64,
    /// ID for the timer which schedules scrollbar fade out
    pub timer_id: TimerToken,
    /// Which if any scrollbar is currently hovered by the mouse
    pub hovered: BarHoveredState,
    /// Which if any scrollbar is currently being dragged by the mouse
    pub held: BarHeldState,
}

impl Default for ScrollComponent {
    fn default() -> Self {
        Self {
            opacity: 0.0,
            timer_id: TimerToken::INVALID,
            hovered: BarHoveredState::None,
            held: BarHeldState::None,
        }
    }
}

impl ScrollComponent {
    /// Constructs a new [`ScrollComponent`](struct.ScrollComponent.html) for use.
    pub fn new() -> ScrollComponent {
        Default::default()
    }

    /// true if either scrollbar is currently held down/being dragged
    pub fn are_bars_held(&self) -> bool {
        !matches!(self.held, BarHeldState::None)
    }

    /// Makes the scrollbars visible, and resets the fade timer.
    pub fn reset_scrollbar_fade<F>(&mut self, request_timer: F, env: &Env)
    where
        F: FnOnce(Duration) -> TimerToken,
    {
        self.opacity = env.get(theme::SCROLLBAR_MAX_OPACITY);
        let fade_delay = env.get(theme::SCROLLBAR_FADE_DELAY);
        let deadline = Duration::from_millis(fade_delay);
        self.timer_id = request_timer(deadline);
    }

    /// Calculates the paint rect of the vertical scrollbar, or `None` if the vertical scrollbar is
    /// not visible.
    pub fn calc_vertical_bar_bounds(
        &self,
        port: &Viewport,
        env: &Env,
    ) -> Option<Rect> {
        let viewport_size = port.rect.size();
        let content_size = port.content_size;
        let scroll_offset = port.rect.origin().to_vec2();

        if viewport_size.height >= content_size.height {
            return None;
        }

        let bar_width = env.get(theme::SCROLLBAR_WIDTH);
        let bar_pad = env.get(theme::SCROLLBAR_PAD);

        let percent_visible = viewport_size.height / content_size.height;
        let percent_scrolled =
            scroll_offset.y / (content_size.height - viewport_size.height);

        let length = (percent_visible * viewport_size.height).ceil();
        let length = length.max(SCROLLBAR_MIN_SIZE);

        let vertical_padding = bar_pad + bar_pad + bar_width;

        let top_y_offset = ((viewport_size.height - length - vertical_padding)
            * percent_scrolled)
            .ceil();
        let bottom_y_offset = top_y_offset + length;

        let x0 = scroll_offset.x + viewport_size.width - bar_width - bar_pad;
        let y0 = scroll_offset.y + top_y_offset + bar_pad;

        let x1 = scroll_offset.x + viewport_size.width - bar_pad;
        let y1 = scroll_offset.y + bottom_y_offset;

        Some(Rect::new(x0, y0, x1, y1))
    }

    /// Calculates the paint rect of the horizontal scrollbar, or `None` if the horizontal
    /// scrollbar is not visible.
    pub fn calc_horizontal_bar_bounds(
        &self,
        port: &Viewport,
        env: &Env,
    ) -> Option<Rect> {
        let viewport_size = port.rect.size();
        let content_size = port.content_size;
        let scroll_offset = port.rect.origin().to_vec2();

        if viewport_size.width >= content_size.width {
            return None;
        }

        let bar_width = env.get(theme::SCROLLBAR_WIDTH);
        let bar_pad = env.get(theme::SCROLLBAR_PAD);

        let percent_visible = viewport_size.width / content_size.width;
        let percent_scrolled =
            scroll_offset.x / (content_size.width - viewport_size.width);

        let length = (percent_visible * viewport_size.width).ceil();
        let length = length.max(SCROLLBAR_MIN_SIZE);

        let horizontal_padding = bar_pad + bar_pad + bar_width;

        let left_x_offset = ((viewport_size.width - length - horizontal_padding)
            * percent_scrolled)
            .ceil();
        let right_x_offset = left_x_offset + length;

        let x0 = scroll_offset.x + left_x_offset + bar_pad;
        let y0 = scroll_offset.y + viewport_size.height - bar_width - bar_pad;

        let x1 = scroll_offset.x + right_x_offset;
        let y1 = scroll_offset.y + viewport_size.height - bar_pad;

        Some(Rect::new(x0, y0, x1, y1))
    }

    /// Draw scroll bars.
    pub fn draw_bars(&self, ctx: &mut PaintCtx, port: &Viewport, env: &Env) {
        let scroll_offset = port.rect.origin().to_vec2();
        if self.opacity <= 0.0 {
            return;
        }

        let brush = ctx
            .render_ctx
            .solid_brush(env.get(theme::SCROLLBAR_COLOR).with_alpha(self.opacity));
        let border_brush = ctx.render_ctx.solid_brush(
            env.get(theme::SCROLLBAR_BORDER_COLOR)
                .with_alpha(self.opacity),
        );

        let radius = env.get(theme::SCROLLBAR_RADIUS);
        let edge_width = env.get(theme::SCROLLBAR_EDGE_WIDTH);

        // Vertical bar
        if let Some(bounds) = self.calc_vertical_bar_bounds(port, env) {
            let rect = (bounds - scroll_offset)
                .inset(-edge_width / 2.0)
                .to_rounded_rect(radius);
            ctx.render_ctx.fill(rect, &brush);
            ctx.render_ctx.stroke(rect, &border_brush, edge_width);
        }

        // Horizontal bar
        if let Some(bounds) = self.calc_horizontal_bar_bounds(port, env) {
            let rect = (bounds - scroll_offset)
                .inset(-edge_width / 2.0)
                .to_rounded_rect(radius);
            ctx.render_ctx.fill(rect, &brush);
            ctx.render_ctx.stroke(rect, &border_brush, edge_width);
        }
    }

    /// Tests if the specified point overlaps the vertical scrollbar
    ///
    /// Returns false if the vertical scrollbar is not visible
    pub fn point_hits_vertical_bar(
        &self,
        port: &Viewport,
        pos: Point,
        env: &Env,
    ) -> bool {
        let viewport_size = port.rect.size();
        let scroll_offset = port.rect.origin().to_vec2();

        if let Some(mut bounds) = self.calc_vertical_bar_bounds(port, env) {
            // Stretch hitbox to edge of widget
            bounds.x1 = scroll_offset.x + viewport_size.width;
            bounds.contains(pos)
        } else {
            false
        }
    }

    /// Tests if the specified point overlaps the horizontal scrollbar
    ///
    /// Returns false if the horizontal scrollbar is not visible
    pub fn point_hits_horizontal_bar(
        &self,
        port: &Viewport,
        pos: Point,
        env: &Env,
    ) -> bool {
        let viewport_size = port.rect.size();
        let scroll_offset = port.rect.origin().to_vec2();

        if let Some(mut bounds) = self.calc_horizontal_bar_bounds(port, env) {
            // Stretch hitbox to edge of widget
            bounds.y1 = scroll_offset.y + viewport_size.height;
            bounds.contains(pos)
        } else {
            false
        }
    }

    /// Checks if the event applies to the scroll behavior, uses it, and marks it handled
    ///
    /// Make sure to call on every event
    pub fn event(
        &mut self,
        port: &mut Viewport,
        ctx: &mut EventCtx,
        event: &Event,
        env: &Env,
    ) {
        let viewport_size = port.rect.size();
        let content_size = port.content_size;
        let scroll_offset = port.rect.origin().to_vec2();

        let scrollbar_is_hovered = match event {
            Event::MouseMove(e) | Event::MouseUp(e) | Event::MouseDown(e) => {
                let offset_pos = e.pos + scroll_offset;
                self.point_hits_vertical_bar(port, offset_pos, env)
                    || self.point_hits_horizontal_bar(port, offset_pos, env)
            }
            _ => false,
        };

        if self.are_bars_held() {
            // if we're dragging a scrollbar
            match event {
                Event::MouseMove(event) => {
                    match self.held {
                        BarHeldState::Vertical(offset) => {
                            let scale_y = viewport_size.height / content_size.height;
                            let bounds = self
                                .calc_vertical_bar_bounds(port, env)
                                .unwrap_or(Rect::ZERO);
                            let mouse_y = event.pos.y + scroll_offset.y;
                            let delta = mouse_y - bounds.y0 - offset;
                            port.pan_by(Vec2::new(0f64, (delta / scale_y).ceil()));
                            ctx.set_handled();
                        }
                        BarHeldState::Horizontal(offset) => {
                            let scale_x = viewport_size.height / content_size.width;
                            let bounds = self
                                .calc_horizontal_bar_bounds(port, env)
                                .unwrap_or(Rect::ZERO);
                            let mouse_x = event.pos.x + scroll_offset.x;
                            let delta = mouse_x - bounds.x0 - offset;
                            port.pan_by(Vec2::new((delta / scale_x).ceil(), 0f64));
                            ctx.set_handled();
                        }
                        _ => (),
                    }
                    ctx.request_paint();
                }
                Event::MouseUp(_) => {
                    self.held = BarHeldState::None;
                    ctx.set_active(false);

                    if !scrollbar_is_hovered {
                        self.hovered = BarHoveredState::None;
                        self.reset_scrollbar_fade(|d| ctx.request_timer(d), env);
                    }

                    ctx.set_handled();
                }
                _ => (), // other events are a noop
            }
        } else if scrollbar_is_hovered {
            // if we're over a scrollbar but not dragging
            match event {
                Event::MouseMove(event) => {
                    let offset_pos = event.pos + scroll_offset;
                    if self.point_hits_vertical_bar(port, offset_pos, env) {
                        self.hovered = BarHoveredState::Vertical;
                    } else if self.point_hits_horizontal_bar(port, offset_pos, env) {
                        self.hovered = BarHoveredState::Horizontal;
                    } else {
                        unreachable!();
                    }

                    self.opacity = env.get(theme::SCROLLBAR_MAX_OPACITY);
                    self.timer_id = TimerToken::INVALID; // Cancel any fade out in progress
                    ctx.request_paint();
                    ctx.set_handled();
                }
                Event::MouseDown(event) => {
                    let pos = event.pos + scroll_offset;

                    if self.point_hits_vertical_bar(port, pos, env) {
                        ctx.set_active(true);
                        self.held = BarHeldState::Vertical(
                            // The bounds must be non-empty, because the point hits the scrollbar.
                            pos.y
                                - self
                                    .calc_vertical_bar_bounds(port, env)
                                    .unwrap()
                                    .y0,
                        );
                    } else if self.point_hits_horizontal_bar(port, pos, env) {
                        ctx.set_active(true);
                        self.held = BarHeldState::Horizontal(
                            // The bounds must be non-empty, because the point hits the scrollbar.
                            pos.x
                                - self
                                    .calc_horizontal_bar_bounds(port, env)
                                    .unwrap()
                                    .x0,
                        );
                    } else {
                        unreachable!();
                    }

                    ctx.set_handled();
                }
                // if the mouse was downed elsewhere, moved over a scroll bar and released: noop.
                Event::MouseUp(_) => (),
                _ => unreachable!(),
            }
        } else {
            match event {
                Event::MouseMove(_) => {
                    // if we have just stopped hovering
                    if self.hovered.is_hovered() && !scrollbar_is_hovered {
                        self.hovered = BarHoveredState::None;
                        self.reset_scrollbar_fade(|d| ctx.request_timer(d), env);
                    }
                }
                Event::Timer(id) if *id == self.timer_id => {
                    // Schedule scroll bars animation
                    ctx.request_anim_frame();
                    self.timer_id = TimerToken::INVALID;
                    ctx.set_handled();
                }
                Event::AnimFrame(interval) => {
                    // Guard by the timer id being invalid, otherwise the scroll bars would fade
                    // immediately if some other widget started animating.
                    if self.timer_id == TimerToken::INVALID {
                        // Animate scroll bars opacity
                        let diff = 2.0 * (*interval as f64) * 1e-9;
                        self.opacity -= diff;
                        if self.opacity > 0.0 {
                            ctx.request_anim_frame();
                        }

                        if let Some(bounds) =
                            self.calc_horizontal_bar_bounds(port, env)
                        {
                            ctx.request_paint_rect(bounds - scroll_offset);
                        }
                        if let Some(bounds) =
                            self.calc_vertical_bar_bounds(port, env)
                        {
                            ctx.request_paint_rect(bounds - scroll_offset);
                        }
                    }
                }

                _ => (),
            }
        }
    }

    /// Applies mousewheel scrolling if the event has not already been handled
    pub fn handle_scroll(
        &mut self,
        port: &mut Viewport,
        ctx: &mut EventCtx,
        event: &Event,
        env: &Env,
    ) {
        if !ctx.is_handled() {
            if let Event::Wheel(mouse) = event {
                if port.pan_by(mouse.wheel_delta) {
                    ctx.request_paint();
                    ctx.set_handled();
                    self.reset_scrollbar_fade(|d| ctx.request_timer(d), env);
                }
            }
        }
    }

    /// Perform any necessary action prompted by a lifecycle event
    ///
    /// Make sure to call on every lifecycle event
    pub fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        env: &Env,
    ) {
        if let LifeCycle::Size(_) = event {
            // Show the scrollbars any time our size changes
            ctx.request_paint();
            self.reset_scrollbar_fade(|d| ctx.request_timer(d), &env);
        }
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct ViewportNew {
    /// The size of the area that we have a viewport into.
    pub content_size: Size,
    /// The view rectangle.
    pub rect: Rect,
}

impl ViewportNew {
    /// Tries to find a position for the view rectangle that is contained in the content rectangle.
    ///
    /// If the supplied origin is good, returns it; if it isn't, we try to return the nearest
    /// origin that would make the view rectangle contained in the content rectangle. (This will
    /// fail if the content is smaller than the view, and we return `0.0` in each dimension where
    /// the content is smaller.)
    pub fn clamp_view_origin(&self, origin: Point) -> Point {
        let x = origin
            .x
            .min(self.content_size.width - self.rect.width())
            .max(0.0);
        let y = origin
            .y
            .min(self.content_size.height - self.rect.height())
            .max(0.0);
        Point::new(x, y)
    }

    /// Changes the viewport offset by `delta`, while trying to keep the view rectangle inside the
    /// content rectangle.
    ///
    /// Returns true if the offset actually changed. Even if `delta` is non-zero, the offset might
    /// not change. For example, if you try to move the viewport down but it is already at the
    /// bottom of the child widget, then the offset will not change and this function will return
    /// false.
    pub fn pan_by(&mut self, delta: Vec2) -> bool {
        self.pan_to(self.rect.origin() + delta)
    }

    /// Sets the viewport origin to `pos`, while trying to keep the view rectangle inside the
    /// content rectangle.
    ///
    /// Returns true if the position changed. Note that the valid values for the viewport origin
    /// are constrained by the size of the child, and so the origin might not get set to exactly
    /// `pos`.
    pub fn pan_to(&mut self, origin: Point) -> bool {
        let new_origin = self.clamp_view_origin(origin);
        if (new_origin - self.rect.origin()).hypot2() > 1e-12 {
            self.rect = self.rect.with_origin(new_origin);
            true
        } else {
            false
        }
    }

    pub fn force_pan_to(&mut self, origin: Point) {
        self.rect = self.rect.with_origin(origin);
    }

    /// Pan the smallest distance that makes the target [`Rect`] visible.
    ///
    /// If the target rect is larger than viewport size, we will prioritize
    /// the region of the target closest to its origin.
    pub fn pan_to_visible(&mut self, rect: Rect) -> bool {
        /// Given a position and the min and max edges of an axis,
        /// return a delta by which to adjust that axis such that the value
        /// falls between its edges.
        ///
        /// if the value already falls between the two edges, return 0.0.
        fn closest_on_axis(val: f64, min: f64, max: f64) -> f64 {
            assert!(min <= max);
            if val > min && val < max {
                0.0
            } else if val <= min {
                val - min
            } else {
                val - max
            }
        }

        // clamp the target region size to our own size.
        // this means we will show the portion of the target region that
        // includes the origin.
        let target_size = Size::new(
            rect.width().min(self.rect.width()),
            rect.height().min(self.rect.height()),
        );
        let rect = rect.with_size(target_size);

        let x0 = closest_on_axis(rect.min_x(), self.rect.min_x(), self.rect.max_x());
        let x1 = closest_on_axis(rect.max_x(), self.rect.min_x(), self.rect.max_x());
        let y0 = closest_on_axis(rect.min_y(), self.rect.min_y(), self.rect.max_y());
        let y1 = closest_on_axis(rect.max_y(), self.rect.min_y(), self.rect.max_y());

        let delta_x = if x0.abs() > x1.abs() { x0 } else { x1 };
        let delta_y = if y0.abs() > y1.abs() { y0 } else { y1 };
        let new_origin = self.rect.origin() + Vec2::new(delta_x, delta_y);
        self.pan_to(new_origin)
    }
}

pub struct ClipBoxNew<T, W> {
    child: WidgetPod<T, W>,
    port: ViewportNew,
    constrain_horizontal: bool,
    constrain_vertical: bool,
}

impl<T, W: Widget<T>> ClipBoxNew<T, W> {
    /// Creates a new `ClipBox` wrapping `child`.
    pub fn new(child: W) -> Self {
        Self {
            child: WidgetPod::new(child),
            port: Default::default(),
            constrain_horizontal: false,
            constrain_vertical: false,
        }
    }

    /// Returns a reference to the child widget.
    pub fn child(&self) -> &W {
        self.child.widget()
    }

    /// Returns a mutable reference to the child widget.
    pub fn child_mut(&mut self) -> &mut W {
        self.child.widget_mut()
    }

    /// Returns a the viewport describing this `ClipBox`'s position.
    pub fn viewport(&self) -> ViewportNew {
        self.port
    }

    /// Returns the size of the rectangular viewport into the child widget.
    /// To get the position of the viewport, see [`viewport_origin`].
    ///
    /// [`viewport_origin`]: struct.ClipBox.html#method.viewport_origin
    pub fn viewport_size(&self) -> Size {
        self.port.rect.size()
    }

    /// Returns the size of the child widget.
    pub fn content_size(&self) -> Size {
        self.port.content_size
    }

    /// Builder-style method for deciding whether to constrain the child horizontally. The default
    /// is `false`. See [`constrain_vertical`] for more details.
    ///
    /// [`constrain_vertical`]: struct.ClipBox.html#constrain_vertical
    pub fn constrain_horizontal(mut self, constrain: bool) -> Self {
        self.constrain_horizontal = constrain;
        self
    }

    /// Determine whether to constrain the child horizontally.
    ///
    /// See [`constrain_vertical`] for more details.
    ///
    /// [`constrain_vertical`]: struct.ClipBox.html#constrain_vertical
    pub fn set_constrain_horizontal(&mut self, constrain: bool) {
        self.constrain_horizontal = constrain;
    }

    /// Builder-style method for deciding whether to constrain the child vertically. The default
    /// is `false`.
    ///
    /// This setting affects how a `ClipBox` lays out its child.
    ///
    /// - When it is `false` (the default), the child does receive any upper bound on its height:
    ///   the idea is that the child can be as tall as it wants, and the viewport will somehow get
    ///   moved around to see all of it.
    /// - When it is `true`, the viewport's maximum height will be passed down as an upper bound on
    ///   the height of the child, and the viewport will set its own height to be the same as its
    ///   child's height.
    pub fn constrain_vertical(mut self, constrain: bool) -> Self {
        self.constrain_vertical = constrain;
        self
    }

    /// Determine whether to constrain the child vertically.
    ///
    /// See [`constrain_vertical`] for more details.
    ///
    /// [`constrain_vertical`]: struct.ClipBox.html#constrain_vertical
    pub fn set_constrain_vertical(&mut self, constrain: bool) {
        self.constrain_vertical = constrain;
    }

    /// Changes the viewport offset by `delta`.
    ///
    /// Returns true if the offset actually changed. Even if `delta` is non-zero, the offset might
    /// not change. For example, if you try to move the viewport down but it is already at the
    /// bottom of the child widget, then the offset will not change and this function will return
    /// false.
    pub fn pan_by(&mut self, delta: Vec2) -> bool {
        self.pan_to(self.viewport_origin() + delta)
    }

    /// Sets the viewport origin to `pos`.
    ///
    /// Returns true if the position changed. Note that the valid values for the viewport origin
    /// are constrained by the size of the child, and so the origin might not get set to exactly
    /// `pos`.
    pub fn pan_to(&mut self, origin: Point) -> bool {
        if self.port.pan_to(origin) {
            self.child
                .set_viewport_offset(self.viewport_origin().to_vec2());
            true
        } else {
            false
        }
    }

    pub fn force_pan_to(&mut self, origin: Point) {
        self.port.force_pan_to(origin);
        self.child
            .set_viewport_offset(self.viewport_origin().to_vec2());
    }

    /// Adjust the viewport to display as much of the target region as is possible.
    ///
    /// Returns `true` if the viewport changes.
    ///
    /// This will move the viewport the smallest distance that fully shows
    /// the target region. If the target region is larger than the viewport,
    /// we will display the portion that fits, prioritizing the portion closest
    /// to the origin.
    pub fn pan_to_visible(&mut self, region: Rect) -> bool {
        if self.port.pan_to_visible(region) {
            self.child
                .set_viewport_offset(self.viewport_origin().to_vec2());
            true
        } else {
            false
        }
    }

    /// Returns the origin of the viewport rectangle.
    pub fn viewport_origin(&self) -> Point {
        self.port.rect.origin()
    }

    /// Allows this `ClipBox`'s viewport rectangle to be modified. The provided callback function
    /// can modify its argument, and when it is done then this `ClipBox` will be modified to have
    /// the new viewport rectangle.
    pub fn with_port<F: FnOnce(&mut ViewportNew)>(&mut self, f: F) {
        f(&mut self.port);
        self.child
            .set_viewport_offset(self.viewport_origin().to_vec2());
    }
}

impl<T: Data, W: Widget<T>> Widget<T> for ClipBoxNew<T, W> {
    fn event(&mut self, ctx: &mut EventCtx, ev: &Event, data: &mut T, env: &Env) {
        let viewport = ctx.size().to_rect();
        let force_event = self.child.is_hot() || self.child.has_active();
        if let Some(child_event) = ev.transform_scroll(
            self.viewport_origin().to_vec2(),
            viewport,
            force_event,
        ) {
            self.child.event(ctx, &child_event, data, env);
        }
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        ev: &LifeCycle,
        data: &T,
        env: &Env,
    ) {
        self.child.lifecycle(ctx, ev, data, env);
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &T, data: &T, env: &Env) {
        self.child.update(ctx, data, env);
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &T,
        env: &Env,
    ) -> Size {
        bc.debug_check("ClipBox");

        let content_size = self.child.layout(ctx, &bc, data, env);
        self.port.content_size = content_size;
        self.child.set_origin(ctx, data, env, Point::ORIGIN);

        self.port.rect = self.port.rect.with_size(bc.constrain(content_size));
        let new_offset = self.port.clamp_view_origin(self.viewport_origin());
        self.pan_to(new_offset);
        self.viewport_size()
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        let viewport = ctx.size().to_rect();
        let offset = self.viewport_origin().to_vec2();
        ctx.with_save(|ctx| {
            ctx.clip(viewport);
            ctx.transform(Affine::translate(-offset));

            let mut visible = ctx.region().clone();
            visible += offset;
            ctx.with_child_ctx(visible, |ctx| self.child.paint_raw(ctx, data, env));
        });
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ScrollComponentNew {
    /// Current opacity for both scrollbars
    pub opacity: f64,
    /// ID for the timer which schedules scrollbar fade out
    pub timer_id: TimerToken,
    pub fade_interval_id: TimerToken,
    /// Which if any scrollbar is currently hovered by the mouse
    pub hovered: BarHoveredState,
    /// Which if any scrollbar is currently being dragged by the mouse
    pub held: BarHeldState,
}

impl Default for ScrollComponentNew {
    fn default() -> Self {
        Self {
            opacity: 0.0,
            timer_id: TimerToken::INVALID,
            fade_interval_id: TimerToken::INVALID,
            hovered: BarHoveredState::None,
            held: BarHeldState::None,
        }
    }
}

impl ScrollComponentNew {
    /// Constructs a new [`ScrollComponent`](struct.ScrollComponent.html) for use.
    pub fn new() -> ScrollComponentNew {
        Default::default()
    }

    /// true if either scrollbar is currently held down/being dragged
    pub fn are_bars_held(&self) -> bool {
        !matches!(self.held, BarHeldState::None)
    }

    /// Makes the scrollbars visible, and resets the fade timer.
    pub fn reset_scrollbar_fade<F>(&mut self, request_timer: F, env: &Env)
    where
        F: FnOnce(Duration) -> TimerToken,
    {
        self.opacity = env.get(theme::SCROLLBAR_MAX_OPACITY);
        let fade_delay = 500;
        let deadline = Duration::from_millis(fade_delay);
        self.timer_id = request_timer(deadline);
    }

    /// Calculates the paint rect of the vertical scrollbar, or `None` if the vertical scrollbar is
    /// not visible.
    pub fn calc_vertical_bar_bounds(
        &self,
        port: &ViewportNew,
        env: &Env,
    ) -> Option<Rect> {
        let viewport_size = port.rect.size();
        let content_size = port.content_size;
        let scroll_offset = port.rect.origin().to_vec2();

        if viewport_size.height >= content_size.height {
            return None;
        }

        let bar_width = env.get(theme::SCROLLBAR_WIDTH);
        let bar_pad = env.get(theme::SCROLLBAR_PAD);

        let percent_visible = viewport_size.height / content_size.height;
        let percent_scrolled =
            scroll_offset.y / (content_size.height - viewport_size.height);

        let length = (percent_visible * viewport_size.height).ceil();
        let length = length.max(SCROLLBAR_MIN_SIZE);

        let vertical_padding = bar_pad + bar_pad + bar_width;

        let top_y_offset = ((viewport_size.height - length - vertical_padding)
            * percent_scrolled)
            .ceil();
        let bottom_y_offset = top_y_offset + length;

        let x0 = scroll_offset.x + viewport_size.width - bar_width - bar_pad;
        let y0 = scroll_offset.y + top_y_offset + bar_pad;

        let x1 = scroll_offset.x + viewport_size.width - bar_pad;
        let y1 = scroll_offset.y + bottom_y_offset;

        Some(Rect::new(x0, y0, x1, y1))
    }

    /// Calculates the paint rect of the horizontal scrollbar, or `None` if the horizontal
    /// scrollbar is not visible.
    pub fn calc_horizontal_bar_bounds(
        &self,
        port: &ViewportNew,
        env: &Env,
    ) -> Option<Rect> {
        let viewport_size = port.rect.size();
        let content_size = port.content_size;
        let scroll_offset = port.rect.origin().to_vec2();

        if viewport_size.width >= content_size.width {
            return None;
        }

        let bar_width = env.get(theme::SCROLLBAR_WIDTH);
        let bar_pad = env.get(theme::SCROLLBAR_PAD);

        let percent_visible = viewport_size.width / content_size.width;
        let percent_scrolled =
            scroll_offset.x / (content_size.width - viewport_size.width);

        let length = (percent_visible * viewport_size.width).ceil();
        let length = length.max(SCROLLBAR_MIN_SIZE);

        let horizontal_padding = bar_pad + bar_pad + bar_width;

        let left_x_offset = ((viewport_size.width - length - horizontal_padding)
            * percent_scrolled)
            .ceil();
        let right_x_offset = left_x_offset + length;

        let x0 = scroll_offset.x + left_x_offset + bar_pad;
        let y0 = scroll_offset.y + viewport_size.height - bar_width - bar_pad;

        let x1 = scroll_offset.x + right_x_offset;
        let y1 = scroll_offset.y + viewport_size.height - bar_pad;

        Some(Rect::new(x0, y0, x1, y1))
    }

    /// Draw scroll bars.
    pub fn draw_bars(&self, ctx: &mut PaintCtx, port: &ViewportNew, env: &Env) {
        let scroll_offset = port.rect.origin().to_vec2();
        if self.opacity <= 0.0 {
            return;
        }

        let brush = ctx
            .render_ctx
            .solid_brush(env.get(theme::SCROLLBAR_COLOR).with_alpha(self.opacity));
        let border_brush = ctx.render_ctx.solid_brush(
            env.get(theme::SCROLLBAR_BORDER_COLOR)
                .with_alpha(self.opacity),
        );

        let radius = env.get(theme::SCROLLBAR_RADIUS);
        let edge_width = env.get(theme::SCROLLBAR_EDGE_WIDTH);

        // Vertical bar
        if let Some(bounds) = self.calc_vertical_bar_bounds(port, env) {
            let rect = (bounds - scroll_offset).inset(-edge_width / 2.0);
            ctx.render_ctx.fill(rect, &brush);
            ctx.render_ctx.stroke(rect, &border_brush, edge_width);
        }

        // Horizontal bar
        if let Some(bounds) = self.calc_horizontal_bar_bounds(port, env) {
            let rect = (bounds - scroll_offset).inset(-edge_width / 2.0);
            ctx.render_ctx.fill(
                rect,
                &env.get(theme::SCROLLBAR_COLOR).with_alpha(self.opacity),
            );
            ctx.render_ctx.stroke(
                rect,
                &env.get(theme::SCROLLBAR_BORDER_COLOR)
                    .with_alpha(self.opacity),
                edge_width,
            );
        }
    }

    /// Tests if the specified point overlaps the vertical scrollbar
    ///
    /// Returns false if the vertical scrollbar is not visible
    pub fn point_hits_vertical_bar(
        &self,
        port: &ViewportNew,
        pos: Point,
        env: &Env,
    ) -> bool {
        let viewport_size = port.rect.size();
        let scroll_offset = port.rect.origin().to_vec2();

        if let Some(mut bounds) = self.calc_vertical_bar_bounds(port, env) {
            // Stretch hitbox to edge of widget
            bounds.x1 = scroll_offset.x + viewport_size.width;
            bounds.contains(pos)
        } else {
            false
        }
    }

    /// Tests if the specified point overlaps the horizontal scrollbar
    ///
    /// Returns false if the horizontal scrollbar is not visible
    pub fn point_hits_horizontal_bar(
        &self,
        port: &ViewportNew,
        pos: Point,
        env: &Env,
    ) -> bool {
        let viewport_size = port.rect.size();
        let scroll_offset = port.rect.origin().to_vec2();

        if let Some(mut bounds) = self.calc_horizontal_bar_bounds(port, env) {
            // Stretch hitbox to edge of widget
            bounds.y1 = scroll_offset.y + viewport_size.height;
            bounds.contains(pos)
        } else {
            false
        }
    }

    /// Checks if the event applies to the scroll behavior, uses it, and marks it handled
    ///
    /// Make sure to call on every event
    pub fn event(
        &mut self,
        port: &mut ViewportNew,
        ctx: &mut EventCtx,
        event: &Event,
        env: &Env,
    ) {
        let viewport_size = port.rect.size();
        let content_size = port.content_size;
        let scroll_offset = port.rect.origin().to_vec2();

        let scrollbar_is_hovered = match event {
            Event::MouseMove(e) | Event::MouseUp(e) | Event::MouseDown(e) => {
                let offset_pos = e.pos + scroll_offset;
                self.point_hits_vertical_bar(port, offset_pos, env)
                    || self.point_hits_horizontal_bar(port, offset_pos, env)
            }
            _ => false,
        };

        if self.are_bars_held() {
            // if we're dragging a scrollbar
            match event {
                Event::MouseMove(event) => {
                    match self.held {
                        BarHeldState::Vertical(offset) => {
                            let scale_y = viewport_size.height / content_size.height;
                            let bounds = self
                                .calc_vertical_bar_bounds(port, env)
                                .unwrap_or(Rect::ZERO);
                            let mouse_y = event.pos.y + scroll_offset.y;
                            let delta = mouse_y - bounds.y0 - offset;
                            port.pan_by(Vec2::new(0f64, (delta / scale_y).ceil()));
                            ctx.set_handled();
                        }
                        BarHeldState::Horizontal(offset) => {
                            let scale_x = viewport_size.height / content_size.width;
                            let bounds = self
                                .calc_horizontal_bar_bounds(port, env)
                                .unwrap_or(Rect::ZERO);
                            let mouse_x = event.pos.x + scroll_offset.x;
                            let delta = mouse_x - bounds.x0 - offset;
                            port.pan_by(Vec2::new((delta / scale_x).ceil(), 0f64));
                            ctx.set_handled();
                        }
                        _ => (),
                    }
                    ctx.request_paint();
                }
                Event::MouseUp(_) => {
                    self.held = BarHeldState::None;
                    ctx.set_active(false);

                    if !scrollbar_is_hovered {
                        self.hovered = BarHoveredState::None;
                        self.reset_scrollbar_fade(|d| ctx.request_timer(d), env);
                    }

                    ctx.set_handled();
                }
                _ => (), // other events are a noop
            }
        } else if scrollbar_is_hovered {
            // if we're over a scrollbar but not dragging
            match event {
                Event::MouseMove(event) => {
                    let offset_pos = event.pos + scroll_offset;
                    if self.point_hits_vertical_bar(port, offset_pos, env) {
                        self.hovered = BarHoveredState::Vertical;
                    } else if self.point_hits_horizontal_bar(port, offset_pos, env) {
                        self.hovered = BarHoveredState::Horizontal;
                    } else {
                        unreachable!();
                    }

                    self.opacity = env.get(theme::SCROLLBAR_MAX_OPACITY);
                    self.timer_id = TimerToken::INVALID; // Cancel any fade out in progress
                    ctx.request_paint();
                    ctx.set_handled();
                }
                Event::MouseDown(event) => {
                    let pos = event.pos + scroll_offset;

                    if self.point_hits_vertical_bar(port, pos, env) {
                        ctx.set_active(true);
                        self.held = BarHeldState::Vertical(
                            // The bounds must be non-empty, because the point hits the scrollbar.
                            pos.y
                                - self
                                    .calc_vertical_bar_bounds(port, env)
                                    .unwrap()
                                    .y0,
                        );
                    } else if self.point_hits_horizontal_bar(port, pos, env) {
                        ctx.set_active(true);
                        self.held = BarHeldState::Horizontal(
                            // The bounds must be non-empty, because the point hits the scrollbar.
                            pos.x
                                - self
                                    .calc_horizontal_bar_bounds(port, env)
                                    .unwrap()
                                    .x0,
                        );
                    } else {
                        unreachable!();
                    }

                    ctx.set_handled();
                }
                // if the mouse was downed elsewhere, moved over a scroll bar and released: noop.
                Event::MouseUp(_) => (),
                _ => unreachable!(),
            }
        } else {
            match event {
                Event::MouseMove(_) => {
                    // if we have just stopped hovering
                    if self.hovered.is_hovered() && !scrollbar_is_hovered {
                        self.hovered = BarHoveredState::None;
                        self.reset_scrollbar_fade(|d| ctx.request_timer(d), env);
                    }
                }
                Event::Timer(id) if *id == self.timer_id => {
                    // Schedule scroll bars animation
                    self.timer_id = TimerToken::INVALID;
                    self.fade_interval_id =
                        ctx.request_timer(Duration::from_millis(20));
                    ctx.set_handled();
                }
                Event::Timer(id) if *id == self.fade_interval_id => {
                    if self.timer_id == TimerToken::INVALID {
                        let diff = 0.02;
                        self.opacity -= diff;
                        if self.opacity > 0.0 {
                            self.fade_interval_id =
                                ctx.request_timer(Duration::from_millis(20));
                            if let Some(bounds) =
                                self.calc_horizontal_bar_bounds(port, env)
                            {
                                ctx.request_paint_rect(bounds - scroll_offset);
                            }
                            if let Some(bounds) =
                                self.calc_vertical_bar_bounds(port, env)
                            {
                                ctx.request_paint_rect(bounds - scroll_offset);
                            }
                        }
                    }
                }
                _ => (),
            }
        }
    }

    /// Applies mousewheel scrolling if the event has not already been handled
    pub fn handle_scroll(
        &mut self,
        port: &mut ViewportNew,
        ctx: &mut EventCtx,
        event: &Event,
        env: &Env,
    ) {
        if !ctx.is_handled() {
            if let Event::Wheel(mouse) = event {
                if port.pan_by(mouse.wheel_delta.round()) {}
                ctx.request_paint();
                self.reset_scrollbar_fade(|d| ctx.request_timer(d), env);
                ctx.set_handled();
            }
        }
    }

    /// Perform any necessary action prompted by a lifecycle event
    ///
    /// Make sure to call on every lifecycle event
    pub fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        env: &Env,
    ) {
        if let LifeCycle::Size(_) = event {
            // Show the scrollbars any time our size changes
            ctx.request_paint();
            self.reset_scrollbar_fade(|d| ctx.request_timer(d), &env);
        }
    }
}

pub struct LapceScrollNew<T, W> {
    clip: ClipBoxNew<T, W>,
    scroll_component: ScrollComponentNew,
}

impl<T, W: Widget<T>> LapceScrollNew<T, W> {
    /// Create a new scroll container.
    ///
    /// This method will allow scrolling in all directions if child's bounds
    /// are larger than the viewport. Use [vertical](#method.vertical) and
    /// [horizontal](#method.horizontal) methods to limit scrolling to a specific axis.
    pub fn new(child: W) -> LapceScrollNew<T, W> {
        Self {
            clip: ClipBoxNew::new(child),
            scroll_component: ScrollComponentNew::new(),
        }
    }

    /// Restrict scrolling to the vertical axis while locking child width.
    pub fn vertical(mut self) -> Self {
        self.clip.set_constrain_vertical(false);
        self.clip.set_constrain_horizontal(true);
        self
    }

    /// Restrict scrolling to the horizontal axis while locking child height.
    pub fn horizontal(mut self) -> Self {
        self.clip.set_constrain_vertical(true);
        self.clip.set_constrain_horizontal(false);
        self
    }

    /// Returns a reference to the child widget.
    pub fn child(&self) -> &W {
        self.clip.child()
    }

    /// Returns a mutable reference to the child widget.
    pub fn child_mut(&mut self) -> &mut W {
        self.clip.child_mut()
    }

    /// Returns the size of the child widget.
    pub fn child_size(&self) -> Size {
        self.clip.content_size()
    }

    pub fn set_child_size(&mut self, size: Size) {
        self.clip.port.content_size = size;
    }

    /// Returns the current scroll offset.
    pub fn offset(&self) -> Vec2 {
        self.clip.viewport_origin().to_vec2()
    }

    /// Scroll `delta` units.
    ///
    /// Returns `true` if the scroll offset has changed.
    pub fn scroll_by(&mut self, delta: Vec2) -> bool {
        self.clip.pan_by(delta)
    }

    pub fn force_scroll_to(&mut self, point: Point) {
        self.clip.force_pan_to(point)
    }

    pub fn scroll_to(&mut self, point: Point) -> bool {
        self.clip.pan_to(point)
    }

    /// Scroll the minimal distance to show the target rect.
    ///
    /// If the target region is larger than the viewport, we will display the
    /// portion that fits, prioritizing the portion closest to the origin.
    pub fn scroll_to_visible(&mut self, region: Rect, env: &Env) -> bool {
        if self.clip.pan_to_visible(region) {
            true
        } else {
            false
        }
    }
}

impl<T: Data, W: Widget<T>> Widget<T> for LapceScrollNew<T, W> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        let scroll_component = &mut self.scroll_component;
        self.clip.with_port(|port| {
            scroll_component.event(port, ctx, event, env);
        });
        if !ctx.is_handled() {
            self.clip.event(ctx, event, data, env);
        }

        self.clip.with_port(|port| {
            scroll_component.handle_scroll(port, ctx, event, env);
        });

        match event {
            Event::Command(cmd) if cmd.is(LAPCE_UI_COMMAND) => {
                let command = cmd.get_unchecked(LAPCE_UI_COMMAND);
                match command {
                    LapceUICommand::ResetFade => {
                        scroll_component
                            .reset_scrollbar_fade(|d| ctx.request_timer(d), env);
                    }
                    _ => (),
                }
            }
            _ => (),
        }
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &T,
        env: &Env,
    ) {
        self.scroll_component.lifecycle(ctx, event, env);
        self.clip.lifecycle(ctx, event, data, env);
    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &T, data: &T, env: &Env) {
        self.clip.update(ctx, old_data, data, env);
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &T,
        env: &Env,
    ) -> Size {
        bc.debug_check("Scroll");

        let old_viewport = self.clip.port.clone();
        let child_size = self.clip.layout(ctx, &bc, data, env);

        let self_size = bc.constrain(child_size);
        // The new size might have made the current scroll offset invalid. This makes it valid
        // again.
        let _ = self.scroll_by(Vec2::ZERO);
        if old_viewport != self.clip.port {
            self.scroll_component
                .reset_scrollbar_fade(|d| ctx.request_timer(d), env);
        }

        self_size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        self.clip.paint(ctx, data, env);
        self.scroll_component
            .draw_bars(ctx, &self.clip.viewport(), env);
    }
}

pub struct LapcePadding<T, W> {
    left: f64,
    right: f64,
    top: f64,
    bottom: f64,

    child: WidgetPod<T, W>,
}

impl<T, W: Widget<T>> LapcePadding<T, W> {
    pub fn new(insets: impl Into<Insets>, child: W) -> Self {
        let insets = insets.into();
        Self {
            left: insets.x0,
            right: insets.x1,
            top: insets.y0,
            bottom: insets.y1,
            child: WidgetPod::new(child),
        }
    }

    pub fn child_size(&self) -> Size {
        self.child.layout_rect().size()
    }

    pub fn child(&self) -> &W {
        self.child.widget()
    }

    /// Returns a mutable reference to the child widget.
    pub fn child_mut(&mut self) -> &mut W {
        self.child.widget_mut()
    }
}

impl<T: Data, W: Widget<T>> Widget<T> for LapcePadding<T, W> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        self.child.event(ctx, event, data, env)
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &T,
        env: &Env,
    ) {
        self.child.lifecycle(ctx, event, data, env)
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &T, data: &T, env: &Env) {
        self.child.update(ctx, data, env);
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &T,
        env: &Env,
    ) -> Size {
        bc.debug_check("Padding");

        let hpad = self.left + self.right;
        let vpad = self.top + self.bottom;

        let child_bc = bc.shrink((hpad, vpad));
        let size = self.child.layout(ctx, &child_bc, data, env);
        let origin = Point::new(self.left, self.top);
        self.child.set_origin(ctx, data, env, origin);

        let my_size = Size::new(size.width + hpad, size.height + vpad);
        let my_insets = self.child.compute_parent_paint_insets(my_size);
        ctx.set_paint_insets(my_insets);
        my_size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        self.child.paint(ctx, data, env);
    }
}

pub struct LapceIdentityWrapper<W> {
    id: WidgetId,
    inner: W,
}

impl<W> LapceIdentityWrapper<W> {
    /// Assign an identity to a widget.
    pub fn wrap(inner: W, id: WidgetId) -> LapceIdentityWrapper<W> {
        Self { id, inner }
    }

    pub fn inner(&self) -> &W {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut W {
        &mut self.inner
    }
}

impl<T: Data, W: Widget<T>> Widget<T> for LapceIdentityWrapper<W> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        self.inner.event(ctx, event, data, env);
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &T,
        env: &Env,
    ) {
        self.inner.lifecycle(ctx, event, data, env)
    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &T, data: &T, env: &Env) {
        self.inner.update(ctx, old_data, data, env);
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &T,
        env: &Env,
    ) -> Size {
        self.inner.layout(ctx, bc, data, env)
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        self.inner.paint(ctx, data, env);
    }

    fn id(&self) -> Option<WidgetId> {
        Some(self.id)
    }
}
