//! Average-linkage agglomerative clustering on a similarity matrix: start with
//! one cluster per file, keep merging the most similar pair while their
//! average similarity is at least the threshold.

use crate::embed::{cosine, Vector};
use std::collections::HashSet;

pub struct Clusters {
    /// Indices into the input, largest group first; each has at least
    /// `min_size` members.
    pub groups: Vec<Vec<usize>>,
    pub ungrouped: Vec<usize>,
}

/// Pairwise similarities between files, symmetric. n×n floats: 16 MB for
/// 2,000 files.
pub struct Similarities {
    n: usize,
    values: Vec<f32>,
}

impl Similarities {
    pub fn get(&self, i: usize, j: usize) -> f32 {
        self.values[i * self.n + j]
    }

    fn set(&mut self, i: usize, j: usize, v: f32) {
        self.values[i * self.n + j] = v;
        self.values[j * self.n + i] = v;
    }

    /// Each file's `k` most similar other files, most similar first.
    pub fn nearest(&self, k: usize) -> Vec<Vec<(usize, f32)>> {
        (0..self.n)
            .map(|i| {
                let mut row: Vec<(usize, f32)> = (0..self.n).filter(|&j| j != i).map(|j| (j, self.get(i, j))).collect();
                row.sort_by(|a, b| b.1.total_cmp(&a.1));
                row.truncate(k);
                row
            })
            .collect()
    }
}

/// Cosine similarity of the vectors, plus `name_bonus` when two files' names
/// share a word (`IELTS`, `CS101`, `invoice`), capped at 1. The embedding
/// sees the name too, but only as meaning; the bonus is the exact match the
/// baseline groups by.
pub fn similarities(vectors: &[Vector], name_words: &[HashSet<String>], name_bonus: f32) -> Similarities {
    let n = vectors.len();
    let mut sims = Similarities { n, values: vec![1.0; n * n] };
    for i in 0..n {
        for j in i + 1..n {
            let shared = !name_words[i].is_disjoint(&name_words[j]);
            let s = cosine(&vectors[i], &vectors[j]) + if shared { name_bonus } else { 0.0 };
            sims.set(i, j, s.min(1.0));
        }
    }
    sims
}

/// Consumes the matrix: its rows become the linkage between live clusters.
pub fn agglomerate(mut link: Similarities, threshold: f32, min_size: usize) -> Clusters {
    let n = link.n;
    let mut sizes = vec![1usize; n];
    let mut members: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
    let mut alive = vec![true; n];
    let nearest = |i: usize, alive: &[bool], link: &Similarities| -> Option<(usize, f32)> {
        (0..n)
            .filter(|&j| j != i && alive[j])
            .map(|j| (j, link.get(i, j)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
    };
    // Each live cluster's most similar live neighbour.
    let mut best: Vec<Option<(usize, f32)>> = (0..n).map(|i| nearest(i, &alive, &link)).collect();

    while let Some((a, (b, _))) = (0..n)
        .filter(|&i| alive[i])
        .filter_map(|i| best[i].map(|nb| (i, nb)))
        .max_by(|x, y| x.1 .1.total_cmp(&y.1 .1))
        .filter(|(_, (_, s))| *s >= threshold)
    {
        // Merge b into a: the average similarity to the merged cluster is
        // the size-weighted average of the similarities to a and to b.
        let (sa, sb) = (sizes[a] as f32, sizes[b] as f32);
        for k in (0..n).filter(|&k| alive[k] && k != a && k != b) {
            link.set(a, k, (sa * link.get(a, k) + sb * link.get(b, k)) / (sa + sb));
        }
        sizes[a] += sizes[b];
        let moved = std::mem::take(&mut members[b]);
        members[a].extend(moved);
        alive[b] = false;
        best[b] = None;
        // Only rows whose neighbour was a or b need a new one: anyone else's
        // similarity to the merged cluster is a weighted average of its old
        // similarities to a and b, so it can't beat that row's best.
        for k in 0..n {
            if alive[k] && (k == a || matches!(best[k], Some((j, _)) if j == a || j == b)) {
                best[k] = nearest(k, &alive, &link);
            }
        }
    }

    let mut groups = Vec::new();
    let mut ungrouped = Vec::new();
    for (i, m) in members.into_iter().enumerate() {
        if !alive[i] {
            continue;
        }
        if m.len() >= min_size {
            groups.push(m);
        } else {
            ungrouped.extend(m);
        }
    }
    groups.iter_mut().for_each(|g| g.sort());
    groups.sort_by(|a, b| b.len().cmp(&a.len()).then(a[0].cmp(&b[0])));
    ungrouped.sort();
    Clusters { groups, ungrouped }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::normalized;

    fn vectors(raw: &[[f32; 3]]) -> Vec<Vector> {
        raw.iter().map(|v| normalized(v.to_vec())).collect()
    }

    fn words(names: &[&[&str]]) -> Vec<HashSet<String>> {
        names.iter().map(|w| w.iter().map(|s| s.to_string()).collect()).collect()
    }

    #[test]
    fn merges_similar_vectors_and_leaves_the_odd_one_out() {
        let v = vectors(&[[1.0, 0.1, 0.0], [1.0, 0.0, 0.1], [0.9, 0.1, 0.1], [0.0, 0.0, 1.0]]);
        let sims = similarities(&v, &words(&[&[], &[], &[], &[]]), 0.1);
        let clusters = agglomerate(sims, 0.5, 2);
        assert_eq!(clusters.groups, [vec![0, 1, 2]]);
        assert_eq!(clusters.ungrouped, [3]);
    }

    #[test]
    fn a_shared_name_word_lifts_a_pair_over_the_threshold() {
        // cos 45° ≈ 0.71: under 0.75 alone, over it with the bonus.
        let v = vectors(&[[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        let alone = agglomerate(similarities(&v, &words(&[&["ielts"], &["writing"], &[]]), 0.1), 0.75, 2);
        assert!(alone.groups.is_empty());
        let shared = agglomerate(similarities(&v, &words(&[&["ielts"], &["ielts"], &[]]), 0.1), 0.75, 2);
        assert_eq!(shared.groups, [vec![0, 1]]);
        assert_eq!(shared.ungrouped, [2]);
    }

    #[test]
    fn linkage_is_the_average_over_both_clusters() {
        let mut sims = Similarities { n: 3, values: vec![1.0; 9] };
        sims.set(0, 1, 0.9);
        sims.set(0, 2, 0.6);
        sims.set(1, 2, 0.2);
        // {0,1} merge at 0.9; 2 is then at (0.6 + 0.2) / 2 = 0.4 from them.
        assert_eq!(agglomerate(sims, 0.45, 2).ungrouped, [2]);
    }
}
