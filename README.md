# Hellfire-JIT
**NOTE: This library likely has undefined behavior if it is used incorrectly. That will hopefully be made safer as time goes on.**

The idea behind this project is for generating code at *run-time* with some given information about what it can assume.  
The library works by including LLVM and using that to generate and optimize code. This is obviously not cheap in terms of the dependencies that one must include, and not cheap in terms of generating code, but it works.  
This project initially used Cranelift, which has benefits of being pure-rust (and so harder to mess up the API, though still entirely possible to mess up the generated code), but doesn't seem to be good enough at optimizations for this library.  
Obviously, generating code is not an option always, but it could help get some extra performance out of complex actions if you do them often enough to offset the cost of generating code. That said, since LLVM is not exactly made for the very fast compilation that JITs rather like, this library is more of a proof-of-concept and probably is not something you typically want to use.  
  
## Example Usage:
```rust
// Read in some runtime data. In this case, space separated integers
let data = std::fs::read_to_string("./data.txt").unwrap();
let mut data = data
    .split(' ')
    .map(|x| x.parse::<u64>())
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
let count = data.len();
// A query that modifies a u64, subtracting and adding some amount
let query: ModifyInPlace<u64, (Sub<u64>, Add<u64>)> =
    ModifyInPlace::new((Sub::new(2u64), Add::new(1)));
// Instantiate the function to operate over a slice, so operate on &[u64]
let func: InstantiatedSliceModifyInPlace<u64> = query
    .instantiate_slice(InstantiateSliceOptions {
        // Whether to log some information, like the initially generated IR and the IR after optimization
        log: true,
        // A statically known count. This specifies the expected length that the slice will be, and so LLVM
        // can optimize based on that.
        count: Some(count),
    })
    .unwrap();

// Open GDB, start, and then press next until you see the ptr
// then you can do `disas 0xblah` to get the assembly code
// Though, as the methods comments mention, calling the function pointer directly is unsafe
println!(
    "Function Pointer: 0x{:X}",
    func.get_func_unchecked() as *const () as usize
);

println!("Data Before: {:?}", data);
// However, this is safe to call (assuming the generated code isn't bad, but it tries not to be)
// because it asserts that the requirements are met.
// It would be nice to have some typesafe way of constructing a wrapper type.
func.call_mut(&mut data);
println!("Data After:  {:?}", data);
```
As long as the `func` instance is alive, the function pointer can be repeatedly called.  
The query can also be reused.  
