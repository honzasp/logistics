use crate::{bit_mat::BitMat};

/// Maintains a transitive relation i -> j (i is-before j). The structure has
/// slow inserts (O(N^2) worst case, O(N^3 + I) amortized over I operations) but
/// fast queries (O(1) worst case).
#[derive(Debug)]
pub struct Constraints {
  /// `after[i, j]` is 1 iff i -> j
  after: BitMat,
  /// `before[j, i]` is 1 iff i -> j
  before: BitMat,
}

impl Constraints {
  pub fn new() -> Constraints {
    Constraints {
      after: BitMat::new(),
      before: BitMat::new(),
    }
  }

  /// Adds the relation i -> j to the relation. To maintain consistency, we must
  /// have `!self.is_before(j, i)`, but it is ok if `self.is_before(i, j)`.
  pub fn add_before(&mut self, i: u32, j: u32) {
    // check that we don't have j -> i
    assert!(!self.after.get(j as usize, i as usize));

    // if this relation is already satisfied, we don't have to do anything
    if self.after.set_replace(i as usize, j as usize) { return }
    self.before.set(j as usize, i as usize);

    // recursively fix the transitive closure:
    let mut todo = Vec::with_capacity(16);
    todo.push((i, j));
    while let Some((i, j)) = todo.pop() {
      // propagate "if i -> j and j -> k, then i -> k"
      {
        let (after, before) = (&mut self.after, &mut self.before);
        after.bitor_row(i as usize, j as usize, |k| {
          before.set(k, i as usize);
          todo.push((i, k as u32));
        });
      }

      // propagage "if i -> j and k -> i, then k -> j"
      {
        let (after, before) = (&mut self.after, &mut self.before);
        before.bitor_row(j as usize, i as usize, |k| {
          after.set(k, j as usize);
          todo.push((k as u32, j));
        });
      }
    }
  }

  /// Adds a new element to the support set, without any constraints.
  pub fn push(&mut self) {
    let i = self.after.count();
    self.after.push();
    self.after.set(i, i);
    self.before.push();
    self.before.set(i, i);
  }

  /// Checks whether i -> j is in the relation. Returns true if i == j.
  pub fn is_before(&self, i: u32, j: u32) -> bool {
    self.after.get(i as usize, j as usize)
  }

  /// Iterator over all j such that i -> j. In contrast to is_before(), i is not
  /// included in the set.
  pub fn successors<'s>(&'s self, i: u32) -> impl Iterator<Item = u32> + 's {
    self.after.row_ones(i as usize).map(|j| j as u32).filter(move |&j| j != i)
  }

  /// Iterator over all i such that i -> j. In contrast to is_before(), j is not
  /// included in the set.
  pub fn predecessors<'s>(&'s self, j: u32) -> impl Iterator<Item = u32> + 's {
    self.before.row_ones(j as usize).map(|i| i as u32).filter(move |&i| i != j)
  }

  /// Efficiently counts the number of predecessors.
  pub fn count_predecessors(&self, j: u32) -> u32 {
    self.before.count_row_ones(j as usize) as u32
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_single() {
    let mut c = Constraints::new();
    c.push();
    assert!(c.is_before(0, 0));
    assert_eq!(c.successors(0).collect::<Vec<_>>(), vec![]);
    assert_eq!(c.predecessors(0).collect::<Vec<_>>(), vec![]);
  }

  #[test]
  fn test_two() {
    let mut c = Constraints::new();
    c.push(); c.push();
    assert!(!c.is_before(0, 1));
    assert!(!c.is_before(1, 0));
    assert_eq!(c.successors(0).collect::<Vec<_>>(), vec![]);
    assert_eq!(c.successors(1).collect::<Vec<_>>(), vec![]);
    assert_eq!(c.predecessors(0).collect::<Vec<_>>(), vec![]);
    assert_eq!(c.predecessors(1).collect::<Vec<_>>(), vec![]);

    c.add_before(1, 0);
    assert!(!c.is_before(0, 1));
    assert!(c.is_before(1, 0));
    assert_eq!(c.successors(0).collect::<Vec<_>>(), vec![]);
    assert_eq!(c.successors(1).collect::<Vec<_>>(), vec![0]);
    assert_eq!(c.predecessors(0).collect::<Vec<_>>(), vec![1]);
    assert_eq!(c.predecessors(1).collect::<Vec<_>>(), vec![]);
  }

  #[test]
  fn test_path() {
    let mut c = Constraints::new();
    for _ in 0..5 { c.push(); }
    c.add_before(3, 0);
    c.add_before(2, 1);
    c.add_before(1, 3);
    c.add_before(0, 4);

    let order = vec![2, 1, 3, 0, 4];
    for i in 0..order.len() {
      for j in 0..order.len() {
        assert_eq!(c.is_before(order[i], order[j]), i <= j);
      }

      let mut successors = order[i+1..].to_vec(); successors.sort();
      let mut predecessors = order[..i].to_vec(); predecessors.sort();
      assert_eq!(c.successors(order[i]).collect::<Vec<_>>(), successors);
      assert_eq!(c.predecessors(order[i]).collect::<Vec<_>>(), predecessors);
    }
  }

  #[test]
  fn test_bipartite() {
    let mut c = Constraints::new();
    for _ in 0..5 { c.push(); }
    for &i in &[0, 1, 2] {
      for &j in &[3, 4] {
        c.add_before(i, j);
      }
    }

    for &i in &[0, 1, 2] {
      assert_eq!(c.successors(i).collect::<Vec<_>>(), vec![3, 4]);
      assert_eq!(c.predecessors(i).collect::<Vec<_>>(), vec![]);
    }
    for &j in &[3, 4] {
      assert_eq!(c.successors(j).collect::<Vec<_>>(), vec![]);
      assert_eq!(c.predecessors(j).collect::<Vec<_>>(), vec![0, 1, 2]);
    }
  }

  #[test]
  fn test_dag() {
    let mut c = Constraints::new();
    for _ in 0..6 { c.push(); }
    c.add_before(1, 2);
    c.add_before(2, 4);
    c.add_before(3, 5);
    c.add_before(0, 1);
    c.add_before(1, 3);
    c.add_before(3, 4);

    assert!(c.is_before(0, 4));
    assert!(c.is_before(1, 4));
    assert!(!c.is_before(2, 3));
    assert!(!c.is_before(3, 2));
    assert!(!c.is_before(4, 5));
    assert!(!c.is_before(5, 4));

    assert_eq!(c.successors(0).collect::<Vec<_>>(), vec![1, 2, 3, 4, 5]);
    assert_eq!(c.successors(1).collect::<Vec<_>>(), vec![2, 3, 4, 5]);
    assert_eq!(c.successors(2).collect::<Vec<_>>(), vec![4]);
    assert_eq!(c.successors(3).collect::<Vec<_>>(), vec![4, 5]);
    assert_eq!(c.successors(4).collect::<Vec<_>>(), vec![]);
    assert_eq!(c.successors(5).collect::<Vec<_>>(), vec![]);

    assert_eq!(c.predecessors(0).collect::<Vec<_>>(), vec![]);
    assert_eq!(c.predecessors(1).collect::<Vec<_>>(), vec![0]);
    assert_eq!(c.predecessors(2).collect::<Vec<_>>(), vec![0, 1]);
    assert_eq!(c.predecessors(3).collect::<Vec<_>>(), vec![0, 1]);
    assert_eq!(c.predecessors(4).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
    assert_eq!(c.predecessors(5).collect::<Vec<_>>(), vec![0, 1, 3]);
  }
}
