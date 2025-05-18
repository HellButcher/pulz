use super::Tuple;

pub trait Converter<From: ?Sized, Into: ?Sized> {
    fn convert(from: From) -> Into;
}
pub enum FromConverter {}
impl<A, B> Converter<A, B> for FromConverter
where
    B: From<A>,
{
    #[inline]
    fn convert(from: A) -> B {
        B::from(from)
    }
}

pub enum AsRefConverter {}
impl<'l, A, B> Converter<&'l A, &'l B> for AsRefConverter
where
    A: AsRef<B>,
{
    #[inline]
    fn convert(from: &A) -> &B {
        A::as_ref(from)
    }
}
impl<'l, A, B> Converter<&'l mut A, &'l B> for AsRefConverter
where
    A: AsRef<B>,
{
    #[inline]
    fn convert(from: &mut A) -> &B {
        A::as_ref(from)
    }
}
impl<'l, A, B> Converter<&'l mut A, &'l mut B> for AsRefConverter
where
    A: AsMut<B>,
{
    #[inline]
    fn convert(from: &mut A) -> &mut B {
        A::as_mut(from)
    }
}

pub enum BorrowConverter {}
impl<'l, A, B> Converter<&'l A, &'l B> for BorrowConverter
where
    A: std::borrow::Borrow<B>,
{
    #[inline]
    fn convert(from: &A) -> &B {
        A::borrow(from)
    }
}
impl<'l, A, B> Converter<&'l mut A, &'l B> for BorrowConverter
where
    A: std::borrow::Borrow<B>,
{
    #[inline]
    fn convert(from: &mut A) -> &B {
        A::borrow(from)
    }
}
impl<'l, A, B> Converter<&'l mut A, &'l mut B> for BorrowConverter
where
    A: std::borrow::BorrowMut<B>,
{
    #[inline]
    fn convert(from: &mut A) -> &mut B {
        A::borrow_mut(from)
    }
}

pub trait TupleConvert<C, Into: Tuple> {
    fn convert(self) -> Into;
}

macro_rules! impl_tuple_convert {
    ([$($(($big1:ident,$big2:ident,$index:tt)),+)?]) => {

        impl<C $($(,$big1,$big2)*)? > TupleConvert<C, ( $($($big2,)*)? )> for ( $($($big1,)*)? )
        $(
            where
                $(C: Converter<$big1,$big2>,)*
        )?
        {
            #[inline]
            fn convert(self) -> ( $($($big2,)*)? ) {
                $(( $(<C as Converter<$big1,$big2>>::convert(self.$index), )*))?
            }
        }

        impl<'l, C $($(,$big1,$big2)*)? > TupleConvert<C, ( $($($big2,)*)? )> for &'l ( $($($big1,)*)? )
        $(
            where
                $(C: Converter<&'l $big1,$big2>,)*
        )?
        {
            #[inline]
            fn convert(self) -> ( $($($big2,)*)? ) {
                $(( $(<C as Converter<&'l $big1,$big2>>::convert(&self.$index), )*))?
            }
        }

        impl<'l, C $($(,$big1,$big2)*)? > TupleConvert<C, ( $($($big2,)*)? )> for &'l mut ( $($($big1,)*)? )
        $(
            where
                $(C: Converter<&'l mut $big1,$big2>,)*
        )?
        {
            #[inline]
            fn convert(self) -> ( $($($big2,)*)? ) {
                $(( $(<C as Converter<&'l mut $big1,$big2>>::convert(&mut self.$index), )*))?
            }
        }
    };
}

crate::generate_variadic_array! {[21 S,T,#] impl_tuple_convert!{}}
