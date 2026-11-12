//! Column widths for tables that always fill their width exactly.
//!
//! Each column has a preferred width and a minimum. Laying out for a given total width:
//! the *flexible* columns (title, artist) share whatever the fixed-size columns leave,
//! in proportion to their preferred widths; if even their minimums don't fit, the other
//! columns shrink too. Dragging a divider moves width between the two columns on either
//! side, so the total never changes and the table never overflows or leaves a gap.

#[derive(Debug, Clone, PartialEq)]
pub struct FitColumns {
    /// Preferred widths (from the defaults, or the last time the user resized).
    widths: Vec<f32>,
    mins: Vec<f32>,
    /// Columns that take up extra space when the table grows.
    flex: Vec<bool>,
}

impl FitColumns {
    /// `columns` is `(preferred width, minimum width, flexible)` per column.
    pub fn new(columns: &[(f32, f32, bool)]) -> Self {
        Self {
            widths: columns.iter().map(|c| c.0.max(c.1)).collect(),
            mins: columns.iter().map(|c| c.1).collect(),
            flex: columns.iter().map(|c| c.2).collect(),
        }
    }

    /// The preferred widths, for saving.
    pub fn preferred(&self) -> &[f32] {
        &self.widths
    }

    /// Restore saved preferred widths (ignored if the column count differs).
    pub fn set_preferred(&mut self, widths: &[f32]) {
        if widths.len() == self.widths.len() && widths.iter().all(|w| w.is_finite()) {
            for (w, (&saved, &min)) in self.widths.iter_mut().zip(widths.iter().zip(&self.mins)) {
                *w = saved.max(min);
            }
        }
    }

    /// Widths that add up to exactly `total` (unless even the minimums don't fit, when
    /// every column is at its minimum).
    pub fn fit(&self, total: f32) -> Vec<f32> {
        let n = self.widths.len();
        let fixed: Vec<usize> = (0..n).filter(|&i| !self.flex[i]).collect();
        let flex: Vec<usize> = (0..n).filter(|&i| self.flex[i]).collect();
        let fixed_sum: f32 = fixed.iter().map(|&i| self.widths[i]).sum();
        let flex_min: f32 = flex.iter().map(|&i| self.mins[i]).sum();
        let mut out = self.widths.clone();
        if flex.is_empty() {
            self.distribute(&mut out, &fixed, total);
        } else if total - fixed_sum >= flex_min {
            self.distribute(&mut out, &flex, total - fixed_sum);
        } else {
            // Too narrow: flexible columns at their minimum, the rest share what's left.
            for &i in &flex {
                out[i] = self.mins[i];
            }
            self.distribute(&mut out, &fixed, total - flex_min);
        }
        out
    }

    /// Share `target` between `cols` in proportion to their preferred widths, never
    /// going below a minimum (a column that would is pinned there and the rest re-shared).
    fn distribute(&self, out: &mut [f32], cols: &[usize], target: f32) {
        let mut free: Vec<usize> = cols.to_vec();
        let mut room = target;
        loop {
            let weight: f32 = free.iter().map(|&i| self.widths[i]).sum();
            if free.is_empty() || weight <= 0.0 {
                return;
            }
            let scale = room.max(0.0) / weight;
            let pinned: Vec<usize> = free
                .iter()
                .copied()
                .filter(|&i| self.widths[i] * scale < self.mins[i])
                .collect();
            if pinned.is_empty() {
                for &i in &free {
                    out[i] = self.widths[i] * scale;
                }
                return;
            }
            for &i in &pinned {
                out[i] = self.mins[i];
                room -= self.mins[i];
            }
            free.retain(|i| !pinned.contains(i));
        }
    }

    /// Move the divider between column `i` and `i + 1` by `dx` points, as laid out at
    /// `total`. The new layout becomes the preferred widths.
    pub fn drag(&mut self, i: usize, dx: f32, total: f32) {
        if i + 1 >= self.widths.len() {
            return;
        }
        let mut w = self.fit(total);
        // Neither neighbour may go below its minimum.
        let dx = dx.clamp(self.mins[i] - w[i], w[i + 1] - self.mins[i + 1]);
        if dx.is_nan() || self.mins[i] - w[i] > w[i + 1] - self.mins[i + 1] {
            return;
        }
        w[i] += dx;
        w[i + 1] -= dx;
        self.widths = w;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cols() -> FitColumns {
        // Title and artist flex; three fixed columns.
        FitColumns::new(&[
            (300.0, 100.0, true),
            (200.0, 60.0, true),
            (60.0, 40.0, false),
            (50.0, 40.0, false),
            (60.0, 40.0, false),
        ])
    }

    fn sum(v: &[f32]) -> f32 {
        v.iter().sum()
    }

    #[test]
    fn always_fills_the_width() {
        let c = cols();
        for total in [400.0, 670.0, 900.0, 2000.0] {
            assert!((sum(&c.fit(total)) - total).abs() < 0.01, "{total}");
        }
    }

    #[test]
    fn extra_width_goes_to_the_flexible_columns_in_proportion() {
        let w = cols().fit(1170.0); // 500 more than preferred
        assert!(
            (w[0] - 600.0).abs() < 0.01 && (w[1] - 400.0).abs() < 0.01,
            "{w:?}"
        );
        assert_eq!(&w[2..], &[60.0, 50.0, 60.0]);
    }

    #[test]
    fn narrow_tables_respect_minimums() {
        let c = cols();
        let w = c.fit(400.0);
        assert!(w.iter().zip(&c.mins).all(|(w, m)| w >= m), "{w:?}");
        assert!((sum(&w) - 400.0).abs() < 0.01);
        // Below the sum of the minimums, everything sits at its minimum.
        assert_eq!(c.fit(100.0), c.mins);
    }

    #[test]
    fn dragging_a_divider_moves_width_between_neighbours() {
        let mut c = cols();
        c.drag(0, 50.0, 670.0);
        let w = c.fit(670.0);
        assert_eq!(w, vec![350.0, 150.0, 60.0, 50.0, 60.0]);
        // A fixed column can be widened at its neighbour's expense (here only by 10, as
        // the key column stops at its minimum), and keeps that width when the table grows.
        c.drag(2, 20.0, 670.0);
        assert_eq!(c.fit(670.0), vec![350.0, 150.0, 70.0, 40.0, 60.0]);
        assert_eq!(c.fit(1000.0)[2..], [70.0, 40.0, 60.0]);
    }

    #[test]
    fn dragging_stops_at_minimums() {
        let mut c = cols();
        c.drag(1, 500.0, 670.0); // artist would take everything from BPM
        let w = c.fit(670.0);
        assert_eq!(w[2], 40.0);
        assert!((sum(&w) - 670.0).abs() < 0.01);
    }

    #[test]
    fn saved_widths_are_restored_but_never_below_minimums() {
        let mut c = cols();
        c.set_preferred(&[400.0, 10.0, 70.0, 50.0, 60.0]);
        assert_eq!(c.preferred(), &[400.0, 60.0, 70.0, 50.0, 60.0]);
        c.set_preferred(&[1.0, 2.0]); // wrong length: ignored
        assert_eq!(c.preferred()[0], 400.0);
    }
}
