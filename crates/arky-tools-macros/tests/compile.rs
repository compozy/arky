//! Trybuild coverage for `#[tool]` expansion and diagnostics.

#[test]
fn tool_macro_compile_tests_should_match_expected_results() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/pass/*.rs");
    tests.compile_fail("tests/ui/*.rs");
}
