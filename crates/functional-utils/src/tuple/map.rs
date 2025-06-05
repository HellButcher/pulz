use super::Tuple;

pub trait Mapper<T> {
    type Target;
    fn map(value: T) -> Self::Target;
}

pub enum DerefMapper {}
impl<'a, T: ?Sized> Mapper<&'a T> for DerefMapper
where
    T: std::ops::Deref,
{
    type Target = &'a T::Target;
    #[inline]
    fn map(value: &'a T) -> &'a T::Target {
        value
    }
}
impl<'a, T: ?Sized> Mapper<&'a mut T> for DerefMapper
where
    T: std::ops::Deref,
{
    type Target = &'a T::Target;
    #[inline]
    fn map(value: &'a mut T) -> &'a T::Target {
        value
    }
}

pub enum DerefMutMapper {}
impl<'a, T: ?Sized> Mapper<&'a mut T> for DerefMutMapper
where
    T: std::ops::DerefMut,
{
    type Target = &'a mut T::Target;
    #[inline]
    fn map(value: &'a mut T) -> &'a mut T::Target {
        value
    }
}

pub trait TupleMap<M>: Tuple {
    type Target: Tuple;
    fn map(self) -> Self::Target;
}

macro_rules! impl_tuple_mapper {
    ([$($(($big:ident,$index:tt)),+)?]) => {

        maybe_tuple_doc! {
            $($($big)+)? @

            /// This trait is implemented for tuples up to 21 items long
            impl<M $($(,$big)*)? > TupleMap<M> for ( $($($big,)*)? )
            $(
                where
                    $(M: Mapper<$big>,)*
            )?
            {
                type Target = ( $($( <M as Mapper<$big>>::Target,)*)? );
                #[inline]
                fn map(self) -> Self::Target {
                    $(( $(<M as Mapper<$big>>::map(self.$index), )*))?
                }
            }
        }
    };
}

crate::generate_variadic_array! {[21 T,#] impl_tuple_mapper!{}}
