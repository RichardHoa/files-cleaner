//! Average-linkage agglomerative clustering on cosine similarity: start with
//! one cluster per file, keep merging the most similar pair while their
//! average similarity is at least the threshold.

/// An L2-normalized embedding.
pub type Vector = Vec<f32>;

pub struct Clusters {
    /// Indices into the input, largest group first; each has at least
    /// `min_size` members.
    pub groups: Vec<Vec<usize>>,
    pub ungrouped: Vec<usize>,
}

/// Pairwise similarities between files, symmetric. n×n floats: 16 MB for
/// 2,000 files.
struct Similarities {
    n: usize,
    values: Vec<f32>,
}

impl Similarities {
    fn get(&self, i: usize, j: usize) -> f32 {
        self.values[i * self.n + j]
    }

    fn set(&mut self, i: usize, j: usize, v: f32) {
        self.values[i * self.n + j] = v;
        self.values[j * self.n + i] = v;
    }
}

/// Clusters files by their vectors, counting two files `same_topic` says
/// share a topic as similarity 1: the LLM has already said they belong
/// together, however differently they are worded.
pub fn cluster(
    vectors: &[Vector],
    same_topic: impl Fn(usize, usize) -> bool,
    threshold: f32,
    min_size: usize,
) -> Clusters {
    let n = vectors.len();
    let mut sims = Similarities {
        n,
        values: vec![1.0; n * n],
    };
    for i in 0..n {
        for j in i + 1..n {
            sims.set(i, j, similarity(&vectors[i], &vectors[j], same_topic(i, j)));
        }
    }
    agglomerate(sims, threshold, min_size)
}

/// Consumes the matrix: its rows become the linkage between live clusters.
fn agglomerate(mut link: Similarities, threshold: f32, min_size: usize) -> Clusters {
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
            link.set(
                a,
                k,
                (sa * link.get(a, k) + sb * link.get(b, k)) / (sa + sb),
            );
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

/// How alike two files are: 1 when they share a topic, else their cosine.
pub fn similarity(a: &[f32], b: &[f32], same_topic: bool) -> f32 {
    if same_topic {
        1.0
    } else {
        cosine(a, b)
    }
}

pub fn normalized(mut v: Vector) -> Vector {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        v.iter_mut().for_each(|x| *x /= norm);
    }
    v
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// The normalized mean of normalized vectors.
pub fn centroid<'a>(vectors: impl IntoIterator<Item = &'a Vector>) -> Option<Vector> {
    let mut sum: Option<Vector> = None;
    for v in vectors {
        let s = sum.get_or_insert_with(|| vec![0.0; v.len()]);
        s.iter_mut().zip(v).for_each(|(s, x)| *s += x);
    }
    sum.map(normalized)
}
