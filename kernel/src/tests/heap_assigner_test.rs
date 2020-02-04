/// Heap assigner tests.
#[cfg(feature = "test-process-heap-assigner-tests")]
pub fn test_main() {
    crate::mm::process_heap_assigner::test_main();
}
