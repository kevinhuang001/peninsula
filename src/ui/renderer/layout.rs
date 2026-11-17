use super::{point, Renderer, MAX_DEPTH};
use crate::ui::declarative::*;
use eframe::egui::{pos2, vec2, Pos2, Rect, Ui, Vec2};

const MAX_TRACKS: usize = 128;
const MAX_CHILDREN: usize = 512;

#[derive(Default)]
pub(super) struct Layout {
    pub size: Vec2,
    pub cells: Vec<(usize, Rect)>,
}

impl Layout {
    fn push(&mut self, index: usize, at: Pos2, size: Vec2) {
        self.cells.push((index, Rect::from_min_size(at, size)));
    }
}

pub(super) const TABLE_HEADER_HEIGHT: f32 = 29.0;
pub(super) const TABLE_X_PADDING: f32 = 10.0;
pub(super) const TABLE_Y_PADDING: f32 = 6.0;
pub(super) const TABLE_GAP: f32 = 10.0;

pub(super) struct TableLayout {
    pub columns: Vec<f32>,
    pub rows: Vec<f32>,
}

fn table_overhead(column_count: usize) -> f32 {
    2.0 * TABLE_X_PADDING + TABLE_GAP * column_count.saturating_sub(1) as f32
}

fn column_weight(column: &TableColumn) -> f32 {
    point(column.weight).max(0.01)
}

impl TableLayout {
    fn height(&self) -> f32 {
        TABLE_HEADER_HEIGHT + self.rows.iter().sum::<f32>()
    }
}

pub(super) fn inset(rect: Rect, [left, top, right, bottom]: [f32; 4]) -> Rect {
    let min = (rect.min + vec2(point(left), point(top))).min(rect.max);
    Rect::from_min_max(min, (rect.max - vec2(point(right), point(bottom))).max(min))
}

struct Axis {
    mode: Size,
    min: f32,
    max: f32,
}

impl Axis {
    fn new(mode: Size, min: Option<f32>, max: Option<f32>) -> Self {
        let (floor, ceiling) = match mode {
            Size::MinMax { min, max } => (point(min), point(max)),
            _ => (0.0, super::MAX_EXTENT),
        };
        let min = min.map_or(floor, |value| floor.max(point(value)));
        let max = max
            .map_or(ceiling, |value| ceiling.min(point(value)))
            .max(min);
        Self { mode, min, max }
    }

    fn definite(&self, available: f32) -> Option<f32> {
        match self.mode {
            Size::Fixed(value) => Some(self.bound(value)),
            Size::Percent(value) => Some(self.bound(available * point(value).min(1.0))),
            Size::Auto | Size::Fill(_) | Size::MinMax { .. } => None,
        }
    }

    fn bound(&self, value: f32) -> f32 {
        point(value).clamp(self.min, self.max)
    }
}

fn stretch_cross(axis: &Axis, align: Align) -> bool {
    matches!(axis.mode, Size::Fill(_))
        || (align == Align::Stretch && matches!(axis.mode, Size::Auto | Size::MinMax { .. }))
}

fn axes(size: SizeSpec) -> [Axis; 2] {
    [
        Axis::new(size.width, size.min_width, size.max_width),
        Axis::new(size.height, size.min_height, size.max_height),
    ]
}

pub(super) fn offset(align: Align, spare: f32) -> f32 {
    let spare = spare.max(0.0);
    match align {
        Align::Start | Align::Stretch => 0.0,
        Align::Center => spare * 0.5,
        Align::End => spare,
    }
}

