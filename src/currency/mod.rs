#[derive(Debug)]
pub struct Currency {
    symbol: char,
    exponent_seperator: char,
    digit_seperator: char,
    exponent: usize,
}

pub const GBP: &Currency = &Currency {
    symbol: '£',
    exponent_seperator: '.',
    digit_seperator: ',',
    exponent: 2,
};

#[derive(Debug)]
pub struct Money {
    total: u64,
}

impl Money {
    pub fn from_str(s: &str, currency: &Currency) -> Result<Self, String> {
        let mut s = s.to_string();
        if s.starts_with(currency.symbol) {
            s = s.chars().skip(1).collect::<String>();
        }

        let mut xs = s.split(currency.exponent_seperator).collect::<Vec<_>>();

        if xs.len() == 1 {
            xs.push("00");
        } else if xs.len() > 2 {
            return Err("Invalid number of parts".into());
        };

        let major = xs[0].split(currency.digit_seperator).collect::<String>();
        let minor = format!("{:0<1$}", xs[1], currency.exponent);
        let total = format!("{major}{minor}");

        Ok(Money {
            total: total.parse().map_err(|_| "Failed to parse total")?,
        })
    }
}

impl From<&Money> for u64 {
    fn from(value: &Money) -> Self {
        value.total
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn it_works_with_a_left_aligned_exponent() {
        let gbp = Currency {
            symbol: '£',
            exponent_seperator: '.',
            digit_seperator: ',',
            exponent: 2,
        };

        let Ok(money) = Money::from_str("£1,000,000.50", &gbp) else {
            panic!("Failed to parse money");
        };

        assert!(money.total == 100000050);
    }

    #[test]
    fn it_works_with_a_right_aligned_exponent() {
        let gbp = Currency {
            symbol: '£',
            exponent_seperator: '.',
            digit_seperator: ',',
            exponent: 2,
        };

        let Ok(money) = Money::from_str("£1,000,000.05", &gbp) else {
            panic!("Failed to parse money");
        };

        assert!(money.total == 100000005);
    }

    #[test]
    fn it_works_with_no_exponent() {
        let gbp = Currency {
            symbol: '£',
            exponent_seperator: '.',
            digit_seperator: ',',
            exponent: 2,
        };

        let Ok(money) = Money::from_str("£1,000,000", &gbp) else {
            panic!("Failed to parse money");
        };

        assert!(money.total == 100000000);
    }

    #[test]
    fn it_works_with_no_formatting() {
        let gbp = Currency {
            symbol: '£',
            exponent_seperator: '.',
            digit_seperator: ',',
            exponent: 2,
        };

        let Ok(money) = Money::from_str("1000000", &gbp) else {
            panic!("Failed to parse money");
        };

        assert!(money.total == 100000000);
    }

    #[test]
    fn it_converts_to_a_u64() {
        let gbp = Currency {
            symbol: '£',
            exponent_seperator: '.',
            digit_seperator: ',',
            exponent: 2,
        };

        let Ok(money) = Money::from_str("1000000", &gbp) else {
            panic!("Failed to parse money");
        };

        assert!(std::convert::Into::<u64>::into(&money) == 100000000);
    }
}
