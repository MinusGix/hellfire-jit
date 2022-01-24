use instantiate::InstantiateSliceOptions;
use wrappers::{Add, ModifyInPlace, Sub};

pub mod instantiate;
pub mod jit;
pub mod util;
pub mod wrappers;

fn main() {
    let data = std::fs::read_to_string("./data.txt").unwrap();
    let mut data = data
        .split(' ')
        .map(|x| x.parse::<u64>())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let count = data.len();
    let query: ModifyInPlace<u64, (Sub<u64>, Add<u64>)> =
        ModifyInPlace::new((Sub::new(2u64), Add::new(1)));
    let func = query
        .instantiate_slice(InstantiateSliceOptions {
            log: true,
            count: Some(count),
        })
        .unwrap();

    println!("Got function");

    // Open GDB, start, and then press next until you see the ptr
    // then you can do `disas 0xblah` to get the assembly code
    println!(
        "Function Pointer: 0x{:X}",
        func.get_func_unchecked() as *const () as usize
    );

    println!("Data Before: {:?}", data);
    func.call_mut(&mut data);
    println!("Data After:  {:?}", data);
}
