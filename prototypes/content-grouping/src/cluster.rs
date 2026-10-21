//! Average-linkage agglomerative clustering on cosine similarity: start with
//! one cluster per file, keep merging the most similar pair while their
//! average similarity is at least the threshold.

use crate::embed::{cosine, Vector};

pub struct Clusters {
    /// Indices into the input, largest group first; each has at least
    /// `min_size` members.
    pub groups: Vec<Vec<usize>>,
    pub ungrouped: Vec<usize>,
}

pub fn agglomerate(vectors: &[Vector], threshold: f32, min_size: usize) -> Clusters {
    let n = vectors.len();
    // sim[i][j] is the average similarity between clusters i and j while both are alive.
    let mut sim = vec![vec![0f32; n]; n];
    for i in 0..n {
        for j in i + 1..n {
            let s = cosine(&vectors[i], &vectors[j]);
            sim[i][j] = s;
            sim[j][i] = s;
        }
    }
    let mut members: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
    let mut alive = vec![true; n];
    // Each live cluster's most similar live neighbour, so a merge step is O(n)
    // except for the rows whose neighbour just changed.
    let nearest = |i: usize, alive: &[bool], sim: &[Vec<f32>]| -> Option<(usize, f32)> {
        (0..n).filter(|&j| j != i && alive[j]).map(|j| (j, sim[i][j])).max_by(|a, b| a.1.total_cmp(&b.1))
    };
    let mut best: Vec<Option<(usize, f32)>> = (0..n).map(|i| nearest(i, &alive, &sim)).collect();

    while let Some((a, (b, _))) = (0..n)
        .filter(|&i| alive[i])
        .filter_map(|i| best[i].map(|nb| (i, nb)))
        .max_by(|x, y| x.1 .1.total_cmp(&y.1 .1))
        .filter(|(_, (_, s))| *s >= threshold)
    {
        // Merge b into a; the new row is the size-weighted average (Lance–Williams).
        let (na, nb) = (members[a].len() as f32, members[b].len() as f32);
        for k in 0..n {
            if alive[k] && k != a && k != b {
                let merged = (na * sim[a][k] + nb * sim[b][k]) / (na + nb);
                sim[a][k] = merged;
                sim[k][a] = merged;
            }
        }
        let moved = std::mem::take(&mut members[b]);
        members[a].extend(moved);
        alive[b] = false;
        best[b] = None;
        for k in 0..n {
            if !alive[k] {
                continue;
            }
            let stale = k == a || matches!(best[k], Some((j, _)) if j == a || j == b);
            if stale {
                best[k] = nearest(k, &alive, &sim);
            } else if let Some((_, ks)) = best[k] {
                if sim[k][a] > ks {
                    best[k] = Some((a, sim[k][a]));
                }
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
