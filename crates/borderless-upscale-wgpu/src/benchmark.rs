// SPDX-License-Identifier: GPL-3.0-or-later

use borderless_upscale_core::RendererBackend;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkSample {
    pub backend: RendererBackend,
    pub frame_index: u64,
    pub elapsed_nanos: u128,
}

impl BenchmarkSample {
    #[must_use]
    pub fn from_duration(backend: RendererBackend, frame_index: u64, elapsed: Duration) -> Self {
        Self {
            backend,
            frame_index,
            elapsed_nanos: elapsed.as_nanos(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendBenchmark {
    samples: Vec<BenchmarkSample>,
}

impl BackendBenchmark {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            samples: Vec::new(),
        }
    }

    pub fn push(&mut self, sample: BenchmarkSample) {
        self.samples.push(sample);
    }

    #[must_use]
    pub fn summary(&self, backend: RendererBackend) -> Option<BenchmarkSummary> {
        let mut samples = self
            .samples
            .iter()
            .copied()
            .filter(|sample| sample.backend == backend)
            .map(|sample| sample.elapsed_nanos)
            .collect::<Vec<_>>();
        if samples.is_empty() {
            return None;
        }
        samples.sort_unstable();
        let total = samples.iter().copied().sum::<u128>();
        Some(BenchmarkSummary {
            backend,
            frames: u64::try_from(samples.len()).ok()?,
            average_nanos: total / u128::try_from(samples.len()).ok()?,
            p95_nanos: percentile(&samples, 95),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    pub backend: RendererBackend,
    pub frames: u64,
    pub average_nanos: u128,
    pub p95_nanos: u128,
}

fn percentile(sorted: &[u128], percentile: usize) -> u128 {
    let index = sorted.len().saturating_sub(1).saturating_mul(percentile) / 100;
    sorted[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_backend_samples() {
        let mut benchmark = BackendBenchmark::new();
        benchmark.push(BenchmarkSample {
            backend: RendererBackend::WgpuDx12,
            frame_index: 0,
            elapsed_nanos: 10,
        });
        benchmark.push(BenchmarkSample {
            backend: RendererBackend::WgpuDx12,
            frame_index: 1,
            elapsed_nanos: 30,
        });

        let summary = benchmark.summary(RendererBackend::WgpuDx12).unwrap();

        assert_eq!(summary.frames, 2);
        assert_eq!(summary.average_nanos, 20);
    }
}
