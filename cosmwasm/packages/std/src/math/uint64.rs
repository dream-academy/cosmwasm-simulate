use forward_ref::{forward_ref_binop, forward_ref_op_assign};
use schemars::JsonSchema;
use serde::{de, ser, Deserialize, Deserializer, Serialize};
use std::fmt::{self};
use std::ops::{
    Add, AddAssign, Div, DivAssign, Mul, MulAssign, Rem, RemAssign, Shr, ShrAssign, Sub, SubAssign,
};

use crate::errors::{
    CheckedMultiplyRatioError, DivideByZeroError, OverflowError, OverflowOperation, StdError,
};
use crate::Uint128;

/// A thin wrapper around u64 that is using strings for JSON encoding/decoding,
/// such that the full u64 range can be used for clients that convert JSON numbers to floats,
/// like JavaScript and jq.
///
/// # Examples
///
/// Use `from` to create instances of this and `u64` to get the value out:
///
/// ```
/// # use cosmwasm_std::Uint64;
/// let a = Uint64::from(42u64);
/// assert_eq!(a.u64(), 42);
///
/// let b = Uint64::from(70u32);
/// assert_eq!(b.u64(), 70);
/// ```
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord, JsonSchema)]
pub struct Uint64(#[schemars(with = "String")] u64);

impl Uint64 {
    pub const MAX: Self = Self(u64::MAX);
    pub const MIN: Self = Self(u64::MIN);

    /// Creates a Uint64(value).
    ///
    /// This method is less flexible than `from` but can be called in a const context.
    pub const fn new(value: u64) -> Self {
        Uint64(value)
    }

    /// Creates a Uint64(0)
    #[inline]
    pub const fn zero() -> Self {
        Uint64(0)
    }

    /// Creates a Uint64(1)
    #[inline]
    pub const fn one() -> Self {
        Self(1)
    }

    /// Returns a copy of the internal data
    pub const fn u64(&self) -> u64 {
        self.0
    }

    /// Returns a copy of the number as big endian bytes.
    pub const fn to_be_bytes(self) -> [u8; 8] {
        self.0.to_be_bytes()
    }

    /// Returns a copy of the number as little endian bytes.
    pub const fn to_le_bytes(self) -> [u8; 8] {
        self.0.to_le_bytes()
    }

    pub const fn is_zero(&self) -> bool {
        self.0 == 0
    }

    pub fn pow(self, exp: u32) -> Self {
        self.0.pow(exp).into()
    }

    /// Returns `self * numerator / denominator`
    pub fn multiply_ratio<A: Into<u64>, B: Into<u64>>(
        &self,
        numerator: A,
        denominator: B,
    ) -> Uint64 {
        match self.checked_multiply_ratio(numerator, denominator) {
            Ok(value) => value,
            Err(CheckedMultiplyRatioError::DivideByZero) => {
                panic!("Denominator must not be zero")
            }
            Err(CheckedMultiplyRatioError::Overflow) => panic!("Multiplication overflow"),
        }
    }

    /// Returns `self * numerator / denominator`
    pub fn checked_multiply_ratio<A: Into<u64>, B: Into<u64>>(
        &self,
        numerator: A,
        denominator: B,
    ) -> Result<Uint64, CheckedMultiplyRatioError> {
        let numerator = numerator.into();
        let denominator = denominator.into();
        if denominator == 0 {
            return Err(CheckedMultiplyRatioError::DivideByZero);
        }
        match (self.full_mul(numerator) / Uint128::from(denominator)).try_into() {
            Ok(ratio) => Ok(ratio),
            Err(_) => Err(CheckedMultiplyRatioError::Overflow),
        }
    }

    /// Multiplies two `Uint64`/`u64` values without overflow, producing an
    /// [`Uint128`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cosmwasm_std::Uint64;
    ///
    /// let a = Uint64::MAX;
    /// let result = a.full_mul(2u32);
    /// assert_eq!(result.to_string(), "36893488147419103230");
    /// ```
    pub fn full_mul(self, rhs: impl Into<u64>) -> Uint128 {
        Uint128::from(self.u64())
            .checked_mul(Uint128::from(rhs.into()))
            .unwrap()
    }

    pub fn checked_add(self, other: Self) -> Result<Self, OverflowError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or_else(|| OverflowError::new(OverflowOperation::Add, self, other))
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, OverflowError> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or_else(|| OverflowError::new(OverflowOperation::Sub, self, other))
    }

    pub fn checked_mul(self, other: Self) -> Result<Self, OverflowError> {
        self.0
            .checked_mul(other.0)
            .map(Self)
            .ok_or_else(|| OverflowError::new(OverflowOperation::Mul, self, other))
    }

    pub fn checked_pow(self, exp: u32) -> Result<Self, OverflowError> {
        self.0
            .checked_pow(exp)
            .map(Self)
            .ok_or_else(|| OverflowError::new(OverflowOperation::Pow, self, exp))
    }

    pub fn checked_div(self, other: Self) -> Result<Self, DivideByZeroError> {
        self.0
            .checked_div(other.0)
            .map(Self)
            .ok_or_else(|| DivideByZeroError::new(self))
    }

    pub fn checked_div_euclid(self, other: Self) -> Result<Self, DivideByZeroError> {
        self.0
            .checked_div_euclid(other.0)
            .map(Self)
            .ok_or_else(|| DivideByZeroError::new(self))
    }

    pub fn checked_rem(self, other: Self) -> Result<Self, DivideByZeroError> {
        self.0
            .checked_rem(other.0)
            .map(Self)
            .ok_or_else(|| DivideByZeroError::new(self))
    }

    #[inline]
    pub fn wrapping_add(self, other: Self) -> Self {
        Self(self.0.wrapping_add(other.0))
    }

    #[inline]
    pub fn wrapping_sub(self, other: Self) -> Self {
        Self(self.0.wrapping_sub(other.0))
    }

    #[inline]
    pub fn wrapping_mul(self, other: Self) -> Self {
        Self(self.0.wrapping_mul(other.0))
    }

    #[inline]
    pub fn wrapping_pow(self, other: u32) -> Self {
        Self(self.0.wrapping_pow(other))
    }

    pub fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    pub fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    pub fn saturating_mul(self, other: Self) -> Self {
        Self(self.0.saturating_mul(other.0))
    }

    pub fn saturating_pow(self, exp: u32) -> Self {
        Self(self.0.saturating_pow(exp))
    }

    pub const fn abs_diff(self, other: Self) -> Self {
        Self(if self.0 < other.0 {
            other.0 - self.0
        } else {
            self.0 - other.0
        })
    }
}

