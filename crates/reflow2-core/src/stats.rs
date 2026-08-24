//! Basic statistics over numeric slices — the two functions `dimensions.rs` needs.
//!
//! # Provenance
//!
//! ABSORBED FROM `dynograph-foundation`, and the exact source is recorded here
//! because a later reader has no other way to find it:
//!
//! ```text
//! repo     https://github.com/sligara7/dynograph-foundation
//! tag      v0.12.0
//! commit   19b676033a668b4563e50a757b2700e921322016   (2026-06-07)
//! file     crates/dynograph-vector/src/stats.rs
//! taken    linear_regression_slope, mean, and their tests — verbatim
//! licence  MIT, Copyright (c) 2026 Anthony Sligar
//! ```
//!
//! 🛑 WHY THE HEADER IS A REQUIREMENT RATHER THAN A COURTESY. The recorded
//! objection to absorbing anything (`dec:absorb-the-foundation-subset-and-end-the-dependency`)
//! is that **vendoring converts a visible dependency into an invisible one**:
//! the pin in `reflow2.toml` carried a written reason for every version bump,
//! and once the code is in-tree that record has no successor. This block is the
//! successor. An absorbed file that does not say where it came from is the cost
//! the objection predicted, arriving quietly.
//!
//! # What was deliberately NOT taken
//!
//! `stats.rs` upstream exports nine functions; reflow2 calls two. The other
//! seven — `pearson_correlation`, `variance`, `std_dev`, `percentile`,
//! `median`, `softmax`, `spearman_rank_correlation` — are not here, nor is any
//! of `hnsw.rs` (1,165 lines) or `distance.rs` (776). Taking them would be
//! exactly the dead weight the decision exists to avoid: measured, reflow2 used
//! 436 of `dynograph-vector`'s 2,377 lines, and of the 436 it calls two
//! functions. They remain in the upstream repository if ever wanted.
//!
//! # Why this module is private
//!
//! `pub(crate)`, not `pub`. `ifc:core-api` already records that
//! `reflow2_core::DesignGraph` exposes **277 public functions and grows by
//! default**, and calls that unenumerable — "a contract whose scope grows by
//! default is a label on a moving target". Absorbing code is not a reason to
//! widen a surface already recorded as too wide, so these stay inside the crate.

/// Ordinary-least-squares slope of `y` regressed on `x` for a sequence of
/// `(x, y)` points.
///
/// Returns `None` if fewer than 2 points are supplied or the `x` values have
/// zero variance (vertical line — slope is undefined). Degenerate input yields
/// `None` rather than `0.0` **so the caller has to make the fallback choice
/// explicit** — that is upstream's design and it is kept, because a silent
/// zero slope reads as "no trend" when it means "no answer".
#[inline]
pub(crate) fn linear_regression_slope(points: &[(f64, f64)]) -> Option<f64> {
    if points.len() < 2 {
        return None;
    }
    let n = points.len() as f64;
    let sum_x: f64 = points.iter().map(|(x, _)| x).sum();
    let sum_y: f64 = points.iter().map(|(_, y)| y).sum();
    let sum_xy: f64 = points.iter().map(|(x, y)| x * y).sum();
    let sum_xx: f64 = points.iter().map(|(x, _)| x * x).sum();

    let denom = n * sum_xx - sum_x * sum_x;
    if denom.abs() < 1e-12 {
        return None;
    }
    Some((n * sum_xy - sum_x * sum_y) / denom)
}

/// Arithmetic mean. `None` for an empty slice — same rule as above: an empty
/// input has no mean, and `0.0` would be an answer to a different question.
#[inline]
pub(crate) fn mean(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    Some(xs.iter().sum::<f64>() / xs.len() as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Upstream's tests, taken verbatim with the functions. They come along
    // BECAUSE they came along: absorbing code without its tests would mean
    // reflow2 now owns behaviour nothing checks, which is a worse position than
    // the dependency it replaced.

    #[test]
    fn linreg_perfect_positive() {
        let pts = vec![(0.0, 0.0), (1.0, 2.0), (2.0, 4.0), (3.0, 6.0)];
        let slope = linear_regression_slope(&pts).unwrap();
        assert!((slope - 2.0).abs() < 1e-12);
    }

    #[test]
    fn linreg_perfect_negative() {
        let pts = vec![(0.0, 10.0), (1.0, 8.0), (2.0, 6.0), (3.0, 4.0)];
        let slope = linear_regression_slope(&pts).unwrap();
        assert!((slope - (-2.0)).abs() < 1e-12);
    }

    #[test]
    fn linreg_horizontal() {
        let pts = vec![(0.0, 5.0), (1.0, 5.0), (2.0, 5.0)];
        let slope = linear_regression_slope(&pts).unwrap();
        assert!(slope.abs() < 1e-12);
    }

    #[test]
    fn linreg_with_noise() {
        // y = 3x + 1 + small noise; OLS should recover ~3
        let pts = vec![
            (1.0, 4.1),
            (2.0, 6.9),
            (3.0, 10.2),
            (4.0, 12.8),
            (5.0, 16.1),
        ];
        let slope = linear_regression_slope(&pts).unwrap();
        assert!((slope - 3.0).abs() < 0.2);
    }

    #[test]
    fn linreg_too_few_points() {
        assert_eq!(linear_regression_slope(&[]), None);
        assert_eq!(linear_regression_slope(&[(1.0, 2.0)]), None);
    }

    #[test]
    fn linreg_zero_x_variance() {
        // All x values identical — vertical line, slope undefined.
        let pts = vec![(1.0, 0.0), (1.0, 5.0), (1.0, 10.0)];
        assert_eq!(linear_regression_slope(&pts), None);
    }

    #[test]
    fn mean_basic_and_empty() {
        assert_eq!(mean(&[1.0, 2.0, 3.0, 4.0]), Some(2.5));
        assert_eq!(mean(&[]), None);
    }
}
