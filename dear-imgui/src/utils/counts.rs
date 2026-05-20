pub(super) fn non_negative_count_from_i32(caller: &str, raw: i32) -> usize {
    usize::try_from(raw).unwrap_or_else(|_| panic!("{caller} returned a negative count"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn non_negative_count_conversion_rejects_negative_values() {
        assert_eq!(super::non_negative_count_from_i32("test", 7), 7);
        assert!(
            std::panic::catch_unwind(|| {
                let _ = super::non_negative_count_from_i32("test", -1);
            })
            .is_err()
        );
    }
}
