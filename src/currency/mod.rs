#[derive(Debug)]
pub struct Currency {
    symbol: char,
    exponent_seperator: char,
    digit_seperator: char,
    exponent: u8,
}

pub const GBP: &Currency = &Currency {
    symbol: '£',
    exponent_seperator: '.',
    digit_seperator: ',',
    exponent: 2,
};

#[derive(Debug)]
pub struct Money<'a> {
    currency: &'a Currency,
    total: u64,
}

impl<'a> Money<'a> {
    pub fn from_str(s: &str, currency: &'a Currency) -> Result<Self, String> {
        let s = if s.starts_with(currency.symbol) {
            s.chars().skip(1).collect::<String>()
        } else {
            s.to_string()
        };

        let mut xs = s.split(currency.exponent_seperator).collect::<Vec<_>>();

        if xs.len() == 1 {
            xs.push("00");
        } else if xs.len() > 2 {
            return Err("Invalid number of parts".into());
        };

        let major = xs[0].split(currency.digit_seperator).collect::<String>();
        let minor = format!("{:0<1$}", xs[1], currency.exponent.into());
        let total = format!("{major}{minor}");

        Ok(Money {
            currency,
            total: total.parse().map_err(|_| "Failed to parse total")?,
        })
    }
}

impl std::convert::From<&Money<'_>> for u64 {
    fn from(value: &Money<'_>) -> Self {
        value.total
    }
}

impl std::fmt::Display for Money<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let scale = 10u64.pow(self.currency.exponent.into());
        let major = self.total / scale;
        let minor = self.total % scale;
        write!(
            f,
            "{}{}{}{:0>4$}",
            self.currency.symbol,
            major,
            self.currency.exponent_seperator,
            minor,
            self.currency.exponent.into(),
        )
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

    #[test]
    fn it_implements_display() {
        let gbp = Currency {
            symbol: '£',
            exponent_seperator: '.',
            digit_seperator: ',',
            exponent: 2,
        };

        let Ok(money) = Money::from_str("£1,234.05", &gbp) else {
            panic!("Failed to parse money");
        };

        assert!(money.to_string() == "£1234.05");
    }
}
