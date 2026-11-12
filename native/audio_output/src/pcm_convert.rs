//! Pure normalized-f64 mono-to-stereo conversion at the native output boundary.

use crate::StereoFrame;

/// Convert one normalized mono f64 sample into an interleaved stereo frame.
///
/// Balance is expressed in the inclusive range -100..=100:
/// - -100 routes entirely to the left channel;
/// - 0 duplicates the sample to both channels;
/// - +100 routes entirely to the right channel.
///
/// Volume and clipping are applied by the upstream resampler exactly once.
/// This function applies only channel balance.
pub(crate) fn mono_sample_to_stereo_frame(sample: f64, balance: i8) -> StereoFrame {
    let left_gain = if balance > 0 {
        (100 - i32::from(balance)) as f64 / 100.0
    } else {
        1.0
    };
    let right_gain = if balance < 0 {
        (100 + i32::from(balance)) as f64 / 100.0
    } else {
        1.0
    };

    [sample * left_gain, sample * right_gain]
}

#[cfg(test)]
mod tests {
    use super::mono_sample_to_stereo_frame;

    #[test]
    fn center_balance_duplicates_normalized_samples_without_quantization() {
        for sample in [-1.0, -0.123_456_789, 0.0, 0.123_456_789, 1.0] {
            assert_eq!(mono_sample_to_stereo_frame(sample, 0), [sample, sample]);
        }
    }

    #[test]
    fn full_balance_routes_to_one_channel_without_changing_sample() {
        for sample in [-1.0, -0.123_456_789, 0.0, 0.123_456_789, 1.0] {
            assert_eq!(mono_sample_to_stereo_frame(sample, -100), [sample, 0.0]);
            assert_eq!(mono_sample_to_stereo_frame(sample, 100), [0.0, sample]);
        }
    }

    #[test]
    fn partial_balance_scales_only_the_attenuated_channel() {
        assert_eq!(mono_sample_to_stereo_frame(0.8, -50), [0.8, 0.4]);
        assert_eq!(mono_sample_to_stereo_frame(-0.8, 50), [-0.4, -0.8]);
    }
}
