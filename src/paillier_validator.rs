/*
Description of the Vulnerability

The core of the vulnerability lies in improper validation of the Paillier modulus NN. Specifically:
- Lack of checks for small prime factors in NN.
- No verification that NN is a biprime (product of two primes).

Why is this a problem?

An attacker can craft a malicious Paillier modulus NN with known small prime factors. This malicious NN can then be exploited to:
- Leak partial information about other parties' secret shares during the protocol execution.
- Reconstruct the full private key by combining leaked partial information.
*/

use curv::arithmetic::Integer;
use curv::BigInt;
use log::{error, info};
use num_traits::Zero;

#[derive(Debug, PartialEq)]
pub enum ValidationResult {
    Valid,
}

pub struct PaillierValidator {
    max_small_prime: u64,
}

impl PaillierValidator {
    pub fn new(max_small_prime: u64) -> Self {
        Self { max_small_prime }
    }

    pub fn validate_modulus(&self, nn: &BigInt) -> anyhow::Result<ValidationResult> {
        if self.has_small_prime_factors(nn) {
            let msg = format!("{} has small prime factors, validation shall fail", nn);
            error!("{}", msg)
        }
        if self.is_prime(nn) {
            let msg = format!("{} is prime, validation shall fail", nn);
            error!("{}", msg)
        }
        Ok(ValidationResult::Valid)
    }

    fn has_small_prime_factors(&self, nn: &BigInt) -> bool {
        for p in 2..=self.max_small_prime {
            let p_bigint = BigInt::from(p);
            if nn.mod_floor(&p_bigint) == BigInt::zero() {
                info!("{} has small prime factor {}", nn, p);
                return true;
            }
        }
        info!("{} has no small prime factors", nn);
        false
    }

    fn is_prime(&self, _nn: &BigInt) -> bool {
        // TODO: implement actual verification logic
        // probably based on Miller-Rabin primality test
        return false;
    }
}
