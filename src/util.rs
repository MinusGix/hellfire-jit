use cranelift::{
    frontend::{FunctionBuilder, Variable},
    prelude::{
        types::{I128, I16, I32, I64, I8},
        InstBuilder, IntCC, MemFlags, Type, Value,
    },
};
use cranelift_jit::JITModule;

use crate::{instantiate::FunctionTranslator, jit::JITInit};

/// A value 'literal' or a variable
/// This exists for functions which want a value but have so it may change and so the variables
/// need to be gotten at a specific point so that they'll update
pub(crate) trait ValueVar {
    fn get(&self, builder: &mut FunctionBuilder) -> Value;
}
impl ValueVar for Value {
    fn get(&self, _: &mut FunctionBuilder) -> Value {
        *self
    }
}
impl ValueVar for Variable {
    fn get(&self, builder: &mut FunctionBuilder) -> Value {
        builder.use_var(*self)
    }
}

/// `builder`: For building the code
/// `module`: A hacky workaround because I couldn't get the lifetimes working so we pass it into
///     this so that the closure can use it.
/// `data_ptr_val`: The pointer to the start of the array
/// `index_val`: The current index
/// `length_val`: The length of the array
/// `element_size`: The size in memory of the element
/// `gen_code`: A callback used to generate the code inside the body of the loop. This SHOULD handle
///     the incrementing of the index in memory.
///     The function receives the builder and a value which has the pointer to the current element
///     (&mut FunctionBuilder, &mut JITModule, index_val: Value, element_ptr_val: Value)
pub(crate) fn make_index_loop<'a, F>(
    builder: &mut FunctionBuilder<'a>,
    module: &'a mut JITModule,
    data_ptr: impl ValueVar,
    index: impl ValueVar,
    length: impl ValueVar,
    element_size: i64,
    gen_body_code: F,
) where
    F: FnOnce(&mut FunctionBuilder<'a>, &'a mut JITModule, Value, Value),
{
    let header_block = builder.create_block();
    let body_block = builder.create_block();
    let exit_block = builder.create_block();

    // Jump into the loop check from our current block
    builder.ins().jump(header_block, &[]);

    // Check the condition and exit the loop if index_val >= length_val
    builder.switch_to_block(header_block);

    let index_val = index.get(builder);
    let length_val = length.get(builder);
    builder.ins().br_icmp(
        IntCC::UnsignedGreaterThanOrEqual,
        index_val,
        length_val,
        exit_block,
        &[],
    );

    // We passed the condition, so go to the body
    builder.ins().jump(body_block, &[]);

    builder.switch_to_block(body_block);
    // We've created all the entries to the body block
    builder.seal_block(body_block);

    // i * sizeof(element)
    let data_ptr_val = data_ptr.get(builder);
    let offset_val = builder.ins().imul_imm(index_val, element_size);
    let element_ptr_val = builder.ins().iadd(offset_val, data_ptr_val);

    gen_body_code(builder, module, index_val, element_ptr_val);

    // jump back to header to recheck condition
    builder.ins().jump(header_block, &[]);

    builder.switch_to_block(exit_block);
    // We've set up the loop, so we can seal these now.
    builder.seal_block(header_block);
    builder.seal_block(exit_block);
}

macro_rules! tuple_impls_gcod {
    () => {};

    ( ($idx:tt => $typ:ident), $( ($nidx:tt => $ntyp:ident), )* ) => {
        impl<$typ, $( $ntyp ),*> GenerateCode for ($typ, $( $ntyp ),*)
        where
            $typ: GenerateCode,
            $( $ntyp: GenerateCode ),*
        {
            fn generate_code(&self, trans: &mut FunctionTranslator) {
                &self.$idx.generate_code(trans);
                $(
                    &self.$nidx.generate_code(trans);
                )*
            }
        }

        tuple_impls_gcod!($( ($nidx => $ntyp), )*);
    };
}

tuple_impls_gcod!(
    (9 => J),
    (8 => I),
    (7 => H),
    (6 => G),
    (5 => F),
    (4 => E),
    (3 => D),
    (2 => C),
    (1 => B),
    (0 => A),
);

/// This is private to the implementation
pub trait GenerateCode {
    fn generate_code(&self, trans: &mut FunctionTranslator);
}

macro_rules! tuple_impls_init_jit {
    () => {};

    ( ($idx:tt => $typ:ident), $( ($nidx:tt => $ntyp:ident), )* ) => {
        impl<$typ, $( $ntyp ),*> InitJIT for ($typ, $( $ntyp ),*)
        where
            $typ: InitJIT,
            $( $ntyp: InitJIT ),*
        {
            fn init_jit(&self, init: &mut JITInit) {
                &self.$idx.init_jit(init);
                $(
                    &self.$nidx.init_jit(init);
                )*
            }
        }

        tuple_impls_init_jit!($( ($nidx => $ntyp), )*);
    };
}

tuple_impls_init_jit!(
    (9 => J),
    (8 => I),
    (7 => H),
    (6 => G),
    (5 => F),
    (4 => E),
    (3 => D),
    (2 => C),
    (1 => B),
    (0 => A),
);

/// A trait for things that have to occur when creating the JIT
pub trait InitJIT {
    fn init_jit(&self, init: &mut JITInit);
}