pub(super) fn scrolls(node: &ViewNode, axis: usize, depth: usize, remaining: &mut usize) -> bool {
    if depth == MAX_DEPTH || *remaining == 0 {
        return false;
    }
    *remaining -= 1;
    match &node.kind {
        Node::Scroll(axes, child) => {
            axes.axes()[axis] || scrolls(child, axis, depth + 1, remaining)
        }
        Node::Container(child) | Node::ActionArea { content: child, .. } => {
            scrolls(child, axis, depth + 1, remaining)
        }
        Node::Table(table) => table
            .rows
            .iter()
            .take(MAX_CHILDREN)
            .flat_map(|row| row.iter().take(table.columns.len().min(MAX_TRACKS)))
            .any(|child| scrolls(child, axis, depth + 1, remaining)),
        Node::Row(_, children)
        | Node::Column(_, children)
        | Node::Wrap(_, children)
        | Node::Grid(_, children)
        | Node::Stack(_, children) => children
            .iter()
            .take(MAX_CHILDREN)
            .any(|child| scrolls(child, axis, depth + 1, remaining)),
        _ => false,
    }
}

impl Renderer<'_> {
    /// 'available' is the reference for percentages. 'forced' is an already
    /// resolved allocation, e.g. a weighted share or a stretched grid cell.
    pub(super) fn measure(
        &mut self,
        ui: &Ui,
        node: &ViewNode,
        available: Vec2,
        forced: [Option<f32>; 2],
        depth: usize,
    ) -> Vec2 {
        if depth >= MAX_DEPTH {
            return Vec2::ZERO;
        }
        let key = (
            node as *const ViewNode,
            available.x.to_bits(),
            available.y.to_bits(),
            forced.map(|value| value.map(f32::to_bits)),
            depth,
        );
        if let Some(size) = self.measured.get(&key) {
            return *size;
        }
        if self.measure_remaining == 0 {
            return Vec2::ZERO;
        }
        self.measure_remaining -= 1;
        let axes = axes(node.size);
        let mut explicit = [
            forced[0]
                .map(|v| axes[0].bound(v))
                .or_else(|| axes[0].definite(available.x)),
            forced[1]
                .map(|v| axes[1].bound(v))
                .or_else(|| axes[1].definite(available.y)),
        ];
        let ratio = node.size.aspect_ratio.filter(|r| r.is_finite() && *r > 0.0);
        if let Some(ratio) = ratio {
            match explicit {
                [Some(w), None] => explicit[1] = Some(axes[1].bound(w / ratio)),
                [None, Some(h)] => explicit[0] = Some(axes[0].bound(h * ratio)),
                [Some(w), Some(h)] if forced[0].is_some() || forced[1].is_some() => {
                    // Keep fixed dimensions authoritative; otherwise fit the ratio in the cell.
                    if forced[0].is_none() || (forced[1].is_some() && w / ratio <= h) {
                        explicit[1] = Some(axes[1].bound(w / ratio));
                    } else {
                        explicit[0] = Some(axes[0].bound(h * ratio));
                    }
                }
                _ => {}
            }
        }
        if let [Some(w), Some(h)] = explicit {
            let size = vec2(w, h);
            self.measured.insert(key, size);
            return size;
        }
        let available = vec2(
            explicit[0].unwrap_or_else(|| axes[0].bound(available.x)),
            explicit[1].unwrap_or_else(|| axes[1].bound(available.y)),
        );
        let padding = node.style.padding.map(point);
        let extras = vec2(padding[0] + padding[2], padding[1] + padding[3]);
        let inner = (available - extras).max(Vec2::ZERO);
        let native = match &node.kind {
            Node::Table(table) => {
                let width = if explicit[0].is_some() {
                    inner.x
                } else {
                    self.table_natural_width(ui, table, inner.x, depth + 1)
                };
                vec2(width, self.table(ui, table, width, depth + 1).height())
            }
            _ => self
                .layout(ui, node, inner, depth + 1)
                .map_or_else(|| self.leaf_size(ui, node, inner), |layout| layout.size),
        };
        let mut size = native + extras;
        if let Some(ratio) = ratio {
            if explicit == [None, None] {
                size.y = size.y.max(size.x / ratio);
                size.x = size.y * ratio;
            }
        }
        let size = vec2(
            explicit[0].unwrap_or_else(|| axes[0].bound(size.x)),
            explicit[1].unwrap_or_else(|| axes[1].bound(size.y)),
        );
        self.measured.insert(key, size);
        size
    }

    fn table_natural_width(&mut self, ui: &Ui, table: &Table, limit: f32, depth: usize) -> f32 {
        let columns = &table.columns[..table.columns.len().min(MAX_TRACKS)];
        let overhead = table_overhead(columns.len());
        if limit <= overhead {
            return limit;
        }
        let total = columns.iter().map(column_weight).sum::<f32>();
        let available = limit - overhead;
        let mut required: f32 = 0.0;
        for (index, column) in columns.iter().enumerate() {
            let share = column_weight(column) / total;
            let column_limit = available * share;
            let mut widest = super::paint::table_header_width(ui, &column.title);
            for cell in table
                .rows
                .iter()
                .take(MAX_CHILDREN)
                .filter_map(|row| row.get(index))
            {
                let size = self.measure(ui, cell, vec2(super::MAX_EXTENT, 0.0), [None; 2], depth);
                widest = widest.max(size.x);
                if widest >= column_limit {
                    return limit;
                }
            }
            required = required.max(widest / share);
        }
        (required.ceil() + overhead).min(limit)
    }

    pub(super) fn table(
        &mut self,
        ui: &Ui,
        table: &Table,
        width: f32,
        depth: usize,
    ) -> TableLayout {
        let columns = &table.columns[..table.columns.len().min(MAX_TRACKS)];
        let available = (width - table_overhead(columns.len())).max(0.0);
        let weights: Vec<_> = columns.iter().map(column_weight).collect();
        let total: f32 = weights.iter().sum();
        let columns: Vec<_> = weights
            .into_iter()
            .map(|weight| available * weight / total)
            .collect();
        let rows = table
            .rows
            .iter()
            .take(MAX_CHILDREN)
            .map(|row| {
                row.iter()
                    .zip(&columns)
                    .map(|(cell, &width)| {
                        self.in_cell(
                            ui,
                            cell,
                            vec2(width, 0.0),
                            [Align::Stretch, Align::Start],
                            depth,
                        )
                        .y
                    })
                    .fold(15.0, f32::max)
                    + 2.0 * TABLE_Y_PADDING
            })
            .collect();
        TableLayout { columns, rows }
    }

    pub(super) fn layout(
        &mut self,
        ui: &Ui,
        node: &ViewNode,
        available: Vec2,
        depth: usize,
    ) -> Option<Layout> {
        if depth > MAX_DEPTH {
            return Some(Layout::default());
        }
        Some(match &node.kind {
            Node::Row(flow, children) => self.flow(ui, children, *flow, available, true, depth),
            Node::Column(flow, children) => self.flow(ui, children, *flow, available, false, depth),
            Node::Wrap(flow, children) => self.flow(
                ui,
                children,
                Flow {
                    wrap: true,
                    ..*flow
                },
                available,
                true,
                depth,
            ),
            Node::Grid(grid, children) => self.grid(ui, grid, children, available, depth),
            Node::Stack(align, children) => {
                let mut result = Layout::default();
                for (index, child) in children.iter().take(MAX_CHILDREN).enumerate() {
                    let size = self.in_cell(ui, child, available, *align, depth);
                    result.size = result.size.max(size);
                    result.push(
                        index,
                        pos2(
                            offset(align[0], available.x - size.x),
                            offset(align[1], available.y - size.y),
                        ),
                        size,
                    );
                }
                result
            }
            Node::Container(child) | Node::ActionArea { content: child, .. } => {
                let size = self.in_cell(ui, child, available, [Align::Start; 2], depth);
                let mut result = Layout {
                    size,
                    ..Layout::default()
                };
                result.push(0, Pos2::ZERO, size);
                result
            }
            Node::Scroll(_, child) => {
                let size = self
                    .in_cell(ui, child, available, [Align::Start; 2], depth)
                    .min(available);
                Layout {
                    size,
                    ..Layout::default()
                }
            }
            _ => return None,
        })
    }

    pub(super) fn overflow_extent(
        &mut self,
        ui: &Ui,
        node: &ViewNode,
        size: Vec2,
        depth: usize,
        budget: &mut usize,
    ) -> Vec2 {
        if *budget == 0 || depth >= MAX_DEPTH || node.clip || matches!(node.kind, Node::Scroll(..))
        {
            return size;
        }
        *budget -= 1;
        let Some(children) = children(node) else {
            return size;
        };
        let inner = inset(Rect::from_min_size(Pos2::ZERO, size), node.style.padding);
        let Some(layout) = self.layout(ui, node, inner.size(), depth + 1) else {
            return size;
        };
        let trailing = size - inner.max.to_vec2();
        layout
            .cells
            .into_iter()
            .fold(size, |extent, (index, rect)| {
                extent.max(
                    inner.min.to_vec2()
                        + rect.min.to_vec2()
                        + self.overflow_extent(
                            ui,
                            &children[index],
                            rect.size(),
                            depth + 1,
                            budget,
                        )
                        + trailing,
                )
            })
    }

    pub(super) fn in_cell(
        &mut self,
        ui: &Ui,
        child: &ViewNode,
        available: Vec2,
        alignment: [Align; 2],
        depth: usize,
    ) -> Vec2 {
        let modes = [child.size.width, child.size.height];
        let mut forced = [None; 2];
        for axis in 0..2 {
            if axis == 0 && self.intrinsic_width {
                continue;
            }
            if matches!(modes[axis], Size::Fill(_))
                || (alignment[axis] == Align::Stretch
                    && matches!(modes[axis], Size::Auto | Size::MinMax { .. }))
            {
                forced[axis] = Some(available[axis]);
            }
        }
        self.measure(ui, child, available, forced, depth)
    }

    fn flow(
        &mut self,
        ui: &Ui,
        children: &[ViewNode],
        flow: Flow,
        available: Vec2,
        horizontal: bool,
        depth: usize,
    ) -> Layout {
        let children = &children[..children.len().min(MAX_CHILDREN)];
        let main = usize::from(!horizontal);
        let cross = 1 - main;
        let gap = point(flow.gap.unwrap_or(self.theme.gap));
        let line_gap = point(flow.line_gap.unwrap_or(gap));
        let mut lengths = Vec::with_capacity(children.len());
        let mut weights = Vec::with_capacity(children.len());
        let mut lines = vec![0];
        let mut occupied = 0.0;
        for (index, child) in children.iter().enumerate() {
            let axes = axes(child.size);
            let weight = match axes[main].mode {
                Size::Fill(weight) if !(horizontal && self.intrinsic_width) => point(weight),
                _ => 0.0,
            };
            let mut force = [None; 2];
            if weight > 0.0 {
                force[main] = Some(axes[main].min);
            }
            if !(cross == 0 && self.intrinsic_width)
                && !flow.wrap
                && stretch_cross(&axes[cross], flow.align)
            {
                force[cross] = Some(available[cross]);
            }
            let basis = self.measure(ui, child, available, force, depth)[main];
            if flow.wrap
                && index > *lines.last().unwrap()
                && occupied + gap + basis > available[main]
            {
                lines.push(index);
                occupied = 0.0;
            }
            if index > *lines.last().unwrap() {
                occupied += gap;
            }
            occupied += basis;
            lengths.push(basis);
            weights.push(weight);
        }
        lines.push(children.len());
        let mut result = Layout::default();
        let mut cross_at = 0.0;
        for (line_index, line) in lines.windows(2).enumerate() {
            let (start, end) = (line[0], line[1]);
            if start == end {
                continue;
            }
            let count = end - start;
            let base = lengths[start..end].iter().sum::<f32>() + gap * (count - 1) as f32;
            let limits: Vec<_> = children[start..end]
                .iter()
                .map(|child| axes(child.size)[main].max)
                .collect();
            distribute(
                &mut lengths[start..end],
                &weights[start..end],
                &limits,
                available[main] - base,
            );
            let mut sizes = Vec::with_capacity(count);
            for index in start..end {
                let child = &children[index];
                let mut force = [None; 2];
                if weights[index] > 0.0 {
                    force[main] = Some(lengths[index]);
                }
                if !(cross == 0 && self.intrinsic_width)
                    && !flow.wrap
                    && stretch_cross(&axes(child.size)[cross], flow.align)
                {
                    force[cross] = Some(available[cross]);
                }
                let size = self.measure(ui, child, available, force, depth);
                sizes.push(size);
            }
            let cross_used = sizes.iter().map(|size| size[cross]).fold(0.0, f32::max);
            let line_cross = if flow.wrap {
                cross_used
            } else {
                available[cross].max(cross_used)
            };
            let used = sizes.iter().map(|size| size[main]).sum::<f32>() + gap * (count - 1) as f32;
            let free = (available[main] - used).max(0.0);
            let (mut main_at, gap) = match flow.justify {
                Justify::Start => (0.0, gap),
                Justify::Center => (free * 0.5, gap),
                Justify::End => (free, gap),
                Justify::SpaceBetween => (0.0, gap + free / (count - 1).max(1) as f32),
            };
            if line_index > 0 {
                cross_at += line_gap;
            }
            for (relative, mut size) in sizes.into_iter().enumerate() {
                let index = start + relative;
                let mut at = Pos2::ZERO;
                at[main] = main_at;
                if !(cross == 0 && self.intrinsic_width)
                    && flow.wrap
                    && stretch_cross(&axes(children[index].size)[cross], flow.align)
                {
                    let bounded = axes(children[index].size)[cross].bound(line_cross);
                    // The line determines this allocation after its natural height is known.
                    // Keep the measured main-axis allocation; it determined wrapping.
                    size[cross] = children[index]
                        .size
                        .aspect_ratio
                        .filter(|ratio| ratio.is_finite() && *ratio > 0.0)
                        .map_or(bounded, |_| size[cross]);
                }
                at[cross] = cross_at + offset(flow.align, line_cross - size[cross]);
                result.push(index, at, size);
                main_at += size[main] + gap;
            }
            cross_at += cross_used;
            result.size[main] = result.size[main].max(used);
        }
        result.size[cross] = cross_at;
        result
    }

    fn grid(
        &mut self,
        ui: &Ui,
        grid: &Grid,
        children: &[ViewNode],
        available: Vec2,
        depth: usize,
    ) -> Layout {
        let gx = point(grid.column_gap.unwrap_or(self.theme.gap));
        let gy = point(grid.row_gap.unwrap_or(self.theme.gap));
        let columns = match &grid.columns {
            Columns::Tracks(tracks) if !tracks.is_empty() => {
                tracks[..tracks.len().min(MAX_TRACKS)].to_vec()
            }
            Columns::Tracks(_) => vec![Track::Auto],
            Columns::AutoFit {
                min_item_width,
                max_columns,
            } => {
                let min = point(*min_item_width).max(1.0);
                let n = (((available.x + gx) / (min + gx)).floor() as usize)
                    .clamp(1, (*max_columns).clamp(1, MAX_TRACKS));
                vec![Track::Fraction(1.0); n]
            }
        };
        let children = &children[..children.len().min(MAX_CHILDREN)];
        let positions = place(children, columns.len(), grid.rows.len(), grid.auto_flow);
        let rows = positions
            .iter()
            .flatten()
            .map(|cell| cell.y + cell.h)
            .max()
            .unwrap_or(0)
            .max(grid.rows.len().min(MAX_TRACKS));
        let y_tracks: Vec<_> = (0..rows)
            .map(|i| grid.rows.get(i).copied().unwrap_or(Track::Auto))
            .collect();
        let mut intrinsic = vec![Vec2::ZERO; children.len()];
        for (index, cell) in positions.iter().enumerate() {
            if let Some(cell) = cell {
                // Measure intrinsic tracks against the host width. An equal
                // share would truncate an Auto column beside a narrow Fixed one.
                let width = if matches!(children[index].size.width, Size::Percent(_)) {
                    available.x * cell.w as f32 / columns.len() as f32
                } else {
                    available.x
                };
                intrinsic[index] = self.measure(
                    ui,
                    &children[index],
                    vec2(width, available.y),
                    [None; 2],
                    depth,
                );
            }
        }
        let widths = tracks(
            &columns,
            available.x,
            gx,
            positions
                .iter()
                .zip(&intrinsic)
                .filter_map(|(cell, size)| cell.map(|c| (c.x, c.w, size.x))),
        );
        for (index, cell) in positions.iter().enumerate() {
            if let Some(cell) = cell {
                let width = extent(&widths, cell.x, cell.w, gx);
                let child = &children[index];
                let forced = match child.size.width {
                    Size::Auto | Size::Fill(_) | Size::MinMax { .. } => Some(width),
                    _ => None,
                };
                intrinsic[index] =
                    self.measure(ui, child, vec2(width, available.y), [forced, None], depth);
            }
        }
        let heights = tracks(
            &y_tracks,
            available.y,
            gy,
            positions
                .iter()
                .zip(&intrinsic)
                .filter_map(|(cell, size)| cell.map(|c| (c.y, c.h, size.y))),
        );
        let mut result = Layout {
            size: vec2(
                extent(&widths, 0, widths.len(), gx),
                extent(&heights, 0, heights.len(), gy),
            ),
            ..Layout::default()
        };
        for (index, cell) in positions.into_iter().enumerate() {
            let Some(cell) = cell else {
                continue;
            };
            let available = vec2(
                extent(&widths, cell.x, cell.w, gx),
                extent(&heights, cell.y, cell.h, gy),
            );
            let size = self.in_cell(ui, &children[index], available, [Align::Stretch; 2], depth);
            result.push(
                index,
                pos2(
                    extent(&widths, 0, cell.x, gx) + if cell.x > 0 { gx } else { 0.0 },
                    extent(&heights, 0, cell.y, gy) + if cell.y > 0 { gy } else { 0.0 },
                ),
                size,
            );
        }
        result
    }
}

