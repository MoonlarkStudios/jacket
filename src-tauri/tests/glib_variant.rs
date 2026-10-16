//! Run optimized: the upstream defect can be hidden by debug builds.
#![cfg(target_os = "linux")]

use glib::variant::ToVariant;

#[test]
fn variant_string_iterators_preserve_c_output_pointers_under_optimization() {
    let strings = ["first", "Björk — 東京", "third", "last"];
    let value = strings.to_variant();
    assert_eq!(value.array_iter_str().unwrap().collect::<Vec<_>>(), strings);
    let mut iter = value.array_iter_str().unwrap();
    assert_eq!(iter.next(), Some("first"));
    assert_eq!(iter.next_back(), Some("last"));
    assert_eq!(iter.nth(1), Some("third"));
    assert_eq!(iter.next(), None);
    assert_eq!(value.array_iter_str().unwrap().last(), Some("last"));
    assert_eq!(value.array_iter_str().unwrap().nth_back(1), Some("third"));
    assert_eq!(
        value.array_iter_str().unwrap().rev().collect::<Vec<_>>(),
        strings.into_iter().rev().collect::<Vec<_>>()
    );
    let empty = Vec::<String>::new().to_variant();
    assert_eq!(empty.array_iter_str().unwrap().next(), None);
}
