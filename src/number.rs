use std::num::NonZeroU64;

use bigdecimal::{BigDecimal, RoundingMode, ToPrimitive, Zero, num_bigint::BigInt};

use crate::parser::Binary;
use crate::{Diagnostic, Position};

const DECIMAL128_DIGITS: u64 = 34;
// BigInteger stores at most Integer.MAX_VALUE bits in the reference VM.
const MAX_COEFFICIENT_DIGITS: u64 = 646_456_993;
const MAX_POWER: u64 = 999_999_999;

pub(crate) fn parse_number(text: &str, position: Position) -> Result<BigDecimal, Diagnostic> {
    let number: BigDecimal = text
        .parse()
        .map_err(|_| position.error("nombre invalide"))?;
    if i32::try_from(number.fractional_digit_count()).is_err() {
        return Err(position.error("nombre hors des limites de cet interpréteur"));
    }
    normalize(number, position)
}

pub(crate) fn normalize(number: BigDecimal, position: Position) -> Result<BigDecimal, Diagnostic> {
    let number = number.normalized();
    let scale = number.fractional_digit_count();
    let digits = number.digits();
    if i32::try_from(scale).is_err()
        || digits + scale.saturating_neg().max(0) as u64 > MAX_COEFFICIENT_DIGITS
    {
        return Err(position.error("nombre hors des limites de cet interpréteur"));
    }

    // In Linotte, whole numbers use scale zero even when the coefficient ends in zero.
    if number.fractional_digit_count() < 0 {
        Ok(number.with_scale(0))
    } else {
        Ok(number)
    }
}

pub(crate) fn arithmetic(
    op: Binary,
    left: &BigDecimal,
    right: &BigDecimal,
    position: Position,
    configured: Option<u64>,
) -> Result<BigDecimal, Diagnostic> {
    let mut precision = configured.unwrap_or(DECIMAL128_DIGITS);
    let mut rounding = if configured.is_some() {
        RoundingMode::HalfUp
    } else {
        RoundingMode::HalfEven
    };
    let improve = matches!(op, Binary::Add | Binary::Subtract | Binary::Divide)
        || matches!(op, Binary::Multiply) && configured.is_none();
    if improve && (left.digits() > precision || right.digits() > precision) {
        precision = left.digits() + right.digits();
        rounding = RoundingMode::HalfUp;
    }

    let result = match op {
        Binary::Add => left + right,
        Binary::Subtract => left - right,
        Binary::Multiply => left * right,
        Binary::Modulo => {
            if right.is_zero() {
                return Err(position.error("division par zéro"));
            }
            if precision > 0 && (left / right).with_scale(0).digits() > precision {
                return Err(position.error("quotient entier hors précision"));
            }
            return normalize(left % right, position);
        }
        Binary::Power => {
            let exp = right
                .to_i64()
                .filter(|e| e.unsigned_abs() <= MAX_POWER)
                .ok_or_else(|| position.error("exposant hors limites"))?;
            if left.is_zero() && exp < 0 {
                return Err(position.error("division par zéro"));
            }
            if precision == 0 {
                if exp < 0 {
                    return Err(position.error("exposant négatif en précision illimitée"));
                }
                let (coefficient, scale) = left.as_bigint_and_exponent();
                let scale = scale
                    .checked_mul(exp)
                    .ok_or_else(|| position.error("exposant hors limites"))?;
                return normalize(
                    BigDecimal::new(coefficient.pow(exp as u32), scale),
                    position,
                );
            }
            if exp.unsigned_abs().to_string().len() as u64 > precision {
                return Err(position.error("exposant hors précision"));
            }
            let context = bigdecimal::Context::new(NonZeroU64::new(precision).unwrap(), rounding);
            left.powi_with_context(exp, &context)
        }
        Binary::Divide => return divide(left, right, precision, rounding, position),
        _ => return Err(position.error("opération numérique invalide")),
    };
    if precision == 0 {
        return normalize(result, position);
    }
    let precision = NonZeroU64::new(precision).expect("decimal precision is positive");
    normalize(result.with_precision_round(precision, rounding), position)
}

/// The statement uses the selected context even when its operands have more digits.
pub(crate) fn divide_verb(
    left: &BigDecimal,
    right: &BigDecimal,
    configured: Option<u64>,
    position: Position,
) -> Result<BigDecimal, Diagnostic> {
    let (precision, rounding) = match configured {
        Some(precision) => (precision, RoundingMode::HalfUp),
        None => (DECIMAL128_DIGITS, RoundingMode::HalfEven),
    };
    divide(left, right, precision, rounding, position)
}

fn divide(
    left: &BigDecimal,
    right: &BigDecimal,
    precision: u64,
    rounding: RoundingMode,
    position: Position,
) -> Result<BigDecimal, Diagnostic> {
    if right.is_zero() {
        return Err(position.error("division par zéro"));
    }
    if left.is_zero() {
        return Ok(BigDecimal::zero());
    }
    if let Some(digits) = NonZeroU64::new(precision) {
        let quotient = division_with_guard_digit(left, right, precision);
        return normalize(quotient.with_precision_round(digits, rounding), position);
    }

    // A terminating reduced denominator has only factors of two and five.
    // Each decimal digit of the denominator can contribute at most log2(10)
    // such factors. This bounds the digits needed to recover an exact quotient.
    let bound = left.digits() + (10 * right.digits()).div_ceil(3);
    let quotient = division_with_guard_digit(left, right, bound);
    if &quotient * right != *left {
        return Err(position.error("développement décimal illimité"));
    }
    normalize(quotient, position)
}

fn division_with_guard_digit(left: &BigDecimal, right: &BigDecimal, precision: u64) -> BigDecimal {
    let (mut numerator, left_scale) = left.as_bigint_and_exponent();
    let (mut denominator, right_scale) = right.as_bigint_and_exponent();
    let scale = precision as i64 + 1 - (left.order_of_magnitude() - right.order_of_magnitude());
    let shift = scale + right_scale - left_scale;
    let power = BigInt::from(10).pow(shift.unsigned_abs() as u32);
    if shift >= 0 {
        numerator *= power;
    } else {
        denominator *= power;
    }
    let quotient = &numerator / &denominator;
    if (&numerator % &denominator).is_zero() {
        return BigDecimal::new(quotient, scale);
    }
    // Keep a sticky digit so the final rounding can distinguish an exact tie.
    let sticky = if numerator.sign() == denominator.sign() {
        1
    } else {
        -1
    };
    BigDecimal::new(quotient * 10 + sticky, scale + 1)
}

pub(crate) fn format_number(number: &BigDecimal) -> String {
    if number.order_of_magnitude() >= -6 {
        return number.to_plain_string();
    }
    let mut coefficient = number.as_bigint_and_scale().0.to_string();
    let first_digit = usize::from(coefficient.starts_with('-'));
    if coefficient.len() > first_digit + 1 {
        coefficient.insert(first_digit + 1, '.');
    }
    format!("{coefficient}E{}", number.order_of_magnitude())
}
