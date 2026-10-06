// Runs against the actual optimized GTK-compatible patched dependency on Linux.
#[cfg(target_os = "linux")]
#[test]
fn optimized_string_variant_iterator_covers_all_advisory_methods() {
    use glib::prelude::*;
    let value = ["zero", "one", "two", "three", "four", "five"]
        .as_slice()
        .to_variant();
    let mut iter = value.array_iter_str().unwrap();
    assert_eq!(iter.next(), Some("zero"));
    assert_eq!(iter.nth(1), Some("two"));
    assert_eq!(iter.next_back(), Some("five"));
    assert_eq!(iter.nth_back(1), Some("three"));
    assert_eq!(iter.next(), None);
    assert_eq!(value.array_iter_str().unwrap().last(), Some("five"));
    let empty = Vec::<String>::new().to_variant();
    assert_eq!(empty.array_iter_str().unwrap().next(), None);
}
