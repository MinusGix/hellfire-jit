# Hellfire-JIT
This is a probably abandoned project.  
The idea was to have a way to specify actions that should be taken with some data, and specify things which are held constant, which it would then use to perform optimizations with the generated code.  
```rust
// Create a query that modifies a u64, by subtracting and then adding
let query: ModifyInPlace<u64, (Sub<u64>, Add<u64>)> =
    ModifyInPlace::new((Sub::new(2u64), Add::new(1)));
// Instantiate the function to operate on a slice of u64
let func = query
    // count allows us to specify a requirement that the slice has a specific size
    // in order to facilitate optimizations
    .instantiate_slice(InstantiateSliceOptions { count: Some(count) })
    .unwrap();


println!("Data Before: {:?}", data);
// Calling this function is safe because it asserts that the count is correct
// and it is given mutable (unique) access to it
func.call_mut(&mut data);
println!("Data After:  {:?}", data);
```
That's basically all that exists in it. If this project was continued, more complex ways of specifying actions (such as using a compile-time 'reflection' crate to allow efficient field-setting) could be made.  
Sadly, Cranelift (which is what I chose, due to it being in Rust), doesn't perform many optimizations. The above code takes in 14 elements and generates this ir:
```
function u0:0(i64, i64) system_v {
block0(v0: i64, v1: i64):
    v5 -> v0
    v2 = iconst.i64 0
    v3 = iconst.i64 14
    jump block1(v2)

block1(v4: i64):
    br_icmp uge v4, v3, block3
    jump block2

block2:
    v6 = imul_imm.i64 v4, 8
    v7 = iadd v6, v5
    v8 = load.i64 v7
    v9 = iconst.i64 1
    v10 = iadd v8, v9
    store v10, v7
    v11 = load.i64 v7
    v12 = iconst.i64 2
    v13 = isub v11, v12
    store v13, v7
    v14 = iadd_imm.i64 v4, 1
    jump block1(v14)

block3:
    return
}
```
Unfortunately, Cranelift doesn't optmize it that well, and it becomes a for loop like:
```C
for (int i = 0; i < 14; i++) {
    data[i] = data[i] - 2;
    data[i] = data[i] + 1;
}
```
without collapsing those add/subs or unrolling the loop.