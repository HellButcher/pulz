#[diagnostic::on_unimplemented(message = "`{Self}` is not a tuple")]
pub trait Tuple {
    const LEN: usize;
}

macro_rules! impl_tuple {
    ([$(($big:ident,$small:ident,$index:tt)),*]) => {
        maybe_tuple_doc! {
            $($big)* @

            /// This trait is implemented for tuples up to 21 items long
            impl<$($big),*> Tuple for ($($big,)*) {
                const LEN: usize = 0 $( + 1 + ($index - $index))*;
            }
        }
    };
}

crate::generate_variadic_array! {[21 T,t,#] impl_tuple!{}}

#[cfg(feature = "tuple-ops")]
pub mod ops;

#[cfg(feature = "tuple-map")]
pub mod map;

#[cfg(feature = "tuple-convert")]
pub mod convert;
