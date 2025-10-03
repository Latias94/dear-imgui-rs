// Simple string handling benchmark for string_view comparison
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::ffi::CString;

fn bench_cstring_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("cstring_creation");
    
    // Short string
    group.bench_function("short_10_chars", |b| {
        let text = "Button123";
        b.iter(|| {
            let _cstr = CString::new(black_box(text)).unwrap();
        });
    });
    
    // Medium string
    group.bench_function("medium_50_chars", |b| {
        let text = "This is a medium length string for testing purposes";
        b.iter(|| {
            let _cstr = CString::new(black_box(text)).unwrap();
        });
    });
    
    // Long string
    group.bench_function("long_200_chars", |b| {
        let text = "This is a very long string that might be used in a text widget or label. It contains multiple sentences and is designed to test the performance of string handling with longer text content. This helps us understand the overhead.";
        b.iter(|| {
            let _cstr = CString::new(black_box(text)).unwrap();
        });
    });
    
    group.finish();
}

fn bench_repeated_cstring(c: &mut Criterion) {
    let mut group = c.benchmark_group("repeated_operations");
    
    // Simulate 100 buttons per frame
    group.bench_function("100_buttons_per_frame", |b| {
        let labels: Vec<String> = (0..100).map(|i| format!("Button {}", i)).collect();
        b.iter(|| {
            for label in &labels {
                let _cstr = CString::new(black_box(label.as_str())).unwrap();
            }
        });
    });
    
    // Simulate 10 text inputs per frame
    group.bench_function("10_text_inputs_per_frame", |b| {
        let texts: Vec<String> = (0..10)
            .map(|i| format!("Input field {} with some default text", i))
            .collect();
        b.iter(|| {
            for text in &texts {
                let _cstr = CString::new(black_box(text.as_str())).unwrap();
            }
        });
    });
    
    group.finish();
}

fn bench_zero_copy_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("zero_copy_simulation");
    
    // Simulate what zero-copy would look like
    group.bench_function("direct_pointer_access", |b| {
        let text = "Button123";
        b.iter(|| {
            let ptr = black_box(text.as_ptr());
            let end = unsafe { black_box(ptr.add(text.len())) };
            // In real zero-copy, we'd pass (ptr, end) directly to ImGui
            (ptr, end)
        });
    });
    
    // Compare with CString creation
    group.bench_function("cstring_creation", |b| {
        let text = "Button123";
        b.iter(|| {
            let _cstr = CString::new(black_box(text)).unwrap();
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_cstring_creation,
    bench_repeated_cstring,
    bench_zero_copy_simulation
);
criterion_main!(benches);

