pub(super) fn columns_count_to_i32(count: usize, caller: &str) -> i32 {
    assert!(count >= 1, "{caller} count must be at least 1");
    i32::try_from(count)
        .unwrap_or_else(|_| panic!("{caller} count exceeded Dear ImGui's i32 range"))
}

pub(super) fn column_count_from_i32(raw: i32, caller: &str) -> usize {
    assert!(raw >= 1, "{caller} returned an invalid legacy column count");
    usize::try_from(raw).expect("positive legacy column count must fit usize")
}
