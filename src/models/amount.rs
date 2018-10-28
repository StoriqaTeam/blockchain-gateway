use std::cmp::{Ord, Ordering};
use std::fmt;
use std::fmt::LowerHex;
use std::mem::transmute;

/// This is a wrapper for monetary amounts in blockchain.
/// You have to be careful that it has a limited amount of 38 significant digits
/// So make sure that total monetary supply of a coin (in satoshis, wei, etc) does not exceed that.
/// It has json and postgres serialization / deserialization implemented.
/// Numeric type from postgres has bigger precision, so you need to impose contraint
/// that your db contains only limited precision numbers, i.e. no floating point and limited by u128 values.
///
/// As a monetary amount it only implements checked_add and checked_sub
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Amount(u128);

impl Amount {
    ///Make addition, return None on overflow
    pub fn checked_add(&self, other: Amount) -> Option<Self> {
        self.0.checked_add(other.0).map(Amount)
    }

    /// Make saubtraction, return None on overflow
    pub fn checked_sub(&self, other: Amount) -> Option<Self> {
        self.0.checked_sub(other.0).map(Amount)
    }

    #[allow(dead_code)]
    pub fn inner(&self) -> u128 {
        self.0
    }

    #[allow(dead_code)]
    pub fn u64(&self) -> Option<u64> {
        if self.0 <= u64::max_value() as u128 {
            Some(self.0 as u64)
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn bytes(&self) -> Vec<u8> {
        let bytes: [u8; 16] = unsafe { transmute(self.0.to_be()) };
        bytes.into_iter().cloned().collect()
    }

    pub fn new(val: u128) -> Self {
        Amount(val)
    }
}

impl Ord for Amount {
    fn cmp(&self, other: &Amount) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for Amount {
    fn partial_cmp(&self, other: &Amount) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl LowerHex for Amount {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        LowerHex::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_serde_conversions() {
        let cases = [
            ("1000000010000000000000000000", 1000000010000000000000000000u128),
            ("354890005000010004355680400034758", 354890005000010004355680400034758u128),
            ("0", 0u128),
            ("1", 1u128),
            ("2", 2u128),
            ("10", 10u128),
            ("9999", 9999u128),
            ("10000", 10000u128),
            ("10001", 10001u128),
            ("11111", 11111u128),
            ("55555555", 55555555u128),
            ("99999999", 99999999u128),
            ("12379871239800000000", 12379871239800000000u128),
            (
                // u128 max value - 1
                "340282366920938463463374607431768211454",
                340282366920938463463374607431768211454u128,
            ),
            (
                "340282366920938463463374607431768211455",
                // u128 max value
                340282366920938463463374607431768211455u128,
            ),
        ];
        for case in cases.into_iter() {
            let (string, number) = case.clone();
            let parsed: Amount = ::serde_json::from_str(string).unwrap();
            assert_eq!(parsed, Amount(number));
        }
    }

    #[test]
    fn test_serde_error_conversions() {
        let error_cases = [
            "-1",
            "-10000",
            "0.1",
            "0.00001",
            "1.1",
            "10000.00001",
            // u128::max_value + 1
            "340282366920938463463374607431768211456",
            // u128::max_value.1
            "340282366920938463463374607431768211455.1",
            // -u128::max_value
            "-340282366920938463463374607431768211455",
            // i128::min_value
            "-170141183460469231731687303715884105728",
            // i128::min_value - 1
            "-170141183460469231731687303715884105729",
        ];
        for case in error_cases.into_iter() {
            let parsed: Result<Amount, _> = ::serde_json::from_str(case);
            assert_eq!(parsed.is_err(), true, "Case: {}", case);
        }
    }

    #[test]
    fn test_checked_ops() {
        assert_eq!(Amount(5).checked_add(Amount(8)), Some(Amount(13)));
        assert_eq!(Amount(u128::max_value()).checked_add(Amount(1)), None);
        assert_eq!(Amount(u128::max_value()).checked_sub(Amount(u128::max_value())), Some(Amount(0)));
        assert_eq!(Amount(13).checked_sub(Amount(11)), Some(Amount(2)));
        assert_eq!(Amount(8).checked_sub(Amount(11)), None);
    }
}
