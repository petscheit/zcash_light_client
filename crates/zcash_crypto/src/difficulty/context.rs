use crate::difficulty::filter::DiffError;
use crate::difficulty::target::{Target, target_from_nbits, target_to_nbits};

/// Sliding window of header data needed for contextual difficulty.
///
/// The timestamps and `nBits` values are kept for the most recent headers on
/// the selected chain, in height order from oldest to newest. This context is
/// assumed to describe headers up to and including `tip_height`.
pub struct DifficultyContext {
    /// Height of the tip header described by this context.
    pub tip_height: u32,
    times: Vec<u32>,
    bits: Vec<u32>,
}

impl DifficultyContext {
    /// Creates an empty context at the given tip height.
    ///
    /// Callers are expected to seed this from a checkpoint so that the context
    /// already includes at least 28 timestamps and 17 `nBits` values before
    /// verifying contextual difficulty for the next header.
    pub fn new(tip_height: u32) -> Self {
        DifficultyContext {
            tip_height,
            times: Vec::new(),
            bits: Vec::new(),
        }
    }

    /// Appends a newly accepted header to the context.
    pub fn push_header(&mut self, height: u32, n_time: u32, n_bits: u32) {
        self.tip_height = height;

        self.times.push(n_time);
        if self.times.len() > POW_MEDIAN_BLOCK_SPAN + POW_AVERAGING_WINDOW {
            self.times.remove(0);
        }

        self.bits.push(n_bits);
        if self.bits.len() > POW_AVERAGING_WINDOW {
            self.bits.remove(0);
        }
    }
}

const POW_AVERAGING_WINDOW: usize = 17;
const POW_MEDIAN_BLOCK_SPAN: usize = 11;
const POW_MAX_ADJUST_DOWN_NUM: i64 = 32;
const POW_MAX_ADJUST_UP_NUM: i64 = 16;
const POW_ADJUST_DEN: i64 = 100;
const POW_DAMPING_FACTOR: i64 = 4;
const POW_TARGET_SPACING: i64 = 75;
const AVERAGING_WINDOW_TIMESPAN: i64 = POW_AVERAGING_WINDOW as i64 * POW_TARGET_SPACING;
const MIN_ACTUAL_TIMESPAN: i64 =
    (AVERAGING_WINDOW_TIMESPAN * (POW_ADJUST_DEN - POW_MAX_ADJUST_UP_NUM)) / POW_ADJUST_DEN;
const MAX_ACTUAL_TIMESPAN: i64 =
    (AVERAGING_WINDOW_TIMESPAN * (POW_ADJUST_DEN + POW_MAX_ADJUST_DOWN_NUM)) / POW_ADJUST_DEN;

fn median_11(values: &[u32]) -> u32 {
    debug_assert!(values.len() == POW_MEDIAN_BLOCK_SPAN);
    let mut tmp = [0u32; POW_MEDIAN_BLOCK_SPAN];
    tmp.copy_from_slice(values);
    tmp.sort_unstable();
    tmp[POW_MEDIAN_BLOCK_SPAN / 2]
}

fn actual_timespan(ctx: &DifficultyContext) -> i64 {
    let len = ctx.times.len();
    if len < POW_MEDIAN_BLOCK_SPAN + POW_AVERAGING_WINDOW {
        return 0;
    }

    let recent_start = len - POW_MEDIAN_BLOCK_SPAN;
    let recent_median = median_11(&ctx.times[recent_start..]);

    let past_start = len - POW_MEDIAN_BLOCK_SPAN - POW_AVERAGING_WINDOW;
    let past_end = past_start + POW_MEDIAN_BLOCK_SPAN;
    let past_median = median_11(&ctx.times[past_start..past_end]);

    let span = recent_median as i64 - past_median as i64;
    if span == 0 {
        // Keep the same difficulty if timestamps are identical.
        AVERAGING_WINDOW_TIMESPAN
    } else {
        span
    }
}

