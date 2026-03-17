// window management for split panes

use crate::cursor::Cursor;
use crate::viewport::Viewport;

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug)]
pub struct Window {
    pub buffer_index: usize,
    pub cursor: Cursor,
    pub viewport: Viewport,
    pub rect: Rect, // screen region for this window
}

impl Window {
    pub fn new(buffer_index: usize, width: usize, height: usize) -> Self {
        Self {
            buffer_index,
            cursor: Cursor::default(),
            viewport: Viewport::new(width, height),
            rect: Rect { x: 0, y: 0, width, height },
        }
    }

    pub fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
        self.viewport.resize(rect.width, rect.height);
    }
}

#[derive(Debug, Clone)]
pub enum Layout {
    Leaf(usize),                              // window index
    Horizontal(Vec<(Layout, f32)>),           // side-by-side, ratio per child
    Vertical(Vec<(Layout, f32)>),             // top-bottom, ratio per child
}

impl Layout {
    pub fn new_leaf(window_index: usize) -> Self { Layout::Leaf(window_index) }

    /// calculate rects for all leaves given a bounding rect
    pub fn calculate_rects(&self, rect: Rect) -> Vec<(usize, Rect)> {
        let mut result = Vec::new();
        self.calc_inner(rect, &mut result);
        result
    }

    fn calc_inner(&self, rect: Rect, out: &mut Vec<(usize, Rect)>) {
        match self {
            Layout::Leaf(idx) => { out.push((*idx, rect)); }
            Layout::Horizontal(children) => {
                let total: f32 = children.iter().map(|(_, r)| r).sum();
                let mut x = rect.x;
                for (i, (child, ratio)) in children.iter().enumerate() {
                    let w = if i == children.len() - 1 {
                        rect.x + rect.width - x // last child gets remainder
                    } else {
                        ((rect.width as f32 * ratio / total) as usize).max(1)
                    };
                    let child_rect = Rect { x, y: rect.y, width: w.saturating_sub(if i < children.len() - 1 { 1 } else { 0 }), height: rect.height };
                    child.calc_inner(child_rect, out);
                    x += w;
                }
            }
            Layout::Vertical(children) => {
                let total: f32 = children.iter().map(|(_, r)| r).sum();
                let mut y = rect.y;
                for (i, (child, ratio)) in children.iter().enumerate() {
                    let h = if i == children.len() - 1 {
                        rect.y + rect.height - y
                    } else {
                        ((rect.height as f32 * ratio / total) as usize).max(1)
                    };
                    let child_rect = Rect { x: rect.x, y, width: rect.width, height: h.saturating_sub(if i < children.len() - 1 { 1 } else { 0 }) };
                    child.calc_inner(child_rect, out);
                    y += h;
                }
            }
        }
    }

    /// split a leaf node horizontally (side-by-side), returning new window index location
    pub fn split_horizontal(&mut self, target: usize, new_idx: usize) -> bool {
        match self {
            Layout::Leaf(idx) if *idx == target => {
                *self = Layout::Horizontal(vec![
                    (Layout::Leaf(target), 1.0),
                    (Layout::Leaf(new_idx), 1.0),
                ]);
                true
            }
            Layout::Horizontal(children) | Layout::Vertical(children) => {
                for (child, _) in children.iter_mut() {
                    if child.split_horizontal(target, new_idx) { return true; }
                }
                false
            }
            _ => false,
        }
    }

    /// split a leaf node vertically (top-bottom)
    pub fn split_vertical(&mut self, target: usize, new_idx: usize) -> bool {
        match self {
            Layout::Leaf(idx) if *idx == target => {
                *self = Layout::Vertical(vec![
                    (Layout::Leaf(target), 1.0),
                    (Layout::Leaf(new_idx), 1.0),
                ]);
                true
            }
            Layout::Horizontal(children) | Layout::Vertical(children) => {
                for (child, _) in children.iter_mut() {
                    if child.split_vertical(target, new_idx) { return true; }
                }
                false
            }
            _ => false,
        }
    }

    /// remove a leaf, returns true if removed
    pub fn remove(&mut self, target: usize) -> bool {
        match self {
            Layout::Horizontal(children) | Layout::Vertical(children) => {
                let before = children.len();
                children.retain(|(child, _)| !matches!(child, Layout::Leaf(idx) if *idx == target));
                if children.len() < before { return true; }
                for (child, _) in children.iter_mut() {
                    if child.remove(target) { return true; }
                }
                false
            }
            _ => false,
        }
    }

    /// collect all leaf window indices
    pub fn leaves(&self) -> Vec<usize> {
        let mut result = Vec::new();
        self.collect_leaves(&mut result);
        result
    }

    fn collect_leaves(&self, out: &mut Vec<usize>) {
        match self {
            Layout::Leaf(idx) => { out.push(*idx); }
            Layout::Horizontal(children) | Layout::Vertical(children) => {
                for (child, _) in children { child.collect_leaves(out); }
            }
        }
    }
}
