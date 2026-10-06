//! What a benchmark's samples say: the median, which an outlier does not move, the spread around
//! it, and the mean and range of the samples that are no outliers (decision 600).

use vun_data::{FromData, ToData};
use vun_term::Percent;

/// How far past the quartiles a sample is an outlier, in interquartile ranges (Tukey's fences).
const FENCE: f64 = 1.5;

/// The statistics of one benchmark's samples, each a measure per iteration.
#[derive(Clone, Copy, Debug, PartialEq, ToData, FromData)]
pub struct Statistics {
    pub samples: usize,
    /// How many samples lie past the fences and are left out of `mean`, `low` and `high`.
    pub outliers: usize,
    pub median: f64,
    pub mean: f64,
    pub low: f64,
    pub high: f64,
    /// The median absolute deviation from the median: the spread outliers do not widen.
    pub deviation: f64,
}

impl Statistics {
    /// The statistics of `values`; `None` when there are none.
    #[must_use]
    pub fn of(values: &[f64]) -> Option<Self> {
        let sorted = Sorted::of(values)?;
        let (q1, q3) = (sorted.quantile(0.25), sorted.quantile(0.75));
        let reach = (q3 - q1) * FENCE;
        let (floor, ceiling) = (q1 - reach, q3 + reach);
        let kept: Vec<f64> = sorted.0.iter().copied().filter(|v| (floor..=ceiling).contains(v)).collect();
        let median = sorted.quantile(0.5);
        let deviations: Vec<f64> = sorted.0.iter().map(|v| (v - median).abs()).collect();
        #[expect(clippy::cast_precision_loss, reason = "a count of samples")]
        let mean = kept.iter().sum::<f64>() / kept.len() as f64;
        Some(Self {
            samples: sorted.0.len(),
            outliers: sorted.0.len() - kept.len(),
            median,
            mean,
            low: kept.first().copied().unwrap_or(median),
            high: kept.last().copied().unwrap_or(median),
            deviation: Sorted::of(&deviations)?.quantile(0.5),
        })
    }

    /// The deviation as a part of the median.
    #[must_use]
    pub fn spread(&self) -> Percent {
        Percent(if self.median > 0.0 { self.deviation / self.median * 100.0 } else { 0.0 })
    }
}

/// Values in ascending order.
struct Sorted(Vec<f64>);

impl Sorted {
    fn of(values: &[f64]) -> Option<Self> {
        if values.is_empty() {
            return None;
        }
        let mut sorted = values.to_vec();
        sorted.sort_by(f64::total_cmp);
        Some(Self(sorted))
    }

    /// The value a fraction `q` of the way up, between neighbours interpolated.
    fn quantile(&self, q: f64) -> f64 {
        #[expect(clippy::cast_precision_loss, reason = "an index")]
        let at = q * (self.0.len() - 1) as f64;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "an index below the length"
        )]
        let (below, above) = (at.floor() as usize, at.ceil() as usize);
        self.0[below] + (self.0[above] - self.0[below]) * (at - at.floor())
    }
}
