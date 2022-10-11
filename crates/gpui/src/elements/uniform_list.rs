use super::{Element, Event, EventContext, LayoutContext, PaintContext, SizeConstraint};
use crate::{
    geometry::{
        rect::RectF,
        vector::{vec2f, Vector2F},
    },
    json::{self, json},
    presenter::MeasurementContext,
    scene::ScrollWheelRegionEvent,
    ElementBox, MouseRegion, RenderContext, ScrollWheelEvent, View,
};
use json::ToJson;
use std::{cell::RefCell, cmp, ops::Range, rc::Rc};

#[derive(Clone, Default)]
pub struct UniformListState(Rc<RefCell<StateInner>>);

#[derive(Debug)]
pub enum ScrollTarget {
    Show(usize),
    Center(usize),
}

impl UniformListState {
    pub fn scroll_to(&self, scroll_to: ScrollTarget) {
        self.0.borrow_mut().scroll_to = Some(scroll_to);
    }

    pub fn scroll_top(&self) -> f32 {
        self.0.borrow().scroll_top
    }
}

#[derive(Default)]
struct StateInner {
    scroll_top: f32,
    scroll_to: Option<ScrollTarget>,
}

pub struct LayoutState {
    scroll_max: f32,
    item_height: f32,
    items: Vec<ElementBox>,
}

pub struct UniformList {
    state: UniformListState,
    item_count: usize,
    #[allow(clippy::type_complexity)]
    append_items: Box<dyn Fn(Range<usize>, &mut Vec<ElementBox>, &mut LayoutContext)>,
    padding_top: f32,
    padding_bottom: f32,
    get_width_from_item: Option<usize>,
    view_id: usize,
}

impl UniformList {
    pub fn new<F, V>(
        state: UniformListState,
        item_count: usize,
        cx: &mut RenderContext<V>,
        append_items: F,
    ) -> Self
    where
        V: View,
        F: 'static + Fn(&mut V, Range<usize>, &mut Vec<ElementBox>, &mut RenderContext<V>),
    {
        let handle = cx.handle();
        Self {
            state,
            item_count,
            append_items: Box::new(move |range, items, cx| {
                if let Some(handle) = handle.upgrade(cx) {
                    cx.render(&handle, |view, cx| {
                        append_items(view, range, items, cx);
                    });
                }
            }),
            padding_top: 0.,
            padding_bottom: 0.,
            get_width_from_item: None,
            view_id: cx.handle().id(),
        }
    }

    pub fn with_width_from_item(mut self, item_ix: Option<usize>) -> Self {
        self.get_width_from_item = item_ix;
        self
    }

    pub fn with_padding_top(mut self, padding: f32) -> Self {
        self.padding_top = padding;
        self
    }

    pub fn with_padding_bottom(mut self, padding: f32) -> Self {
        self.padding_bottom = padding;
        self
    }

    fn scroll(
        state: UniformListState,
        _: Vector2F,
        mut delta: Vector2F,
        precise: bool,
        scroll_max: f32,
        cx: &mut EventContext,
    ) -> bool {
        if !precise {
            delta *= 20.;
        }

        let mut state = state.0.borrow_mut();
        state.scroll_top = (state.scroll_top - delta.y()).max(0.0).min(scroll_max);
        cx.notify();

        true
    }

    fn autoscroll(&mut self, scroll_max: f32, list_height: f32, item_height: f32) {
        let mut state = self.state.0.borrow_mut();

        if let Some(scroll_to) = state.scroll_to.take() {
            let item_ix;
            let center;
            match scroll_to {
                ScrollTarget::Show(ix) => {
                    item_ix = ix;
                    center = false;
                }
                ScrollTarget::Center(ix) => {
                    item_ix = ix;
                    center = true;
                }
            }

            let item_top = self.padding_top + item_ix as f32 * item_height;
            let item_bottom = item_top + item_height;
            if center {
                let item_center = item_top + item_height / 2.;
                state.scroll_top = (item_center - list_height / 2.).max(0.);
            } else {
                let scroll_bottom = state.scroll_top + list_height;
                if item_top < state.scroll_top {
                    state.scroll_top = item_top;
                } else if item_bottom > scroll_bottom {
                    state.scroll_top = item_bottom - list_height;
                }
            }
        }

        if state.scroll_top > scroll_max {
            state.scroll_top = scroll_max;
        }
    }

    fn scroll_top(&self) -> f32 {
        self.state.0.borrow().scroll_top
    }
}

impl Element for UniformList {
    type LayoutState = LayoutState;
    type PaintState = ();

