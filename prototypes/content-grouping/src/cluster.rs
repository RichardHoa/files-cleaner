//! Average-linkage agglomerative clustering on a similarity matrix: start with
//! one cluster per file, keep merging the most similar pair while their
//! average similarity is at least the threshold.

use crate::embed::{cosine, Vector};

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

/// Cosine similarity of the vectors, or 1 when two files have the same
/// topic key: the LLM has already said they belong together, however
/// differently they are worded.
pub fn similarities(vectors: &[Vector], topics: &[Option<String>]) -> Similarities {
    let n = vectors.len();
    let mut sims = Similarities { n, values: vec![1.0; n * n] };
    for i in 0..n {
        for j in i + 1..n {
            let same_topic = topics[i].is_some() && topics[i] == topics[j];
            sims.set(i, j, if same_topic { 1.0 } else { cosine(&vectors[i], &vectors[j]) });
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

    fn topics(keys: &[Option<&str>]) -> Vec<Option<String>> {
        keys.iter().map(|k| k.map(str::to_string)).collect()
    }

    #[test]
    fn merges_similar_vectors_and_leaves_the_odd_one_out() {
        let v = vectors(&[[1.0, 0.1, 0.0], [1.0, 0.0, 0.1], [0.9, 0.1, 0.1], [0.0, 0.0, 1.0]]);
        let sims = similarities(&v, &topics(&[None, None, None, None]));
        let clusters = agglomerate(sims, 0.5, 2);
        assert_eq!(clusters.groups, [vec![0, 1, 2]]);
        assert_eq!(clusters.ungrouped, [3]);
    }

    #[test]
    fn the_same_topic_groups_files_whose_vectors_disagree() {
        // Orthogonal vectors: cosine 0, but the LLM gave both the topic "cs101".
        let v = vectors(&[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        let clusters = agglomerate(similarities(&v, &topics(&[Some("cs101"), Some("cs101"), None])), 0.8, 2);
        assert_eq!(clusters.groups, [vec![0, 1]]);
        assert_eq!(clusters.ungrouped, [2]);
    }

    #[test]
    fn no_topic_never_counts_as_the_same_topic() {
        let v = vectors(&[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
        assert!(agglomerate(similarities(&v, &topics(&[None, None])), 0.8, 2).groups.is_empty());
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
