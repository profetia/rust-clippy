//! Each case uses its own lint, named in its comment, and says which compilations of
//! `cargo clippy --all-targets` report it.

pub mod after_tests;
pub mod shared;

/// Shared code, reported by the library only (`needless_return`).
pub fn shared_fn() -> u8 {
    return 1;
}

/// Shared only while feature `on` is enabled, reported by the library only (`single_match`).
#[cfg(any(test, feature = "on"))]
pub fn any_test_or_on(x: Option<u8>) {
    match x {
        Some(v) => println!("{v}"),
        _ => {},
    }
}

/// Test-only, since feature `off` is disabled; reported by the unit-test build
/// (`needless_range_loop`).
#[cfg(any(test, feature = "off"))]
pub fn any_test_or_off(v: &[u8]) -> u8 {
    let mut sum = 0;
    for i in 0..v.len() {
        sum += v[i];
    }
    sum
}

/// Test-only; reported by the unit-test build (`assign_op_pattern`).
#[cfg(all(test, feature = "on"))]
fn all_test_and_on(mut a: u8) -> u8 {
    a = a + 1;
    a
}

/// Inline test module; reported by the unit-test build (`bool_comparison`).
#[cfg(test)]
mod inline_tests {
    #[test]
    fn inline() {
        let x = super::all_test_and_on(1) == 2;
        if x == true {}
    }
}

/// Out-of-line test module; reported by the unit-test build (`len_zero`).
#[cfg(test)]
mod file_tests;

/// Fulfilled expectation in shared code; reported by no compilation.
#[expect(clippy::needless_return)]
pub fn expect_fulfilled() -> u8 {
    return 1;
}

/// Unfulfilled expectation in shared code; reported by the library only
/// (`unfulfilled_lint_expectations`).
#[expect(clippy::needless_return)]
pub fn expect_unfulfilled_shared() -> u8 {
    1
}

/// Unfulfilled expectation in test-only code; reported by the unit-test build
/// (`unfulfilled_lint_expectations`).
#[cfg(test)]
#[expect(clippy::needless_return)]
fn expect_unfulfilled_test() -> u8 {
    1
}
