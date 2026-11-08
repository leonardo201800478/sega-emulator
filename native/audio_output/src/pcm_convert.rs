//! Pure PCM16 mono sample conversion at the native output boundary.

#[cfg(any(windows, test))]
use crate::StereoFrame;

/// Convert one signed mono PCM16 sample into an interleaved stereo frame.
///
/// Balance is expressed in the inclusive range -100..=100:
/// - -100 routes entirely to the left channel;
/// - 0 duplicates the sample to both channels;
/// - +100 routes entirely to the right channel.
///
/// Callers validate the balance setting when constructing the output device.
#[cfg(any(windows, test))]
pub(crate) fn mono_sample_to_stereo_frame(sample: i16, balance: i8) -> StereoFrame {
    let value = i32::from(sample);
    let left_gain = if balance > 0 {
        100 - i32::from(balance)
    } else {
        100
    };
    let right_gain = if balance < 0 {
        100 + i32::from(balance)
    } else {
        100
    };

    let left = ((value * left_gain) / 100) as i16;
    let right = ((value * right_gain) / 100) as i16;
    [left, right]
}

#[cfg(test)]
mod tests {
    use super::mono_sample_to_stereo_frame;

    #[test]
    fn center_balance_duplicates_signed_pcm16_samples() {
        for sample in [i16::MIN, -1, 0, 1, i16::MAX] {
            assert_eq!(mono_sample_to_stereo_frame(sample, 0), [sample, sample]);
        }
    }

    #[test]
    fn full_balance_routes_to_one_channel_without_changing_sample() {
        for sample in [i16::MIN, -1, 0, 1, i16::MAX] {
            assert_eq!(mono_sample_to_stereo_frame(sample, -100), [sample, 0]);
            assert_eq!(mono_sample_to_stereo_frame(sample, 100), [0, sample]);
        }
    }

    #[test]
    fn partial_balance_scales_only_the_attenuated_channel() {
        assert_eq!(mono_sample_to_stereo_frame(16_000, -50), [16_000, 8_000]);
        assert_eq!(mono_sample_to_stereo_frame(-16_000, 50), [-8_000, -16_000]);
    }
}
