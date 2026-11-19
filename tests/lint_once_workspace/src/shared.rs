/// Reported by the library only; the unit-test build sees the test-only `is_empty`
/// (`len_without_is_empty`).
pub struct Len;

impl Len {
    pub fn len(&self) -> usize {
        0
    }

    /// Test-only method in a shared impl; reported by the unit-test build
    /// (`manual_range_contains`).
    #[cfg(test)]
    fn in_range(x: u8) -> bool {
        x >= 1 && x < 5
    }
}

#[cfg(test)]
impl Len {
    pub fn is_empty(&self) -> bool {
        Self::in_range(self.len() as u8)
    }
}

/// Test-only statement in a shared function: reported by no compilation (`identity_op`).
pub fn statement(x: u8) -> u8 {
    #[cfg(test)]
    let x = x + 0;
    x
}

/// `#[test]` function in a shared module; reported by the unit-test build (`unnecessary_cast`).
#[test]
fn test_in_shared_module() {
    let _ = 1u8 as u8;
}
