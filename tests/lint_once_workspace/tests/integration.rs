/// Integration test; reported by its own compilation (`bool_comparison`).
#[test]
fn integration() {
    let x = lint_once::shared_fn() == 1;
    if x == true {}
}