// `From<u{128,64,32,16,8}>` is implemented manually instead of
// using `impl<T: Into<u64>> From<T> for Uint64` because
// of the conflict with `TryFrom<&str>` as described here
// https://stackoverflow.com/questions/63136970/how-do-i-work-around-the-upstream-crates-may-add-a-new-impl-of-trait-error

impl From<u64> for Uint64 {
    fn from(val: u64) -> Self {
        Uint64(val)
    }
}

impl From<u32> for Uint64 {
    fn from(val: u32) -> Self {
        Uint64(val.into())
    }
}

impl From<u16> for Uint64 {
    fn from(val: u16) -> Self {
        Uint64(val.into())
    }
}

impl From<u8> for Uint64 {
    fn from(val: u8) -> Self {
        Uint64(val.into())
    }
}

impl TryFrom<&str> for Uint64 {
    type Error = StdError;

    fn try_from(val: &str) -> Result<Self, Self::Error> {
        match val.parse::<u64>() {
            Ok(u) => Ok(Uint64(u)),
            Err(e) => Err(StdError::generic_err(format!("Parsing u64: {}", e))),
        }
    }
}

impl From<Uint64> for String {
    fn from(original: Uint64) -> Self {
        original.to_string()
    }
}

impl From<Uint64> for u64 {
    fn from(original: Uint64) -> Self {
        original.0
    }
}

impl fmt::Display for Uint64 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Add<Uint64> for Uint64 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Uint64(self.u64().checked_add(rhs.u64()).unwrap())
    }
}

impl<'a> Add<&'a Uint64> for Uint64 {
    type Output = Self;

    fn add(self, rhs: &'a Uint64) -> Self {
        Uint64(self.u64().checked_add(rhs.u64()).unwrap())
    }
}