fn actual_timespan_damped(ctx: &DifficultyContext) -> i64 {
    let ats = actual_timespan(ctx);
    AVERAGING_WINDOW_TIMESPAN + (ats - AVERAGING_WINDOW_TIMESPAN) / POW_DAMPING_FACTOR
}

fn clamp_timespan(value: i64) -> i64 {
    if value < MIN_ACTUAL_TIMESPAN {
        MIN_ACTUAL_TIMESPAN
    } else if value > MAX_ACTUAL_TIMESPAN {
        MAX_ACTUAL_TIMESPAN
    } else {
        value
    }
}

fn add_target(a: &Target, b: &Target) -> Target {
    let mut out = [0u8; 32];
    let mut carry: u16 = 0;
    for i in 0..32 {
        let sum = a[i] as u16 + b[i] as u16 + carry;
        out[i] = sum as u8;
        carry = sum >> 8;
    }
    out
}

fn div_target_u32(x: &Target, rhs: u32) -> Target {
    let mut out = [0u8; 32];
    let mut rem: u64 = 0;
    for i in (0..32).rev() {
        let cur = (rem << 8) | x[i] as u64;
        let q = cur / rhs as u64;
        rem = cur % rhs as u64;
        out[i] = q as u8;
    }
    out
}

fn mul_target_u32(x: &Target, rhs: u32) -> Target {
    let mut out = [0u8; 32];
    let mut carry: u64 = 0;
    for i in 0..32 {
        let cur = x[i] as u64 * rhs as u64 + carry;
        out[i] = cur as u8;
        carry = cur >> 8;
    }
    out
}

fn min_target(a: &Target, b: &Target) -> Target {
    use crate::difficulty::target::cmp_target;
    if cmp_target(a, b) == core::cmp::Ordering::Greater {
        *b
    } else {
        *a
    }
}

fn mean_target(ctx: &DifficultyContext) -> Target {
    let len = ctx.bits.len();
    let start = len.saturating_sub(POW_AVERAGING_WINDOW);
    let mut acc = [0u8; 32];
    for &bits in &ctx.bits[start..] {
        let t = target_from_nbits(bits);
        acc = add_target(&acc, &t);
    }
    div_target_u32(&acc, POW_AVERAGING_WINDOW as u32)
}

fn threshold(ctx: &DifficultyContext) -> Target {
    let ats = actual_timespan_damped(ctx);
    let ats_bounded = clamp_timespan(ats) as u32;

    let mean = mean_target(ctx);
    let scaled = mul_target_u32(
        &div_target_u32(&mean, AVERAGING_WINDOW_TIMESPAN as u32),
        ats_bounded,
    );
    min_target(&scaled, &crate::difficulty::filter::POW_LIMIT_LE)
}

/// Computes the expected `nBits` for the next header height given the context.
pub fn expected_nbits(ctx: &DifficultyContext, header_height: u32) -> Result<u32, DiffError> {
    if ctx.times.len() < POW_MEDIAN_BLOCK_SPAN + POW_AVERAGING_WINDOW
        || ctx.bits.len() < POW_AVERAGING_WINDOW
    {
        return Err(DiffError::InsufficientContext);
    }

    if header_height != ctx.tip_height + 1 {
        return Err(DiffError::HeightMismatch {
            expected: ctx.tip_height + 1,
            found: header_height,
        });
    }

    let thr = threshold(ctx);
    Ok(target_to_nbits(&thr))
}

/// Verifies that the header's `nBits` matches Zcash contextual difficulty.
pub fn verify_difficulty(
    ctx: &DifficultyContext,
    header_height: u32,
    header_bits: u32,
) -> Result<(), DiffError> {
    let expected = expected_nbits(ctx, header_height)?;
    if header_bits != expected {
        return Err(DiffError::BitsMismatch {
            expected,
            found: header_bits,
        });
    }
    Ok(())
}
