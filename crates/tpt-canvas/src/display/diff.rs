//! Compares two display lists op by op.
//!
//! The diff is the longest common subsequence of the two op sequences. Ops that match keep
//! their place, and ops that differ show up as removed from the first list or added in the
//! second. Ties prefer removals, so a changed op reads as a removal followed by an addition.
//!
//! The table is `n * m` entries, so this is for lists of a few thousand ops. Larger lists
//! return [`DiffError::TooLarge`] instead of using all the memory.

use alloc::vec;
use alloc::vec::Vec;

use super::{DisplayList, DrawOp};

/// Most table cells [`diff`] will allocate: about 64 MiB of `u32`.
pub const MAX_CELLS: usize = 16 * 1024 * 1024;

/// One step of a diff. Indices refer to the op lists being compared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffOp {
    /// Op `a` of the first list matches op `b` of the second.
    Same {
        /// Index in the first list.
        a: usize,
        /// Index in the second list.
        b: usize,
    },
    /// Op `a` of the first list is not in the second.
    Removed(usize),
    /// Op `b` of the second list is not in the first.
    Added(usize),
}

/// Why a diff was not made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffError {
    /// The two lists together are too long for the table [`diff`] would need.
    TooLarge {
        /// Ops in the first list.
        a: usize,
        /// Ops in the second list.
        b: usize,
    },
}

/// Diffs the ops of `a` against those of `b`, in order.
pub fn diff(a: &DisplayList, b: &DisplayList) -> Result<Vec<DiffOp>, DiffError> {
    diff_ops(a.ops(), b.ops())
}

/// Diffs two op slices. See the module docs for the rules.
pub fn diff_ops(a: &[DrawOp], b: &[DrawOp]) -> Result<Vec<DiffOp>, DiffError> {
    let (n, m) = (a.len(), b.len());
    let cells = (n + 1).saturating_mul(m + 1);
    if cells > MAX_CELLS {
        return Err(DiffError::TooLarge { a: n, b: m });
    }
    // lcs[i * (m + 1) + j] is the length of the longest common subsequence of a[i..] and b[j..].
    let w = m + 1;
    let mut lcs = vec![0u32; cells];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i * w + j] = if a[i] == b[j] {
                lcs[(i + 1) * w + j + 1] + 1
            } else {
                lcs[(i + 1) * w + j].max(lcs[i * w + j + 1])
            };
        }
    }

    let mut out = Vec::with_capacity(n + m);
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            out.push(DiffOp::Same { a: i, b: j });
            i += 1;
            j += 1;
        } else if lcs[(i + 1) * w + j] >= lcs[i * w + j + 1] {
            out.push(DiffOp::Removed(i));
            i += 1;
        } else {
            out.push(DiffOp::Added(j));
            j += 1;
        }
    }
    out.extend((i..n).map(DiffOp::Removed));
    out.extend((j..m).map(DiffOp::Added));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::PathBuilder;
    use crate::{Color, Context, Paint};

    fn list(ops: &[DrawOp]) -> DisplayList {
        let mut d = DisplayList::new();
        for op in ops {
            d.push_op(op.clone());
        }
        d
    }

    fn fill(x: f32) -> DrawOp {
        let mut ctx = Context::new(DisplayList::new());
        ctx.set_fill_paint(Paint::Solid(Color::BLACK));
        ctx.fill_rect(x, 0.0, 1.0, 1.0);
        ctx.into_backend().ops()[0].clone()
    }

    #[test]
    fn identical_lists_match_everywhere() {
        let a = list(&[fill(0.0), fill(1.0)]);
        let d = diff(&a, &a).unwrap();
        assert_eq!(
            d,
            vec![DiffOp::Same { a: 0, b: 0 }, DiffOp::Same { a: 1, b: 1 }]
        );
    }

    #[test]
    fn an_added_op_is_reported_in_place() {
        let a = list(&[fill(0.0), fill(2.0)]);
        let b = list(&[fill(0.0), fill(1.0), fill(2.0)]);
        let d = diff(&a, &b).unwrap();
        assert_eq!(
            d,
            vec![
                DiffOp::Same { a: 0, b: 0 },
                DiffOp::Added(1),
                DiffOp::Same { a: 1, b: 2 },
            ]
        );
    }

    #[test]
    fn a_removed_op_is_reported_in_place() {
        let a = list(&[fill(0.0), fill(1.0), fill(2.0)]);
        let b = list(&[fill(0.0), fill(2.0)]);
        let d = diff(&a, &b).unwrap();
        assert_eq!(
            d,
            vec![
                DiffOp::Same { a: 0, b: 0 },
                DiffOp::Removed(1),
                DiffOp::Same { a: 2, b: 1 },
            ]
        );
    }

    #[test]
    fn a_changed_op_is_a_removal_then_an_addition() {
        let a = list(&[fill(0.0), fill(1.0)]);
        let b = list(&[fill(0.0), fill(5.0)]);
        let d = diff(&a, &b).unwrap();
        assert_eq!(
            d,
            vec![
                DiffOp::Same { a: 0, b: 0 },
                DiffOp::Removed(1),
                DiffOp::Added(1),
            ]
        );
    }

    #[test]
    fn empty_lists_and_one_empty_side() {
        let e = DisplayList::new();
        assert!(diff(&e, &e).unwrap().is_empty());
        let a = list(&[fill(0.0)]);
        assert_eq!(diff(&a, &e).unwrap(), vec![DiffOp::Removed(0)]);
        assert_eq!(diff(&e, &a).unwrap(), vec![DiffOp::Added(0)]);
    }

    #[test]
    fn oversized_comparisons_are_refused() {
        // 3001 * 3001 cells is about 9 million, under the limit, so this one is made.
        let ok = vec![fill(0.0); 3000];
        assert!(diff_ops(&ok, &ok).is_ok());
        // 10001 * 10001 cells is about 100 million, which is over it.
        let big = vec![fill(0.0); 10_000];
        let err = diff_ops(&big, &big).unwrap_err();
        assert_eq!(
            err,
            DiffError::TooLarge {
                a: 10_000,
                b: 10_000
            }
        );
    }

    #[test]
    fn a_path_op_round_trips_through_the_diff() {
        let mut b = PathBuilder::new();
        b.rect(0.0, 0.0, 2.0, 2.0);
        let mut ctx = Context::new(DisplayList::new());
        ctx.fill_path(&b.build());
        let d = ctx.into_backend();
        assert_eq!(diff(&d, &d).unwrap(), vec![DiffOp::Same { a: 0, b: 0 }]);
    }
}
