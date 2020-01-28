pub mod pmm;
pub mod mapper;
mod alloc;
mod buddy;

pub fn test() {
    buddy::test();
    alloc::test();
}
