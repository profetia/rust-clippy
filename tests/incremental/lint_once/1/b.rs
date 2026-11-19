#[expect(clippy::needless_return)]
pub fn b() -> u8 {
    return 2;
}
