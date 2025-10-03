//! Benchmark for string handling performance
//!
//! This benchmark compares different string handling approaches:
//! 1. Current implementation (with scratch buffer for some functions)
//! 2. Zero-copy implementation (using begin/end pointers)
//! 3. Future string_view implementation (when available)
//!
//! Run with: cargo bench --bench string_handling

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

// Note: These benchmarks require a running ImGui context
// For now, we'll benchmark the string preparation overhead

/// Benchmark string preparation for C FFI
fn bench_string_preparation(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_preparation");

    // Test different string lengths
    let test_strings = vec![
        ("short", "Hello"),
        ("medium", &"A".repeat(64)),
        ("long", &"A".repeat(256)),
        ("very_long", &"A".repeat(1024)),
    ];

    for (name, text) in test_strings.iter() {
        // Benchmark 1: Creating CString (current approach for some functions)
        group.bench_with_input(
            BenchmarkId::new("cstring", name),
            text,
            |b, text| {
                b.iter(|| {
                    let c_str = std::ffi::CString::new(*text).unwrap();
                    black_box(c_str.as_ptr());
                });
            },
        );

        // Benchmark 2: Direct pointer access (zero-copy approach)
        group.bench_with_input(
            BenchmarkId::new("zero_copy", name),
            text,
            |b, text| {
                b.iter(|| {
                    let begin = text.as_ptr();
                    let end = unsafe { begin.add(text.len()) };
                    black_box((begin, end));
                });
            },
        );

        // Benchmark 3: Scratch buffer approach (current implementation)
        group.bench_with_input(
            BenchmarkId::new("scratch_buffer", name),
            text,
            |b, text| {
                let mut buffer = Vec::with_capacity(256);
                b.iter(|| {
                    buffer.clear();
                    buffer.extend_from_slice(text.as_bytes());
                    buffer.push(0); // null terminator
                    black_box(buffer.as_ptr());
                });
            },
        );
    }

    group.finish();
}

/// Benchmark repeated string operations (simulating UI rendering loop)
fn bench_repeated_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("repeated_operations");

    // Simulate rendering 100 text widgets per frame
    let widget_count = 100;
    let labels: Vec<String> = (0..widget_count)
        .map(|i| format!("Widget {}", i))
        .collect();

    // Benchmark 1: CString allocation per widget
    group.bench_function("cstring_per_widget", |b| {
        b.iter(|| {
            for label in &labels {
                let c_str = std::ffi::CString::new(label.as_str()).unwrap();
                black_box(c_str.as_ptr());
            }
        });
    });

    // Benchmark 2: Zero-copy approach
    group.bench_function("zero_copy_per_widget", |b| {
        b.iter(|| {
            for label in &labels {
                let begin = label.as_ptr();
                let end = unsafe { begin.add(label.len()) };
                black_box((begin, end));
            }
        });
    });

    // Benchmark 3: Shared scratch buffer
    group.bench_function("shared_scratch_buffer", |b| {
        let mut buffer = Vec::with_capacity(1024);
        b.iter(|| {
            for label in &labels {
                buffer.clear();
                buffer.extend_from_slice(label.as_bytes());
                buffer.push(0);
                black_box(buffer.as_ptr());
            }
        });
    });

    group.finish();
}

/// Benchmark memory allocation patterns
fn bench_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_patterns");

    let text = "Sample text for benchmarking";

    // Benchmark 1: Stack allocation (zero-copy)
    group.bench_function("stack_zero_copy", |b| {
        b.iter(|| {
            let begin = text.as_ptr();
            let end = unsafe { begin.add(text.len()) };
            black_box((begin, end));
        });
    });

    // Benchmark 2: Heap allocation (CString)
    group.bench_function("heap_cstring", |b| {
        b.iter(|| {
            let c_str = std::ffi::CString::new(text).unwrap();
            black_box(c_str);
        });
    });

    // Benchmark 3: Pre-allocated buffer reuse
    group.bench_function("buffer_reuse", |b| {
        let mut buffer = Vec::with_capacity(256);
        b.iter(|| {
            buffer.clear();
            buffer.extend_from_slice(text.as_bytes());
            buffer.push(0);
            black_box(buffer.as_ptr());
        });
    });

    group.finish();
}

/// Benchmark UTF-8 validation overhead
fn bench_utf8_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("utf8_validation");

    let ascii_text = "Hello, World!";
    let unicode_text = "Hello, 世界! 🌍";

    // ASCII text
    group.bench_function("ascii_validation", |b| {
        b.iter(|| {
            let _ = std::str::from_utf8(ascii_text.as_bytes());
            black_box(ascii_text);
        });
    });

    // Unicode text
    group.bench_function("unicode_validation", |b| {
        b.iter(|| {
            let _ = std::str::from_utf8(unicode_text.as_bytes());
            black_box(unicode_text);
        });
    });

    // No validation (unsafe, but what C does)
    group.bench_function("no_validation", |b| {
        b.iter(|| {
            let bytes = ascii_text.as_bytes();
            black_box(bytes.as_ptr());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_string_preparation,
    bench_repeated_operations,
    bench_memory_patterns,
    bench_utf8_validation
);
criterion_main!(benches);