pub(super) fn children(node: &ViewNode) -> Option<&[ViewNode]> {
    match &node.kind {
        Node::Row(_, children)
        | Node::Column(_, children)
        | Node::Wrap(_, children)
        | Node::Grid(_, children)
        | Node::Stack(_, children) => Some(children),
        Node::Container(child) | Node::ActionArea { content: child, .. } => {
            Some(std::slice::from_ref(child.as_ref()))
        }
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Cell {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

/// Reserve explicitly placed cells first. Conflicting positions fall back to
/// auto placement, so a responsive grid cannot silently hide a child.
fn place(
    children: &[ViewNode],
    columns: usize,
    explicit_rows: usize,
    flow: AutoFlow,
) -> Vec<Option<Cell>> {
    let rows = MAX_TRACKS.max(children.len());
    let mut occupied = vec![false; columns * rows];
    let mut placed = vec![None; children.len()];
    let area = |c: Cell| {
        (c.y..c.y + c.h).flat_map(move |y| (c.x..c.x + c.w).map(move |x| y * columns + x))
    };
    let mut dense_rows = explicit_rows
        .clamp(1, rows)
        .max(children.len().div_ceil(columns));
    for child in children {
        let p = child.placement;
        dense_rows = dense_rows.max(
            p.row
                .unwrap_or(0)
                .min(rows - 1)
                .saturating_add(p.row_span.clamp(1, rows))
                .min(rows),
        );
    }
    for priority in (0..=2).rev() {
        for (index, child) in children.iter().enumerate() {
            let p = child.placement;
            let explicit = usize::from(p.column.is_some()) + usize::from(p.row.is_some());
            if placed[index].is_some() || (priority > 0 && priority != explicit) {
                continue;
            }
            let w = p.column_span.clamp(1, columns);
            let h = p.row_span.clamp(1, rows);
            let x = p.column.map(|x| x.min(columns - w));
            let y = p.row.map(|y| y.min(rows - h));
            let mut try_cell = |x, y| {
                let c = Cell { x, y, w, h };
                if x + w > columns || y + h > rows || area(c).any(|i| occupied[i]) {
                    return false;
                }
                for i in area(c) {
                    occupied[i] = true;
                }
                placed[index] = Some(c);
                true
            };
            if priority == 2 {
                try_cell(x.unwrap(), y.unwrap());
            } else if priority == 1 {
                if let Some(x) = x {
                    (0..rows).any(|y| try_cell(x, y));
                }
                if let Some(y) = y {
                    (0..columns).any(|x| try_cell(x, y));
                }
            } else if flow == AutoFlow::Column {
                let _ = (0..columns)
                    .any(|x| (0..dense_rows.saturating_sub(h - 1)).any(|y| try_cell(x, y)))
                    || (dense_rows.saturating_sub(h - 1)..rows)
                        .any(|y| (0..columns).any(|x| try_cell(x, y)));
            } else {
                (0..rows).any(|y| (0..columns).any(|x| try_cell(x, y)));
            }
        }
    }
    placed
}

fn extent(tracks: &[f32], start: usize, count: usize, gap: f32) -> f32 {
    tracks[start..start + count].iter().sum::<f32>() + count.saturating_sub(1) as f32 * gap
}

fn tracks(
    tracks: &[Track],
    available: f32,
    gap: f32,
    requirements: impl Iterator<Item = (usize, usize, f32)>,
) -> Vec<f32> {
    let mut sizes: Vec<_> = tracks
        .iter()
        .map(|t| match *t {
            Track::Fixed(value) | Track::MinMax { min: value, .. } => point(value),
            _ => 0.0,
        })
        .collect();
    let limits: Vec<_> = tracks
        .iter()
        .zip(&sizes)
        .map(|(t, base)| match *t {
            Track::Fixed(_) => *base,
            Track::MinMax { max, .. } => point(max).max(*base),
            _ => super::MAX_EXTENT,
        })
        .collect();
    let weights: Vec<_> = tracks
        .iter()
        .map(|t| match *t {
            Track::Fraction(w) => point(w),
            _ => 0.0,
        })
        .collect();
    let mut needs: Vec<_> = requirements.collect();
    needs.sort_by_key(|&(_, span, _)| span);
    for (start, span, size) in needs {
        let end = start + span;
        let deficit = size - extent(&sizes, start, span, gap);
        let intrinsic: Vec<_> = tracks[start..end]
            .iter()
            .map(|t| {
                if matches!(t, Track::Auto | Track::MinMax { .. }) {
                    1.0
                } else {
                    0.0
                }
            })
            .collect();
        distribute(
            &mut sizes[start..end],
            &intrinsic,
            &limits[start..end],
            deficit,
        );
    }
    let free = available - extent(&sizes, 0, sizes.len(), gap);
    distribute(&mut sizes, &weights, &limits, free);
    sizes
}

fn distribute(sizes: &mut [f32], weights: &[f32], limits: &[f32], mut free: f32) {
    // Redistribute unused space when a weighted item hits a maximum.
    while free > 0.01 {
        let sum: f32 = (0..sizes.len())
            .filter(|&i| limits[i] - sizes[i] > 0.001)
            .map(|i| weights[i])
            .sum();
        if sum <= 0.0 {
            break;
        }
        let mut used = 0.0;
        for i in 0..sizes.len() {
            let amount = (free * weights[i] / sum).min(limits[i] - sizes[i]).max(0.0);
            sizes[i] += amount;
            used += amount;
        }
        if used <= 0.001 {
            break;
        }
        free -= used;
    }
}
