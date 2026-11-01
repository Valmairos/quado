// SPDX-License-Identifier: GPL-3.0-or-later
//! Tempi dei frame e percentili (criteri di M0, 07 §1.1).

use std::collections::VecDeque;

/// Riassunto di una serie di tempi, in millisecondi.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Summary {
    pub n: usize,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub max: f64,
    pub mean: f64,
}

/// Percentile col metodo del rango più vicino (nessuna interpolazione: il valore esiste nella serie).
pub fn summarize(samples: &[f64]) -> Summary {
    if samples.is_empty() {
        return Summary::default();
    }
    let mut v = samples.to_vec();
    v.sort_by(f64::total_cmp);
    let rank = |p: f64| -> f64 {
        let k = ((p / 100.0) * v.len() as f64).ceil() as usize;
        v[k.clamp(1, v.len()) - 1]
    };
    Summary {
        n: v.len(),
        p50: rank(50.0),
        p95: rank(95.0),
        p99: rank(99.0),
        max: v[v.len() - 1],
        mean: v.iter().sum::<f64>() / v.len() as f64,
    }
}

/// Ultimi `cap` tempi, per la sovrimpressione dal vivo.
#[derive(Clone, Debug)]
pub struct Rolling {
    cap: usize,
    samples: VecDeque<f64>,
}

impl Rolling {
    pub fn new(cap: usize) -> Self {
        Self { cap, samples: VecDeque::with_capacity(cap) }
    }

    pub fn push(&mut self, ms: f64) {
        if self.samples.len() == self.cap {
            self.samples.pop_front();
        }
        self.samples.push_back(ms);
    }

    pub fn summary(&self) -> Summary {
        let v: Vec<f64> = self.samples.iter().copied().collect();
        summarize(&v)
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        let s = summarize(&v);
        assert_eq!((s.p50, s.p95, s.p99, s.max), (50.0, 95.0, 99.0, 100.0));
        assert_eq!(summarize(&[]).n, 0);
        assert_eq!(summarize(&[7.0]).p99, 7.0);
    }

    #[test]
    fn rolling_drops_oldest() {
        let mut r = Rolling::new(3);
        for x in [100.0, 1.0, 2.0, 3.0] {
            r.push(x);
        }
        assert_eq!(r.summary().max, 3.0);
    }
}