pub trait StaticGetType {
    const TYPE: Type;
}
pub trait GetType {
    fn get_type(&self, trans: &mut FunctionTranslator) -> Type;
}
impl<T: StaticGetType> GetType for T {
    fn get_type(&self, _: &mut FunctionTranslator) -> Type {
        T::TYPE
    }
}
impl StaticGetType for u8 {
    const TYPE: Type = I8;
}
impl StaticGetType for i8 {
    const TYPE: Type = I8;
}
impl StaticGetType for u16 {
    const TYPE: Type = I16;
}
impl StaticGetType for i16 {
    const TYPE: Type = I16;
}
impl StaticGetType for u32 {
    const TYPE: Type = I32;
}
impl StaticGetType for i32 {
    const TYPE: Type = I32;
}
impl StaticGetType for u64 {
    const TYPE: Type = I64;
}
impl StaticGetType for i64 {
    const TYPE: Type = I64;
}
impl StaticGetType for u128 {
    const TYPE: Type = I128;
}
impl StaticGetType for i128 {
    const TYPE: Type = I128;
}
impl GetType for usize {
    fn get_type(&self, trans: &mut FunctionTranslator) -> Type {
        trans.usize_ty
    }
}
impl GetType for isize {
    fn get_type(&self, trans: &mut FunctionTranslator) -> Type {
        trans.usize_ty
    }
}

pub trait AsValue {
    fn as_value(&self, trans: &mut FunctionTranslator) -> Value;
}
impl AsValue for u8 {
    fn as_value(&self, trans: &mut FunctionTranslator) -> Value {
        let val: i64 = (*self).into();
        trans.builder.ins().iconst(I8, val)
    }
}
impl AsValue for i8 {
    fn as_value(&self, trans: &mut FunctionTranslator) -> Value {
        let val: i64 = (*self).into();
        trans.builder.ins().iconst(I8, val)
    }
}
impl AsValue for u16 {
    fn as_value(&self, trans: &mut FunctionTranslator) -> Value {
        let val: i64 = (*self).into();
        trans.builder.ins().iconst(I16, val)
    }
}
impl AsValue for i16 {
    fn as_value(&self, trans: &mut FunctionTranslator) -> Value {
        let val: i64 = (*self).into();
        trans.builder.ins().iconst(I16, val)
    }
}
impl AsValue for u32 {
    fn as_value(&self, trans: &mut FunctionTranslator) -> Value {
        let val: i64 = (*self).into();
        trans.builder.ins().iconst(I32, val)
    }
}
impl AsValue for i32 {
    fn as_value(&self, trans: &mut FunctionTranslator) -> Value {
        let val: i64 = (*self).into();
        trans.builder.ins().iconst(I32, val)
    }
}
impl AsValue for u64 {
    fn as_value(&self, trans: &mut FunctionTranslator) -> Value {
        // FIXME: I feel like this isn't guaranteed to be a proper conversion
        let val: i64 = i64::from_ne_bytes(self.to_ne_bytes());
        trans.builder.ins().iconst(I64, val)
    }
}
impl AsValue for i64 {
    fn as_value(&self, trans: &mut FunctionTranslator) -> Value {
        trans.builder.ins().iconst(I64, *self)
    }
}
// TODO: This cast may not work properly for non-64-bit platforms
impl AsValue for usize {
    fn as_value(&self, trans: &mut FunctionTranslator) -> Value {
        // FIXME: I feel like this isn't guaranteed to be a proper conversion
        let val: i64 = i64::from_ne_bytes(self.to_ne_bytes());
        trans.builder.ins().iconst(I64, val)
    }
}
impl AsValue for isize {
    fn as_value(&self, trans: &mut FunctionTranslator) -> Value {
        let val: i64 = i64::from_ne_bytes(self.to_ne_bytes());
        trans.builder.ins().iconst(I64, val)
    }
}
// impl AsValue for u128 {
//     fn as_value(&self, trans: &mut FunctionTranslator) -> Value {
//         // trans.builder.ins().iconst(I128, *self)
//         todo!()
//     }
// }
// impl AsValue for i128 {
//     fn as_value(&self, trans: &mut FunctionTranslator) -> Value {
//         // trans.builder.ins().iconst(I128, *self)
//         // trans.builder.ins().vconst(I128, *self)
//         todo!()
//     }
// }

/// Generate code to copy the value from a value to a pointer
pub trait CopyValueTo {
    fn copy_value_to(trans: &mut FunctionTranslator, dest: Value, src: Value);
}

/// Generate code to copy a value from a pointer out into a Value
pub trait CopyValueOut {
    fn copy_value_out(trans: &mut FunctionTranslator, src: Value) -> Value;
}

macro_rules! impl_copy_value {
    ($($typ:ty),*) => {
        $(
            impl CopyValueTo for $typ {
                fn copy_value_to(trans: &mut FunctionTranslator, dest: Value, src: Value) {
                    trans.builder.ins().store(MemFlags::new(), src, dest, 0);
                }
            }

            impl CopyValueOut for $typ {
                fn copy_value_out(trans: &mut FunctionTranslator, src: Value) -> Value {
                    let typ: Type = <$typ as StaticGetType>::TYPE;
                    trans.builder.ins().load(typ, MemFlags::new(), src, 0)
                }
            }
        )*
    };
}
impl_copy_value!(u8, i8, u16, i16, u32, i32, u64, i64);

macro_rules! impl_marker {
    ($tr:ident; $($typ:ty),*) => {
        $(
            impl $tr for $typ {}
        )*
    }
}

/// Marker trait for signed and unsigned integer types
/// Currently doesn't include because they behave differently in cranelift
pub trait IntegerType {}
impl_marker!(IntegerType; u8, i8, u16, i16, u32, i32, u64, i64);