impl Sub<Uint64> for Uint64 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Uint64(
            self.u64()
                .checked_sub(rhs.u64())
                .expect("attempt to subtract with overflow"),
        )
    }
}
forward_ref_binop!(impl Sub, sub for Uint64, Uint64);

impl SubAssign<Uint64> for Uint64 {
    fn sub_assign(&mut self, rhs: Uint64) {
        *self = *self - rhs;
    }
}
forward_ref_op_assign!(impl SubAssign, sub_assign for Uint64, Uint64);

impl Mul<Uint64> for Uint64 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self(
            self.u64()
                .checked_mul(rhs.u64())
                .expect("attempt to multiply with overflow"),
        )
    }
}
forward_ref_binop!(impl Mul, mul for Uint64, Uint64);

impl MulAssign<Uint64> for Uint64 {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}
forward_ref_op_assign!(impl MulAssign, mul_assign for Uint64, Uint64);

impl Div<Uint64> for Uint64 {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self(self.u64().checked_div(rhs.u64()).unwrap())
    }
}

impl<'a> Div<&'a Uint64> for Uint64 {
    type Output = Self;

    fn div(self, rhs: &'a Uint64) -> Self::Output {
        Self(self.u64().checked_div(rhs.u64()).unwrap())
    }
}

impl Rem for Uint64 {
    type Output = Self;

    /// # Panics
    ///
    /// This operation will panic if `rhs` is zero.
    #[inline]
    fn rem(self, rhs: Self) -> Self {
        Self(self.0.rem(rhs.0))
    }
}
forward_ref_binop!(impl Rem, rem for Uint64, Uint64);

impl RemAssign<Uint64> for Uint64 {
    fn rem_assign(&mut self, rhs: Uint64) {
        *self = *self % rhs;
    }
}
forward_ref_op_assign!(impl RemAssign, rem_assign for Uint64, Uint64);

impl Shr<u32> for Uint64 {
    type Output = Self;

    fn shr(self, rhs: u32) -> Self::Output {
        Self(self.u64().checked_shr(rhs).unwrap())
    }
}

impl<'a> Shr<&'a u32> for Uint64 {
    type Output = Self;

    fn shr(self, rhs: &'a u32) -> Self::Output {
        Self(self.u64().checked_shr(*rhs).unwrap())
    }
}

impl AddAssign<Uint64> for Uint64 {
    fn add_assign(&mut self, rhs: Uint64) {
        self.0 = self.0.checked_add(rhs.u64()).unwrap();
    }
}

impl<'a> AddAssign<&'a Uint64> for Uint64 {
    fn add_assign(&mut self, rhs: &'a Uint64) {
        self.0 = self.0.checked_add(rhs.u64()).unwrap();
    }
}

impl DivAssign<Uint64> for Uint64 {
    fn div_assign(&mut self, rhs: Self) {
        self.0 = self.0.checked_div(rhs.u64()).unwrap();
    }
}

impl<'a> DivAssign<&'a Uint64> for Uint64 {
    fn div_assign(&mut self, rhs: &'a Uint64) {
        self.0 = self.0.checked_div(rhs.u64()).unwrap();
    }
}

impl ShrAssign<u32> for Uint64 {
    fn shr_assign(&mut self, rhs: u32) {
        self.0 = self.0.checked_shr(rhs).unwrap();
    }
}

impl<'a> ShrAssign<&'a u32> for Uint64 {
    fn shr_assign(&mut self, rhs: &'a u32) {
        self.0 = self.0.checked_shr(*rhs).unwrap();
    }
}

impl Serialize for Uint64 {
    /// Serializes as an integer string using base 10
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Uint64 {
    /// Deserialized from an integer string using base 10
    fn deserialize<D>(deserializer: D) -> Result<Uint64, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(Uint64Visitor)
    }
}

struct Uint64Visitor;

impl<'de> de::Visitor<'de> for Uint64Visitor {
    type Value = Uint64;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("string-encoded integer")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match v.parse::<u64>() {
            Ok(u) => Ok(Uint64(u)),
            Err(e) => Err(E::custom(format!("invalid Uint64 '{}' - {}", v, e))),
        }
    }
}

