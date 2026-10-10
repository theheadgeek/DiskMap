//! Squarified treemap: turns a tree of sizes into nested rectangles.

use crate::scan::Node;
use eframe::egui::{Color32, Rect, pos2, vec2};
use rayon::prelude::*;
use std::cmp::Reverse;

pub struct Tile {
    pub rect: Rect,
    /// Child indexes from the node the map was laid out from.
    pub path: Vec<usize>,
    pub color: Color32,
    /// Leaves are drawn and clicked; folder tiles are only used to outline a selected folder.
    pub is_leaf: bool,
}

/// Sizes to lay the map out by instead of the real ones, e.g. counting only
/// one file type. Mirrors the Node tree index for index.
pub struct Weights {
    pub size: u64,
    pub children: Vec<Weights>,
}

/// Every file's size if `keep` says so, otherwise 0, added up through the folders.
pub fn weigh(node: &Node, keep: &(dyn Fn(&Node) -> bool + Sync)) -> Weights {
    if !node.is_dir {
        return Weights {
            size: if keep(node) { node.size } else { 0 },
            children: Vec::new(),
        };
    }
    let children: Vec<Weights> = node.children.par_iter().map(|child| weigh(child, keep)).collect();
    Weights {
        size: children.iter().map(|c| c.size).sum(),
        children,
    }
}

/// Folders smaller than this (in points) are drawn as one block instead of being split up.
const MIN_SPLIT: f32 = 6.0;

/// Lays out `root` in `rect`, sized by `weights` when given, otherwise by real sizes.
pub fn layout(root: &Node, weights: Option<&Weights>, rect: Rect, color: &dyn Fn(&Node) -> Color32) -> Vec<Tile> {
    let mut tiles = Vec::new();
    place(root, weights, rect, &mut Vec::new(), color, &mut tiles);
    tiles
}

fn place(
    node: &Node,
    weights: Option<&Weights>,
    rect: Rect,
    path: &mut Vec<usize>,
    color: &dyn Fn(&Node) -> Color32,
    tiles: &mut Vec<Tile>,
) {
    let size = weights.map_or(node.size, |w| w.size);
    let splittable = node.is_dir && size > 0 && rect.width() >= MIN_SPLIT && rect.height() >= MIN_SPLIT;
    if !splittable {
        tiles.push(Tile {
            rect,
            path: path.clone(),
            color: color(node),
            is_leaf: true,
        });
        return;
    }
    tiles.push(Tile {
        rect,
        path: path.clone(),
        color: Color32::TRANSPARENT,
        is_leaf: false,
    });

    // A 1-point inset lets the background show between folders, so groups stay visible.
    let inner = if rect.width() > 16.0 && rect.height() > 16.0 {
        rect.shrink(1.0)
    } else {
        rect
    };
    // (child index, size) for every child with something to show, largest first.
    let mut children: Vec<(usize, u64)> = match weights {
        Some(w) => w.children.iter().map(|c| c.size).enumerate().filter(|&(_, s)| s > 0).collect(),
        None => node.children.iter().map(|c| c.size).enumerate().filter(|&(_, s)| s > 0).collect(),
    };
    if weights.is_some() {
        // Real sizes are already sorted; weighted ones need sorting here.
        children.sort_by_key(|&(_, size)| Reverse(size));
    }
    let sizes: Vec<f64> = children.iter().map(|&(_, s)| s as f64).collect();
    for (&(i, _), child_rect) in children.iter().zip(squarify(&sizes, inner)) {
        if child_rect.width() * child_rect.height() < 0.5 {
            continue; // too small to see
        }
        path.push(i);
        place(&node.children[i], weights.map(|w| &w.children[i]), child_rect, path, color, tiles);
        path.pop();
    }
}

/// Splits `rect` into one rectangle per size, keeping each as close to square
/// as possible. `sizes` must be sorted largest first.
/// The algorithm is from Bruls, Huizing & van Wijk, "Squarified Treemaps" (2000).
pub fn squarify(sizes: &[f64], rect: Rect) -> Vec<Rect> {
    let mut out = Vec::with_capacity(sizes.len());
    let total: f64 = sizes.iter().sum();
    if total <= 0.0 {
        return out;
    }
    let scale = (rect.width() as f64 * rect.height() as f64) / total;
    let areas: Vec<f64> = sizes.iter().map(|s| s * scale).collect();

    let mut free = rect;
    let mut start = 0;
    while start < areas.len() {
        let (free_w, free_h) = (free.width() as f64, free.height() as f64);
        if free_w <= 0.0 || free_h <= 0.0 {
            break;
        }
        // Keep adding items to this row while doing so makes its worst rectangle less stretched.
        let side = free_w.min(free_h);
        let mut end = start + 1;
        while end < areas.len() && worst(&areas[start..=end], side) <= worst(&areas[start..end], side) {
            end += 1;
        }
        let row = &areas[start..end];
        let row_area: f64 = row.iter().sum();

        if free_w >= free_h {
            // Wide space: the row becomes a column down the left edge.
            let w = row_area / free_h;
            let mut y = free.top() as f64;
            for area in row {
                let h = area / w;
                out.push(Rect::from_min_size(pos2(free.left(), y as f32), vec2(w as f32, h as f32)));
                y += h;
            }
            free.min.x += w as f32;
        } else {
            // Tall space: the row runs across the top.
            let h = row_area / free_w;
            let mut x = free.left() as f64;
            for area in row {
                let w = area / h;
                out.push(Rect::from_min_size(pos2(x as f32, free.top()), vec2(w as f32, h as f32)));
                x += w;
            }
            free.min.y += h as f32;
        }
        start = end;
    }
    out
}

/// How stretched the worst rectangle in a row would be (1.0 is a perfect square).
fn worst(row: &[f64], side: f64) -> f64 {
    let sum: f64 = row.iter().sum();
    let (largest, smallest) = (row[0], row[row.len() - 1]);
    let (sum2, side2) = (sum * sum, side * side);
    (side2 * largest / sum2).max(sum2 / (side2 * smallest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_the_rect_with_the_right_areas() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(600.0, 400.0));
        let sizes = [6.0, 6.0, 4.0, 3.0, 2.0, 2.0, 1.0];
        let rects = squarify(&sizes, rect);
        assert_eq!(rects.len(), sizes.len());

        let total: f64 = sizes.iter().sum();
        for (r, size) in rects.iter().zip(sizes) {
            let expected = 600.0 * 400.0 * size / total;
            assert!((r.area() as f64 - expected).abs() < 1.0);
            assert!(rect.expand(0.01).contains_rect(*r));
        }
    }
}
