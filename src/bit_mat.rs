use std::{cmp};

#[derive(Debug)]
pub struct BitMat {
  storage: Vec<u32>,
  word_cap: usize,
  bit_count: usize,
}

impl BitMat {
  pub fn new() -> BitMat {
    BitMat { storage: Vec::new(), word_cap: 0, bit_count: 0 }
  }

  pub fn count(&self) -> usize {
    self.bit_count
  }

  pub fn push(&mut self) {
    if self.bit_count >= self.word_cap * 32 {
      // allocate new storage
      let new_word_cap = cmp::max(2 * self.word_cap, 1);
      let mut new_storage = Vec::new();
      new_storage.resize(32*new_word_cap*new_word_cap, 0);

      // copy each row from the old storage to the new storage
      for row_i in 0..self.bit_count {
        let new_row = &mut new_storage[row_i*new_word_cap..(row_i+1)*new_word_cap];
        let old_row = &mut self.storage[row_i*self.word_cap..(row_i+1)*self.word_cap];
        new_row[..old_row.len()].copy_from_slice(old_row);
      }

      // replace the storage
      self.storage = new_storage;
      self.word_cap = new_word_cap;
    }
    self.bit_count += 1;
  }

  fn index(&self, i: usize, j: usize) -> (usize, usize) {
    assert!(i < self.bit_count); assert!(j < self.bit_count);
    let word_idx = i*self.word_cap + (j / 32);
    let bit_idx = j % 32;
    (word_idx, bit_idx)
  }

  pub fn get(&self, i: usize, j: usize) -> bool {
    let (word_idx, bit_idx) = self.index(i, j);
    (self.storage[word_idx] & (1 << bit_idx)) != 0
  }

  pub fn set(&mut self, i: usize, j: usize) {
    let (word_idx, bit_idx) = self.index(i, j);
    self.storage[word_idx] |= 1 << bit_idx;
  }

  pub fn set_replace(&mut self, i: usize, j: usize) -> bool {
    let (word_idx, bit_idx) = self.index(i, j);
    let was_set = (self.storage[word_idx] & (1 << bit_idx)) != 0;
    self.storage[word_idx] |= 1 << bit_idx;
    was_set
  }

  pub fn bitor_row<F: FnMut(usize)>(&mut self,
    i_dst: usize, i_src: usize, mut callback: F)
  {
    for word_idx in 0..self.word_cap {
      // bitor one word of source row into destination row
      let dst_idx = i_dst * self.word_cap + word_idx;
      let src_idx = i_src * self.word_cap + word_idx;
      let dst_word = self.storage[dst_idx];
      let src_word = self.storage[src_idx];
      self.storage[dst_idx] = dst_word | src_word;

      // call the callback for each bit in destination that is newly set
      let mut new_word = !dst_word & src_word;
      let mut new_bit = 0;
      while new_word != 0 {
        let shift = new_word.trailing_zeros() as usize;
        callback(32*word_idx + new_bit + shift);
        new_word >>= shift;
        new_bit += shift + 1;
        new_word >>= 1;
      }
    }
  }

  pub fn row_ones<'s>(&'s self, i: usize) -> impl Iterator<Item = usize> + 's {
    BitMatOnesIterator::new(self, i)
  }

  pub fn count_row_ones(&self, i: usize) -> usize {
    (0..self.word_cap).map(|word_idx|
        self.storage[i * self.word_cap + word_idx].count_ones() as usize
    ).sum()
  }
}

#[derive(Debug)]
struct BitMatOnesIterator<'s> {
  row: &'s [u32],
  word_idx: usize,
  bit_idx: usize,
  word: u32,
}

impl<'s> BitMatOnesIterator<'s> {
  fn new(bitmat: &'s BitMat, row_idx: usize) -> BitMatOnesIterator<'s> {
    BitMatOnesIterator {
      row: &bitmat.storage[row_idx*bitmat.word_cap..(row_idx+1)*bitmat.word_cap],
      word_idx: 0,
      bit_idx: 0,
      word: 0,
    }
  }
}

impl<'s> Iterator for BitMatOnesIterator<'s> {
  type Item = usize;

  fn next(&mut self) -> Option<usize> {
    while self.word == 0 {
      if self.word_idx >= self.row.len() {
        return None
      } else {
        self.word = self.row[self.word_idx];
        self.word_idx += 1;
        self.bit_idx = 0;
      }
    }

    let shift = self.word.trailing_zeros() as usize;
    self.bit_idx += shift + 1;
    self.word >>= shift;
    self.word >>= 1;
    Some(32*(self.word_idx - 1) + (self.bit_idx - 1))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_push_count() {
    let mut bm = BitMat::new();
    assert_eq!(bm.count(), 0);
    bm.push(); assert_eq!(bm.count(), 1);
    bm.push(); assert_eq!(bm.count(), 2);
    bm.push(); assert_eq!(bm.count(), 3);
    bm.push(); assert_eq!(bm.count(), 4);
    bm.push(); assert_eq!(bm.count(), 5);
  }

  #[test]
  fn test_set_get_small() {
    let mut bm = BitMat::new();
    for _ in 0..4 { bm.push(); }

    for i in 0..4 {
      for j in 0..4 {
        assert_eq!(bm.get(i, j), false);
      }
    }

    bm.set(2, 3);
    bm.set(0, 2);
    bm.set(2, 1);
    bm.set(1, 3);
    bm.set(3, 3);

    let expected = [
      [false, false, true, false],
      [false, false, false, true],
      [false, true, false, true],
      [false, false, false, true],
    ];
    for i in 0..4 {
      for j in 0..4 {
        assert_eq!(bm.get(i, j), expected[i][j]);
      }
    }
  }

  #[test]
  fn test_bitor_row_small() {
    let mut bm = BitMat::new();
    for _ in 0..4 { bm.push(); }
    bm.set(0, 1);
    bm.set(0, 2);
    bm.set(1, 0);
    bm.set(1, 2);
    bm.set(2, 3);
    bm.set(3, 1);

    let mut js = Vec::new();
    bm.bitor_row(1, 0, |j| js.push(j));
    assert_eq!(js, vec![1]);

    let expected = [
      [false, true, true, false],
      [true, true, true, false],
      [false, false, false, true],
      [false, true, false, false],
    ];
    for i in 0..4 {
      for j in 0..4 {
        assert_eq!(bm.get(i, j), expected[i][j]);
      }
    }
  }

  #[test]
  fn test_large() {
    let mut bm = BitMat::new();
    for _ in 0..1000 { bm.push(); }
    assert_eq!(bm.count(), 1000);

    let mut truth = vec![false; 1000*1000];
    for n in 0..10000 {
      let i = (2551*n) % 1000;
      let j = (3767*n) % 1000;
      assert_eq!(bm.set_replace(i, j), truth[i*1000+j]);
      truth[i*1000+j] = true;
    }

    for n in 0..200 {
      let i_dst = (3557*n) % 1000;
      let i_src = (1607*n) % 1000;
      let mut js = vec![];
      bm.bitor_row(i_dst, i_src, |j| js.push(j));

      let js_expected = (0..1000).filter(|j|
        !truth[i_dst*1000+j] && truth[i_src*1000+j]).collect::<Vec<_>>();
      assert_eq!(js, js_expected);

      for j in 0..1000 {
        truth[i_dst*1000+j] |= truth[i_src*1000+j];
      }
    }

    for i in 0..1000 {
      for j in 0..1000 {
        assert_eq!(bm.get(i, j), truth[i*1000+j]);
      }
    }
  }
}
