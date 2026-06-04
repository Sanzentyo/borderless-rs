// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::ScalingBackend;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalingAlgorithmId {
    #[default]
    Lanczos,
    Fsr1,
    Anime4k,
}

impl ScalingAlgorithmId {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lanczos => "lanczos",
            Self::Fsr1 => "fsr1",
            Self::Anime4k => "anime4k",
        }
    }

    #[must_use]
    pub fn parse_id(value: &str) -> Option<Self> {
        match value {
            "lanczos" => Some(Self::Lanczos),
            "fsr1" => Some(Self::Fsr1),
            "anime4k" => Some(Self::Anime4k),
            _ => None,
        }
    }

    #[must_use]
    pub fn selected_index(self) -> i32 {
        curated_scaling_algorithms()
            .iter()
            .position(|algorithm| algorithm.id == self)
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(0)
    }

    #[must_use]
    pub fn from_index(index: i32) -> Self {
        usize::try_from(index)
            .ok()
            .and_then(|index| curated_scaling_algorithms().get(index))
            .map_or_else(Self::default, |algorithm| algorithm.id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalingAlgorithmFamily {
    Reconstruction,
    SpatialUpscaler,
    AnimeLineArt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalingImplementationSource {
    CleanRoom,
    MagpiePort,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScalingAlgorithm {
    pub id: ScalingAlgorithmId,
    pub backend: ScalingBackend,
    pub name: &'static str,
    pub summary: &'static str,
    pub family: ScalingAlgorithmFamily,
    pub implementation_source: ScalingImplementationSource,
}

impl ScalingAlgorithm {
    #[must_use]
    pub const fn requires_gpl_boundary(self) -> bool {
        matches!(
            self.implementation_source,
            ScalingImplementationSource::MagpiePort
        )
    }
}

pub const CURATED_SCALING_ALGORITHMS: [ScalingAlgorithm; 3] = [
    ScalingAlgorithm {
        id: ScalingAlgorithmId::Lanczos,
        backend: ScalingBackend::Lanczos,
        name: "Lanczos",
        summary: "Sharp general-purpose reconstruction for 2D games.",
        family: ScalingAlgorithmFamily::Reconstruction,
        implementation_source: ScalingImplementationSource::CleanRoom,
    },
    ScalingAlgorithm {
        id: ScalingAlgorithmId::Fsr1,
        backend: ScalingBackend::Fsr1,
        name: "FSR 1.0",
        summary: "Spatial upscaling plus sharpening for modern low-resolution sources.",
        family: ScalingAlgorithmFamily::SpatialUpscaler,
        implementation_source: ScalingImplementationSource::MagpiePort,
    },
    ScalingAlgorithm {
        id: ScalingAlgorithmId::Anime4k,
        backend: ScalingBackend::Anime4k,
        name: "Anime4K",
        summary: "Line-art oriented upscale path for anime-style and visual novel content.",
        family: ScalingAlgorithmFamily::AnimeLineArt,
        implementation_source: ScalingImplementationSource::MagpiePort,
    },
];

#[must_use]
pub const fn curated_scaling_algorithms() -> &'static [ScalingAlgorithm] {
    &CURATED_SCALING_ALGORITHMS
}

#[must_use]
pub fn scaling_algorithm(id: ScalingAlgorithmId) -> &'static ScalingAlgorithm {
    curated_scaling_algorithms()
        .iter()
        .find(|algorithm| algorithm.id == id)
        .unwrap_or(&CURATED_SCALING_ALGORITHMS[0])
}

#[must_use]
pub fn scaling_algorithm_names() -> Vec<String> {
    curated_scaling_algorithms()
        .iter()
        .map(|algorithm| algorithm.name.to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn curated_registry_is_limited_to_initial_focus() {
        let ids = curated_scaling_algorithms()
            .iter()
            .map(|algorithm| algorithm.id)
            .collect::<Vec<_>>();

        assert_eq!(
            ids,
            vec![
                ScalingAlgorithmId::Lanczos,
                ScalingAlgorithmId::Fsr1,
                ScalingAlgorithmId::Anime4k
            ]
        );
    }

    #[test]
    fn curated_registry_has_unique_ids() {
        let ids = curated_scaling_algorithms()
            .iter()
            .map(|algorithm| algorithm.id.as_str())
            .collect::<HashSet<_>>();

        assert_eq!(ids.len(), curated_scaling_algorithms().len());
    }

    #[test]
    fn selected_index_round_trips() {
        for algorithm in curated_scaling_algorithms() {
            assert_eq!(
                ScalingAlgorithmId::from_index(algorithm.id.selected_index()),
                algorithm.id
            );
        }
    }
}
