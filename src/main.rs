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
        .instantiate_slice(InstantiateSliceOptions { count: Some(count) })
        .unwrap();

    println!("Data Before: {:?}", data);
    func.call_mut(&mut data);
    println!("Data After:  {:?}", data);

    unsafe {
        let func_ptr = func.get_func() as *const u8;
        let func_size = func.size as usize;

        println!("Starting at {:x?} for {} bytes", func_ptr, func_size);

        let mut code = Vec::new();
        for off in 0..func_size {
            let func_ptr = func_ptr.add(off);
            let val = std::ptr::read(func_ptr);
            code.push(val);
        }

        for val in code.iter() {
            print!("\\x{:02X?}", val);
        }
        println!("");
    }
}