impl<A> std::iter::Sum<A> for Uint64
where
    Self: Add<A, Output = Self>,
{
    fn sum<I: Iterator<Item = A>>(iter: I) -> Self {
        iter.fold(Self::zero(), Add::add)
    }
}

impl PartialEq<&Uint64> for Uint64 {
    fn eq(&self, rhs: &&Uint64) -> bool {
        self == *rhs
    }
}

impl PartialEq<Uint64> for &Uint64 {
    fn eq(&self, rhs: &Uint64) -> bool {
        *self == rhs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{from_slice, to_vec};

    #[test]
    fn uint64_zero_works() {
        let zero = Uint64::zero();
        assert_eq!(zero.to_be_bytes(), [0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn uint64_one_works() {
        let one = Uint64::one();
        assert_eq!(one.to_be_bytes(), [0, 0, 0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn uint64_convert_into() {
        let original = Uint64(12345);
        let a = u64::from(original);
        assert_eq!(a, 12345);

        let original = Uint64(12345);
        let a = String::from(original);
        assert_eq!(a, "12345");
    }

    #[test]
    fn uint64_convert_from() {
        let a = Uint64::from(5u64);
        assert_eq!(a.0, 5);

        let a = Uint64::from(5u32);
        assert_eq!(a.0, 5);

        let a = Uint64::from(5u16);
        assert_eq!(a.0, 5);

        let a = Uint64::from(5u8);
        assert_eq!(a.0, 5);

        let result = Uint64::try_from("34567");
        assert_eq!(result.unwrap().0, 34567);

        let result = Uint64::try_from("1.23");
        assert!(result.is_err());
    }

    #[test]
    fn uint64_implements_display() {
        let a = Uint64(12345);
        assert_eq!(format!("Embedded: {}", a), "Embedded: 12345");
        assert_eq!(a.to_string(), "12345");

        let a = Uint64(0);
        assert_eq!(format!("Embedded: {}", a), "Embedded: 0");
        assert_eq!(a.to_string(), "0");
    }

    #[test]
    fn uint64_display_padding_works() {
        let a = Uint64::from(123u64);
        assert_eq!(format!("Embedded: {:05}", a), "Embedded: 00123");
    }

    #[test]
    fn uint64_to_be_bytes_works() {
        assert_eq!(Uint64::zero().to_be_bytes(), [0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            Uint64::MAX.to_be_bytes(),
            [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]
        );
        assert_eq!(Uint64::new(1).to_be_bytes(), [0, 0, 0, 0, 0, 0, 0, 1]);
        // Python: `[b for b in (63374607431768124608).to_bytes(8, "big")]`
        assert_eq!(
            Uint64::new(874607431768124608).to_be_bytes(),
            [12, 35, 58, 211, 72, 116, 172, 192]
        );
    }

    #[test]
    fn uint64_to_le_bytes_works() {
        assert_eq!(Uint64::zero().to_le_bytes(), [0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            Uint64::MAX.to_le_bytes(),
            [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]
        );
        assert_eq!(Uint64::new(1).to_le_bytes(), [1, 0, 0, 0, 0, 0, 0, 0]);
        // Python: `[b for b in (240282366920938463463374607431768124608).to_bytes(16, "little")]`
        assert_eq!(
            Uint64::new(874607431768124608).to_le_bytes(),
            [192, 172, 116, 72, 211, 58, 35, 12]
        );
    }

    #[test]
    fn uint64_is_zero_works() {
        assert!(Uint64::zero().is_zero());
        assert!(Uint64(0).is_zero());

        assert!(!Uint64(1).is_zero());
        assert!(!Uint64(123).is_zero());
    }

    #[test]
    fn uint64_json() {
        let orig = Uint64(1234567890987654321);
        let serialized = to_vec(&orig).unwrap();
        assert_eq!(serialized.as_slice(), b"\"1234567890987654321\"");
        let parsed: Uint64 = from_slice(&serialized).unwrap();
        assert_eq!(parsed, orig);
    }

    #[test]
    fn uint64_compare() {
        let a = Uint64(12345);
        let b = Uint64(23456);

        assert!(a < b);
        assert!(b > a);
        assert_eq!(a, Uint64(12345));
    }

    #[test]
    #[allow(clippy::op_ref)]
    fn uint64_math() {
        let a = Uint64(12345);
        let b = Uint64(23456);

        // test + with owned and reference right hand side
        assert_eq!(a + b, Uint64(35801));
        assert_eq!(a + &b, Uint64(35801));

        // test - with owned and reference right hand side
        assert_eq!((b.checked_sub(a)).unwrap(), Uint64(11111));

        // test += with owned and reference right hand side
        let mut c = Uint64(300000);
        c += b;
        assert_eq!(c, Uint64(323456));
        let mut d = Uint64(300000);
        d += &b;
        assert_eq!(d, Uint64(323456));

        // error result on underflow (- would produce negative result)
        let underflow_result = a.checked_sub(b);
        let OverflowError {
            operand1, operand2, ..
        } = underflow_result.unwrap_err();
        assert_eq!((operand1, operand2), (a.to_string(), b.to_string()));
    }

    #[test]
    #[allow(clippy::op_ref)]
    fn uint64_sub_works() {
        assert_eq!(Uint64(2) - Uint64(1), Uint64(1));
        assert_eq!(Uint64(2) - Uint64(0), Uint64(2));
        assert_eq!(Uint64(2) - Uint64(2), Uint64(0));

        // works for refs
        let a = Uint64::new(10);
        let b = Uint64::new(3);
        let expected = Uint64::new(7);
        assert_eq!(a - b, expected);
        assert_eq!(a - &b, expected);
        assert_eq!(&a - b, expected);
        assert_eq!(&a - &b, expected);
    }

    #[test]
    #[should_panic]
    fn uint64_sub_overflow_panics() {
        let _ = Uint64(1) - Uint64(2);
    }

    #[test]
    fn uint64_sub_assign_works() {
        let mut a = Uint64(14);
        a -= Uint64(2);
        assert_eq!(a, Uint64(12));

        // works for refs
        let mut a = Uint64::new(10);
        let b = Uint64::new(3);
        let expected = Uint64::new(7);
        a -= &b;
        assert_eq!(a, expected);
    }

    #[test]
    #[allow(clippy::op_ref)]
    fn uint64_mul_works() {
        assert_eq!(Uint64::from(2u32) * Uint64::from(3u32), Uint64::from(6u32));
        assert_eq!(Uint64::from(2u32) * Uint64::zero(), Uint64::zero());

        // works for refs
        let a = Uint64::from(11u32);
        let b = Uint64::from(3u32);
        let expected = Uint64::from(33u32);
        assert_eq!(a * b, expected);
        assert_eq!(a * &b, expected);
        assert_eq!(&a * b, expected);
        assert_eq!(&a * &b, expected);
    }

    #[test]
    fn uint64_mul_assign_works() {
        let mut a = Uint64::from(14u32);
        a *= Uint64::from(2u32);
        assert_eq!(a, Uint64::from(28u32));

        // works for refs
        let mut a = Uint64::from(10u32);
        let b = Uint64::from(3u32);
        a *= &b;
        assert_eq!(a, Uint64::from(30u32));
    }

    #[test]
    fn uint64_pow_works() {
        assert_eq!(Uint64::from(2u32).pow(2), Uint64::from(4u32));
        assert_eq!(Uint64::from(2u32).pow(10), Uint64::from(1024u32));
    }

    #[test]
    #[should_panic]
    fn uint64_pow_overflow_panics() {
        Uint64::MAX.pow(2u32);
    }

    #[test]
    #[should_panic]
    fn uint64_math_overflow_panics() {
        // almost_max is 2^64 - 10
        let almost_max = Uint64(18446744073709551606);
        let _ = almost_max + Uint64(12);
    }

    #[test]
    fn uint64_multiply_ratio_works() {
        let base = Uint64(500);

        // factor 1/1
        assert_eq!(base.multiply_ratio(1u64, 1u64), base);
        assert_eq!(base.multiply_ratio(3u64, 3u64), base);
        assert_eq!(base.multiply_ratio(654321u64, 654321u64), base);
        assert_eq!(base.multiply_ratio(u64::MAX, u64::MAX), base);

        // factor 3/2
        assert_eq!(base.multiply_ratio(3u64, 2u64), Uint64(750));
        assert_eq!(base.multiply_ratio(333333u64, 222222u64), Uint64(750));

        // factor 2/3 (integer devision always floors the result)
        assert_eq!(base.multiply_ratio(2u64, 3u64), Uint64(333));
        assert_eq!(base.multiply_ratio(222222u64, 333333u64), Uint64(333));

        // factor 5/6 (integer devision always floors the result)
        assert_eq!(base.multiply_ratio(5u64, 6u64), Uint64(416));
        assert_eq!(base.multiply_ratio(100u64, 120u64), Uint64(416));
    }

    #[test]
    fn uint64_multiply_ratio_does_not_overflow_when_result_fits() {
        // Almost max value for Uint64.
        let base = Uint64(u64::MAX - 9);

        assert_eq!(base.multiply_ratio(2u64, 2u64), base);
    }

    #[test]
    #[should_panic]
    fn uint64_multiply_ratio_panicks_on_overflow() {
        // Almost max value for Uint64.
        let base = Uint64(u64::MAX - 9);

        assert_eq!(base.multiply_ratio(2u64, 1u64), base);
    }

    #[test]
    #[should_panic(expected = "Denominator must not be zero")]
    fn uint64_multiply_ratio_panics_for_zero_denominator() {
        Uint64(500).multiply_ratio(1u64, 0u64);
    }

    #[test]
    fn uint64_checked_multiply_ratio_does_not_panic() {
        assert_eq!(
            Uint64(500u64).checked_multiply_ratio(1u64, 0u64),
            Err(CheckedMultiplyRatioError::DivideByZero),
        );
        assert_eq!(
            Uint64(500u64).checked_multiply_ratio(u64::MAX, 1u64),
            Err(CheckedMultiplyRatioError::Overflow),
        );
    }

    #[test]
    fn sum_works() {
        let nums = vec![Uint64(17), Uint64(123), Uint64(540), Uint64(82)];
        let expected = Uint64(762);

        let sum_as_ref: Uint64 = nums.iter().sum();
        assert_eq!(expected, sum_as_ref);

        let sum_as_owned: Uint64 = nums.into_iter().sum();
        assert_eq!(expected, sum_as_owned);
    }

    #[test]
    fn uint64_methods() {
        // checked_*
        assert!(matches!(
            Uint64::MAX.checked_add(Uint64(1)),
            Err(OverflowError { .. })
        ));
        assert!(matches!(Uint64(1).checked_add(Uint64(1)), Ok(Uint64(2))));
        assert!(matches!(
            Uint64(0).checked_sub(Uint64(1)),
            Err(OverflowError { .. })
        ));
        assert!(matches!(Uint64(2).checked_sub(Uint64(1)), Ok(Uint64(1))));
        assert!(matches!(
            Uint64::MAX.checked_mul(Uint64(2)),
            Err(OverflowError { .. })
        ));
        assert!(matches!(Uint64(2).checked_mul(Uint64(2)), Ok(Uint64(4))));
        assert!(matches!(
            Uint64::MAX.checked_pow(2u32),
            Err(OverflowError { .. })
        ));
        assert!(matches!(Uint64(2).checked_pow(3), Ok(Uint64(8))));
        assert!(matches!(
            Uint64::MAX.checked_div(Uint64(0)),
            Err(DivideByZeroError { .. })
        ));
        assert!(matches!(Uint64(6).checked_div(Uint64(2)), Ok(Uint64(3))));
        assert!(matches!(
            Uint64::MAX.checked_div_euclid(Uint64(0)),
            Err(DivideByZeroError { .. })
        ));
        assert!(matches!(
            Uint64(6).checked_div_euclid(Uint64(2)),
            Ok(Uint64(3)),
        ));
        assert!(matches!(
            Uint64::MAX.checked_rem(Uint64(0)),
            Err(DivideByZeroError { .. })
        ));
        assert!(matches!(Uint64(7).checked_rem(Uint64(2)), Ok(Uint64(1))));

        // saturating_*
        assert_eq!(Uint64::MAX.saturating_add(Uint64(1)), Uint64::MAX);
        assert_eq!(Uint64(0).saturating_sub(Uint64(1)), Uint64(0));
        assert_eq!(Uint64::MAX.saturating_mul(Uint64(2)), Uint64::MAX);
        assert_eq!(Uint64::MAX.saturating_pow(2), Uint64::MAX);
    }

    #[test]
    fn uint64_wrapping_methods() {
        // wrapping_add
        assert_eq!(Uint64(2).wrapping_add(Uint64(2)), Uint64(4)); // non-wrapping
        assert_eq!(Uint64::MAX.wrapping_add(Uint64(1)), Uint64(0)); // wrapping

        // wrapping_sub
        assert_eq!(Uint64(7).wrapping_sub(Uint64(5)), Uint64(2)); // non-wrapping
        assert_eq!(Uint64(0).wrapping_sub(Uint64(1)), Uint64::MAX); // wrapping

        // wrapping_mul
        assert_eq!(Uint64(3).wrapping_mul(Uint64(2)), Uint64(6)); // non-wrapping
        assert_eq!(
            Uint64::MAX.wrapping_mul(Uint64(2)),
            Uint64::MAX - Uint64::one()
        ); // wrapping

        // wrapping_pow
        assert_eq!(Uint64(2).wrapping_pow(3), Uint64(8)); // non-wrapping
        assert_eq!(Uint64::MAX.wrapping_pow(2), Uint64(1)); // wrapping
    }

    #[test]
    #[allow(clippy::op_ref)]
    fn uint64_implements_rem() {
        let a = Uint64::new(10);
        assert_eq!(a % Uint64::new(10), Uint64::zero());
        assert_eq!(a % Uint64::new(2), Uint64::zero());
        assert_eq!(a % Uint64::new(1), Uint64::zero());
        assert_eq!(a % Uint64::new(3), Uint64::new(1));
        assert_eq!(a % Uint64::new(4), Uint64::new(2));

        // works for refs
        let a = Uint64::new(10);
        let b = Uint64::new(3);
        let expected = Uint64::new(1);
        assert_eq!(a % b, expected);
        assert_eq!(a % &b, expected);
        assert_eq!(&a % b, expected);
        assert_eq!(&a % &b, expected);
    }

    #[test]
    #[should_panic(expected = "divisor of zero")]
    fn uint64_rem_panics_for_zero() {
        let _ = Uint64::new(10) % Uint64::zero();
    }

    #[test]
    #[allow(clippy::op_ref)]
    fn uint64_rem_works() {
        assert_eq!(
            Uint64::from(12u32) % Uint64::from(10u32),
            Uint64::from(2u32)
        );
        assert_eq!(Uint64::from(50u32) % Uint64::from(5u32), Uint64::zero());

        // works for refs
        let a = Uint64::from(42u32);
        let b = Uint64::from(5u32);
        let expected = Uint64::from(2u32);
        assert_eq!(a % b, expected);
        assert_eq!(a % &b, expected);
        assert_eq!(&a % b, expected);
        assert_eq!(&a % &b, expected);
    }

    #[test]
    fn uint64_rem_assign_works() {
        let mut a = Uint64::from(30u32);
        a %= Uint64::from(4u32);
        assert_eq!(a, Uint64::from(2u32));

        // works for refs
        let mut a = Uint64::from(25u32);
        let b = Uint64::from(6u32);
        a %= &b;
        assert_eq!(a, Uint64::from(1u32));
    }

    #[test]
    fn uint64_abs_diff_works() {
        let a = Uint64::from(42u32);
        let b = Uint64::from(5u32);
        let expected = Uint64::from(37u32);
        assert_eq!(a.abs_diff(b), expected);
        assert_eq!(b.abs_diff(a), expected);
    }

    #[test]
    fn uint64_partial_eq() {
        let test_cases = [(1, 1, true), (42, 42, true), (42, 24, false), (0, 0, true)]
            .into_iter()
            .map(|(lhs, rhs, expected)| (Uint64::new(lhs), Uint64::new(rhs), expected));

        #[allow(clippy::op_ref)]
        for (lhs, rhs, expected) in test_cases {
            assert_eq!(lhs == rhs, expected);
            assert_eq!(&lhs == rhs, expected);
            assert_eq!(lhs == &rhs, expected);
            assert_eq!(&lhs == &rhs, expected);
        }
    }
}