    fn layout(
        &mut self,
        constraint: SizeConstraint,
        cx: &mut LayoutContext,
    ) -> (Vector2F, Self::LayoutState) {
        if constraint.max.y().is_infinite() {
            unimplemented!(
                "UniformList does not support being rendered with an unconstrained height"
            );
        }

        let no_items = (
            constraint.min,
            LayoutState {
                item_height: 0.,
                scroll_max: 0.,
                items: Default::default(),
            },
        );

        if self.item_count == 0 {
            return no_items;
        }

        let mut items = Vec::new();
        let mut size = constraint.max;
        let mut item_size;
        let sample_item_ix;
        let sample_item;
        if let Some(sample_ix) = self.get_width_from_item {
            (self.append_items)(sample_ix..sample_ix + 1, &mut items, cx);
            sample_item_ix = sample_ix;

            if let Some(mut item) = items.pop() {
                item_size = item.layout(constraint, cx);
                size.set_x(item_size.x());
                sample_item = item;
            } else {
                return no_items;
            }
        } else {
            (self.append_items)(0..1, &mut items, cx);
            sample_item_ix = 0;
            if let Some(mut item) = items.pop() {
                item_size = item.layout(
                    SizeConstraint::new(
                        vec2f(constraint.max.x(), 0.0),
                        vec2f(constraint.max.x(), f32::INFINITY),
                    ),
                    cx,
                );
                item_size.set_x(size.x());
                sample_item = item
            } else {
                return no_items;
            }
        }

        let item_constraint = SizeConstraint {
            min: item_size,
            max: vec2f(constraint.max.x(), item_size.y()),
        };
        let item_height = item_size.y();

        let scroll_height = self.item_count as f32 * item_height;
        if scroll_height < size.y() {
            size.set_y(size.y().min(scroll_height).max(constraint.min.y()));
        }

        let scroll_height =
            item_height * self.item_count as f32 + self.padding_top + self.padding_bottom;
        let scroll_max = (scroll_height - size.y()).max(0.);
        self.autoscroll(scroll_max, size.y(), item_height);

        let start = cmp::min(
            ((self.scroll_top() - self.padding_top) / item_height.max(1.)) as usize,
            self.item_count,
        );
        let end = cmp::min(
            self.item_count,
            start + (size.y() / item_height.max(1.)).ceil() as usize + 1,
        );

        if (start..end).contains(&sample_item_ix) {
            if sample_item_ix > start {
                (self.append_items)(start..sample_item_ix, &mut items, cx);
            }

            items.push(sample_item);

            if sample_item_ix < end {
                (self.append_items)(sample_item_ix + 1..end, &mut items, cx);
            }
        } else {
            (self.append_items)(start..end, &mut items, cx);
        }

        for item in &mut items {
            let item_size = item.layout(item_constraint, cx);
            if item_size.x() > size.x() {
                size.set_x(item_size.x());
            }
        }

        (
            size,
            LayoutState {
                item_height,
                scroll_max,
                items,
            },
        )
    }

    fn paint(
        &mut self,
        bounds: RectF,
        visible_bounds: RectF,
        layout: &mut Self::LayoutState,
        cx: &mut PaintContext,
    ) -> Self::PaintState {
        let visible_bounds = visible_bounds.intersection(bounds).unwrap_or_default();

        cx.scene.push_layer(Some(visible_bounds));

        cx.scene.push_mouse_region(
            MouseRegion::new::<Self>(self.view_id, 0, visible_bounds).on_scroll({
                let scroll_max = layout.scroll_max;
                let state = self.state.clone();
                move |ScrollWheelRegionEvent {
                          platform_event:
                              ScrollWheelEvent {
                                  position,
                                  delta,
                                  precise,
                                  ..
                              },
                          ..
                      },
                      cx| {
                    if !Self::scroll(state.clone(), position, delta, precise, scroll_max, cx) {
                        cx.propogate_event();
                    }
                }
            }),
        );

        let mut item_origin = bounds.origin()
            - vec2f(
                0.,
                (self.state.scroll_top() - self.padding_top) % layout.item_height,
            );

        for item in &mut layout.items {
            item.paint(item_origin, visible_bounds, cx);
            item_origin += vec2f(0.0, layout.item_height);
        }

        cx.scene.pop_layer();
    }

    fn dispatch_event(
        &mut self,
        event: &Event,
        _: RectF,
        _: RectF,
        layout: &mut Self::LayoutState,
        _: &mut Self::PaintState,
        cx: &mut EventContext,
    ) -> bool {
        let mut handled = false;
        for item in &mut layout.items {
            handled = item.dispatch_event(event, cx) || handled;
        }

        handled
    }

    fn rect_for_text_range(
        &self,
        range: Range<usize>,
        _: RectF,
        _: RectF,
        layout: &Self::LayoutState,
        _: &Self::PaintState,
        cx: &MeasurementContext,
    ) -> Option<RectF> {
        layout
            .items
            .iter()
            .find_map(|child| child.rect_for_text_range(range.clone(), cx))
    }

    fn debug(
        &self,
        bounds: RectF,
        layout: &Self::LayoutState,
        _: &Self::PaintState,
        cx: &crate::DebugContext,
    ) -> json::Value {
        json!({
            "type": "UniformList",
            "bounds": bounds.to_json(),
            "scroll_max": layout.scroll_max,
            "item_height": layout.item_height,
            "items": layout.items.iter().map(|item| item.debug(cx)).collect::<Vec<json::Value>>()

        })
    }
}
