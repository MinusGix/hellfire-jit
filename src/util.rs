use llama::{BasicBlock, Builder, Context, Type, Value};

use crate::jit::CodeGen;

/// A custom impl of a for loop with a value
/// Does not have the ability to break
/// This is used rather than the one provided by llama, as that one seems to generate code that only
/// checks the condition after at least one loop. It is closer to a do-while loop than a for loop.
pub(crate) fn for_loop<
    'a,
    S: Into<Value<'a>>,
    C: Into<Value<'a>>,
    X: Into<Value<'a>>,
    Step: FnOnce(&Value<'a>) -> Result<S, llama::Error>,
    Cond: FnOnce(&Value<'a>) -> Result<C, llama::Error>,
    Body: FnOnce(&Value<'a>) -> Result<X, llama::Error>,
>(
    build: &Builder<'a>,
    initial_value: impl AsRef<Value<'a>>,
    cond: Cond,
    step: Step,
    body: Body,
    loop_name: &str,
) -> Result<Value<'a>, llama::Error> {
    let ctx = build.context();

    let initial_value = initial_value.as_ref();

    let preheader_block = build.insertion_block()?;
    let function = preheader_block.parent()?;
    let loop_block = BasicBlock::append(ctx, &function, loop_name)?;

    // Enter into the loop
    build.br(loop_block)?;
    build.position_at_end(loop_block);

    // Get the current value, 0 if we're entering the first loop and otherwise next_var
    let mut var = build.phi(initial_value.type_of()?, "cur")?;
    var.add_incoming(&[(*initial_value, preheader_block)]);
    // Check the condition, so we branch away
    let after_block = BasicBlock::append(ctx, &function, "after")?;
    let cond = cond(var.as_ref())?.into();
    build.cond_br(cond, loop_block, after_block)?;

    let body = body(var.as_ref())?;

    let next_var = step(var.as_ref())?.into();

    let loop_end_block = build.insertion_block()?;
    build.position_at_end(after_block);

    var.add_incoming(&[(next_var, loop_end_block)]);

    Ok(body.into())
}

macro_rules! tuple_impls_gcod {
    () => {};

    ( ($idx:tt => $typ:ident), $( ($nidx:tt => $ntyp:ident), )* ) => {
        impl<$typ, $( $ntyp ),*> GenerateCode for ($typ, $( $ntyp ),*)
        where
            $typ: GenerateCode,
            $( $ntyp: GenerateCode ),*
        {
            fn generate_code<'a>(&self, cg: &CodeGen<'a>, data: Value<'a>) -> Result<(), llama::Error> {
                let _ = &self.$idx.generate_code(cg, data)?;
                $(
                    let _ = &self.$nidx.generate_code(cg, data)?;
                )*
                Ok(())
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
    /// Data is a pointer to the value
    fn generate_code<'a>(&self, cg: &CodeGen<'a>, data: Value<'a>) -> Result<(), llama::Error>;
}

pub trait StaticGetType {
    fn static_get_type<'a>(ctx: &Context<'a>) -> Result<Type<'a>, llama::Error>;

    /// Get a pointer to the type
    /// Most code shouldn't need to customize this
    fn static_get_pointer_type<'a>(
        ctx: &Context<'a>,
        address_space: Option<usize>,
    ) -> Result<Type<'a>, llama::Error> {
        Type::pointer(Self::static_get_type(ctx)?, address_space)
    }
}
macro_rules! impl_static_get_type_bits {
    ($($t:ty),*) => {
        $(
            impl StaticGetType for $t {
                fn static_get_type<'a>(ctx: &Context<'a>) -> Result<Type<'a>, llama::Error> {
                    Type::int(ctx, <$t>::BITS as usize)
                }
            }
        )*
    };
}
impl_static_get_type_bits!(u8, i8, u16, i16, u32, i32, u64, i64, usize, isize);

pub trait AsValue {
    fn as_value<'a>(&self, cg: &CodeGen<'a>) -> Result<Value<'a>, llama::Error>;
}
macro_rules! impl_as_value_ints {
    ($($t:ty),*) => {
        $(
            impl AsValue for $t {
                fn as_value<'a>(&self, cg: &CodeGen<'a>) -> Result<Value<'a>, llama::Error> {
                    // FIXME: Is this correct for u64?
                    let val: i64 = *self as i64;
                    let typ = <$t>::static_get_type(&cg.context)?;
                    Ok(Value::from(llama::Const::int(typ, val)?))
                }
            }
        )*
    };
}
impl_as_value_ints!(u8, i8, u16, i16, u32, i32, u64, i64);

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
