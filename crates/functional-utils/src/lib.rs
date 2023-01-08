#![warn(
    //missing_docs,
    rustdoc::missing_doc_code_examples,
    future_incompatible,
    rust_2018_idioms,
    unused,
    trivial_casts,
    trivial_numeric_casts,
    unused_lifetimes,
    unused_qualifications,
    unused_crate_dependencies,
    clippy::cargo,
    clippy::multiple_crate_versions,
    clippy::empty_line_after_outer_attr,
    clippy::fallible_impl_from,
    clippy::redundant_pub_crate,
    clippy::use_self,
    clippy::suspicious_operation_groupings,
    clippy::useless_let_if_seq,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::wildcard_imports
)]
#![doc(html_logo_url = "https://raw.githubusercontent.com/HellButcher/pulz/master/docs/logo.png")]
#![doc(html_no_source)]
#![doc = include_str!("../README.md")]

pub use pulz_functional_utils_macros::generate_variadic_array;

pub trait TuplePushFront<T> {
    type Pushed: TuplePopFront<Front = T, Rest = Self>;
    fn push_front(self, value: T) -> Self::Pushed;
}

pub trait TuplePushBack<T> {
    type Pushed: TuplePopBack<Back = T, Rest = Self>;
    fn push_back(self, value: T) -> Self::Pushed;
}

pub trait TuplePopFront {
    type Front;
    type Rest: TuplePushFront<Self::Front, Pushed = Self>;
    fn pop_front(self) -> (Self::Front, Self::Rest);
}

pub trait TuplePopBack {
    type Back;
    type Rest: TuplePushBack<Self::Back, Pushed = Self>;
    fn pop_back(self) -> (Self::Rest, Self::Back);
}

macro_rules! impl_push_pop {
    ([$(($big:ident,$small:ident,$index:tt)),*]) => {

        impl<T $(,$big)* > TuplePushFront<T> for ( $($big,)* ) {
            type Pushed = (T, $($big,)* );
            #[inline(always)]
            fn push_front(self, value: T) -> Self::Pushed {
                (value, $(self.$index, )*)
            }
        }

        impl<T $(,$big)* > TuplePushBack<T> for ( $($big,)* ) {
            type Pushed = ($($big,)* T,);
            #[inline(always)]
            fn push_back(self, value: T) -> Self::Pushed {
                ($(self.$index, )* value, )
            }
        }

        impl<T $(,$big)* > TuplePopFront for (T, $($big,)* ) {
            type Front = T;
            type Rest = ($($big,)* );
            #[inline(always)]
            fn pop_front(self) -> (T, Self::Rest) {
                let (f, $($small, )*) = self;
                (f, ($($small,)*))
            }
        }

        impl<$($big,)* T> TuplePopBack for ($($big,)* T, ) {
            type Back = T;
            type Rest = ($($big,)* );
            #[inline(always)]
            fn pop_back(self) -> (Self::Rest, T) {
                let ($($small, )* l,) = self;
                (($($small,)*), l)
            }
        }
    };
}

generate_variadic_array! {[T,t,#] impl_push_pop!{}}

pub trait TupleJoin<E> {
    type Joined;
    fn join(self, extension: E) -> Self::Joined;
}

impl<T> TupleJoin<()> for T {
    type Joined = Self;
    #[inline(always)]
    fn join(self, _e: ()) -> Self {
        self
    }
}

impl<E, T> TupleJoin<E> for T
where
    E: TuplePopFront,
    T: TuplePushBack<E::Front>,
    T::Pushed: TupleJoin<E::Rest>,
{
    type Joined = <T::Pushed as TupleJoin<E::Rest>>::Joined;

    #[inline(always)]
    fn join(self, e: E) -> Self::Joined {
        let (e1, e_rest) = e.pop_front();
        let s = self.push_back(e1);
        s.join(e_rest)
    }
}

pub trait TupleSplitFront<A> {
    type Rest;
    fn split_front(self) -> (A, Self::Rest);
}

impl<T> TupleSplitFront<()> for T {
    type Rest = Self;
    #[inline(always)]
    fn split_front(self) -> ((), Self) {
        ((), self)
    }
}

impl<E, T> TupleSplitFront<E> for T
where
    E: TuplePopFront,
    T: TuplePopFront<Front = E::Front>,
    T::Rest: TupleSplitFront<E::Rest>,
{
    type Rest = <T::Rest as TupleSplitFront<E::Rest>>::Rest;

    #[inline(always)]
    fn split_front(self) -> (E, Self::Rest) {
        let (s1, rest) = self.pop_front();
        let (s2, rest) = rest.split_front();
        let first = s2.push_front(s1);
        (first, rest)
    }
}

pub trait Mapper<T> {
    type Target;
    fn map(value: T) -> Self::Target;
}

pub enum DerefMapper {}
impl<'a, T> Mapper<&'a T> for DerefMapper
where
    T: std::ops::Deref,
{
    type Target = &'a T::Target;
    #[inline]
    fn map(value: &'a T) -> &'a T::Target {
        value
    }
}
impl<'a, T> Mapper<&'a mut T> for DerefMapper
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
impl<'a, T> Mapper<&'a mut T> for DerefMutMapper
where
    T: std::ops::DerefMut,
{
    type Target = &'a mut T::Target;
    #[inline]
    fn map(value: &'a mut T) -> &'a mut T::Target {
        value
    }
}

pub trait TupleMap<M> {
    type Target;
    fn map(self) -> Self::Target;
}

macro_rules! impl_tuple_mapper {
    ([$($(($big:ident,$index:tt)),+)?]) => {

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
    };
}

generate_variadic_array! {[T,#] impl_tuple_mapper!{}}

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

pub trait TupleConvert<C, Into> {
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

generate_variadic_array! {[S,T,#] impl_tuple_convert!{}}

pub trait CallFnOnce<Args> {
    type Output;

    fn call_once(self, args: Args) -> Self::Output;

    #[inline]
    fn bind<B>(self, arg: B) -> BindFn<B, Self>
    where
        Self: Sized,
    {
        BindFn(arg, self)
    }
}
pub trait CallFnMut<Args>: CallFnOnce<Args> {
    fn call_mut(&mut self, args: Args) -> Self::Output;

    #[inline]
    fn bind_mut<B>(self, arg: B) -> BindFnMut<B, Self>
    where
        Self: Sized,
    {
        BindFnMut(arg, self)
    }
}
pub trait CallFn<Args>: CallFnMut<Args> {
    fn call(&self, args: Args) -> Self::Output;

    #[inline]
    fn bind_ref<B>(self, arg: B) -> BindFnRef<B, Self>
    where
        Self: Sized,
    {
        BindFnRef(arg, self)
    }
}

macro_rules! impl_call_fn {
    ([$(($big:ident,$index:tt)),*]) => {
        impl<F, O $(, $big)*> CallFnOnce<($($big,)*)> for F
        where
            F: FnOnce($($big),*) -> O,
        {
            type Output = O;
            #[inline]
            fn call_once(self, _args: ($($big,)*)) -> O {
                self($(_args.$index),*)
            }
        }

        impl<F, O $(, $big)*> CallFnMut<($($big,)*)> for F
        where
            F: FnMut($($big),*) -> O,
        {
            #[inline]
            fn call_mut<'o>(&'o mut self, _args: ($($big,)*)) -> O {
                self($(_args.$index),*)
            }
        }

        impl<F, O $(, $big)*> CallFn<($($big,)*)> for F
        where
            F: Fn($($big),*) -> O,
            //for<'o> F: CallFnOutput<'o, ($($big,)*), Output=O>,
        {
            #[inline]
            fn call<'o>(&'o self, _args: ($($big,)*)) -> O {
                self($(_args.$index),*)
            }
        }
    };
}

generate_variadic_array! {[T,#] impl_call_fn!{}}

pub struct BindFn<Bound, F: ?Sized>(Bound, F);
pub struct BindFnMut<Bound, F: ?Sized>(Bound, F);
pub struct BindFnRef<Bound, F: ?Sized>(Bound, F);

macro_rules! impl_bind_fn {
    ([$(($big:ident,$index:tt)),*]) => {

        impl<'a, B: 'a, F, O $(,$big)*> CallFnOnce<($($big,)*)> for BindFn<B, F>
        where
            F: CallFnOnce<(B, $($big,)*), Output=O>,
        {
            type Output = O;
            #[inline]
            fn call_once(self, _args: ($($big,)*)) -> O {
                self.1.call_once((self.0, $(_args.$index,)*))
            }
        }

        impl<'a, B: ?Sized + 'a, F, O $(,$big)*> CallFnMut<($($big,)*)> for BindFn<&'a mut B, F>
        where
            F: for<'b> CallFnMut<(&'b mut B, $($big,)*), Output=O>,
        {
            #[inline]
            fn call_mut(&mut self, _args: ($($big,)*)) -> O {
                self.1.call_mut((self.0, $(_args.$index,)*))
            }
        }

        impl<'a, B: ?Sized + 'a, F, O $(,$big)*> CallFnMut<($($big,)*)> for BindFn<&'a B, F>
        where
            F: CallFnMut<(&'a B, $($big,)*), Output=O>,
        {
            #[inline]
            fn call_mut(&mut self, _args: ($($big,)*)) -> O {
                self.1.call_mut((self.0, $(_args.$index,)*))
            }
        }

        impl<'a, B: ?Sized + 'a, F, O $(,$big)*> CallFn<($($big,)*)> for BindFn<&'a B, F>
        where
            F: CallFn<(&'a B, $($big,)*), Output=O>,
        {
            #[inline]
            fn call(&self, _args: ($($big,)*)) -> O {
                self.1.call((self.0, $(_args.$index,)*))
            }
        }

        impl<B, F, O $(,$big)*> CallFnOnce<($($big,)*)> for BindFnMut<B, F>
        where
            for<'b> F: CallFnOnce<(&'b mut B, $($big,)*), Output=O>,
        {
            type Output = O;
            #[inline]
            fn call_once(mut self, _args: ($($big,)*)) -> O {
                self.1.call_once((&mut self.0, $(_args.$index,)*))
            }
        }

        impl<B, F, O $(,$big)*> CallFnMut<($($big,)*)> for BindFnMut<B, F>
        where
            for<'b> F: CallFnMut<(&'b mut B, $($big,)*), Output=O>,
        {
            #[inline]
            fn call_mut(&mut self, _args: ($($big,)*)) -> O {
                self.1.call_mut((&mut self.0, $(_args.$index,)*))
            }
        }

        impl<B, F, O $(,$big)*> CallFnOnce<($($big,)*)> for BindFnRef<B, F>
        where
            for<'b> F: CallFnOnce<(&'b B, $($big,)*), Output=O>,
        {
            type Output = O;
            #[inline]
            fn call_once(self, _args: ($($big,)*)) -> O {
                self.1.call_once((&self.0, $(_args.$index,)*))
            }
        }

        impl<B, F, O $(,$big)*> CallFnMut<($($big,)*)> for BindFnRef<B, F>
        where
            for<'b> F: CallFnMut<(&'b B, $($big,)*), Output=O>,
        {
            #[inline]
            fn call_mut(&mut self, _args: ($($big,)*)) -> O {
                self.1.call_mut((&self.0, $(_args.$index,)*))
            }
        }

        impl<B, F, O $(,$big)*> CallFn<($($big,)*)> for BindFnRef<B, F>
        where
            for<'b> F: CallFn<(&'b B, $($big,)*), Output=O>,
        {
            #[inline]
            fn call(&self, _args: ($($big,)*)) -> O {
                self.1.call((&self.0, $(_args.$index,)*))
            }
        }
    };
}

generate_variadic_array! {[T,#] impl_bind_fn!{}}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    fn assert_fn_type<Args, O>(f: impl CallFn<Args, Output = O>) -> impl CallFn<Args, Output = O> {
        f
    }

    fn assert_fnmut_type<Args, O>(
        f: impl CallFnMut<Args, Output = O>,
    ) -> impl CallFnMut<Args, Output = O> {
        f
    }

    fn assert_fnonce_type<Args, O>(
        f: impl CallFnOnce<Args, Output = O>,
    ) -> impl CallFnOnce<Args, Output = O> {
        f
    }

    #[test]
    fn test_push() {
        let a = ();
        let b = a.push_front(1u32);
        assert_eq!((1,), b);
        let b = b.push_front(2u32);
        assert_eq!((2, 1), b);
        let b = b.push_back(3u32);
        assert_eq!((2, 1, 3), b);
    }

    #[test]
    fn test_pop() {
        let a = (1, 2, 3);
        let (a, b) = a.pop_front();
        assert_eq!(1, a);
        assert_eq!((2, 3), b);
        let (b, c) = b.pop_back();
        assert_eq!((2,), b);
        assert_eq!(3, c);
        let (x, b) = b.pop_back();
        assert_eq!((), x);
        assert_eq!(2, b);
    }

    #[test]
    fn test_join() {
        let a = (1, 2, 3);
        let b = (4, 5);
        let c = a.join(b);
        assert_eq!((1, 2, 3, 4, 5), c);
    }

    #[test]
    fn test_split_front() {
        let c = (1, 2, 3, 4, 5);
        let (a, b) = c.split_front();
        assert_eq!((1, 2), a);
        assert_eq!((3, 4, 5), b);
    }

    #[test]
    fn test_fn() {
        fn fixture1(a: u32, b: &str) -> bool {
            a > 4 && !b.is_empty()
        }
        let c = Cell::new(5);
        let f1 = assert_fn_type(fixture1);
        let f2 = assert_fn_type(|a: u32, b: &str| a > 4 && !b.is_empty() && c.get() > 4);

        assert!(f1.call((5, "a")));
        assert!(!f1.call((3, "")));
        assert!(f2.call((5, "a")));
        assert!(!f2.call((3, "")));
        c.set(3);
        assert!(!f2.call((5, "a")));
    }

    #[test]
    fn test_fnmut() {
        let mut c = 0;
        let mut f = assert_fnmut_type(|a: u32, b: &str| {
            c += 1;
            a > 4 && !b.is_empty()
        });

        assert!(f.call_mut((5, "a")));
        assert!(!f.call_mut((3, "")));
        drop(f);
        assert_eq!(2, c);
    }

    #[test]
    fn test_bind_once() {
        fn fixture1(a: u32) -> bool {
            a > 4
        }
        fn fixture2(a: u32, b: &str) -> bool {
            a > 4 && !b.is_empty()
        }
        let f5 = assert_fnonce_type(fixture1.bind(5));
        assert!(f5.call_once(()));
        let f3 = assert_fnonce_type(fixture1.bind(3));
        assert!(!f3.call_once(()));

        let f5 = assert_fnonce_type(fixture2.bind(5));
        assert!(f5.call_once(("foo",)));
        let f3 = assert_fnonce_type(fixture2.bind(3));
        assert!(!f3.call_once(("foo",)));
    }

    #[test]
    fn test_bind_mut() {
        fn fixture1(a: &mut u32) -> bool {
            *a -= 1;
            *a > 3
        }
        fn fixture2(a: &mut u32, b: &mut bool) -> bool {
            *a -= 1;
            let r = *a > 3 && *b;
            *b = !*b;
            r
        }
        let mut f = assert_fnmut_type(fixture1.bind_mut(5));
        assert!(f.call_mut(()));
        assert!(!f.call_mut(()));

        let mut foo = true;
        let mut foo2 = true;
        let mut f = assert_fnmut_type(fixture2.bind_mut(5));

        assert!(f.call_mut((&mut foo,)));
        assert!(!f.call_mut((&mut foo2,)));
        drop(f);
        assert!(!foo);
        assert!(!foo2);

        foo = true;
        let mut f = assert_fnmut_type(fixture2.bind_mut(5).bind(&mut foo));
        assert!(f.call_mut(()));
        drop(f);
        assert!(!foo);
    }

    #[test]
    fn test_bind_ref() {
        fn fixture1(a: &Cell<u32>) -> bool {
            let v = a.get();
            a.set(v - 1);
            v > 4
        }
        fn fixture2(a: &Cell<u32>, b: &bool) -> bool {
            let v = a.get();
            a.set(v - 1);
            v > 4 && *b
        }

        fn fixture3(a: &Cell<u32>, b: &str) -> bool {
            let v = a.get();
            a.set(v - 1);
            v > 4 && !b.is_empty()
        }
        let f = assert_fn_type(fixture1.bind_ref(Cell::new(5)));
        assert!(f.call(()));
        assert!(!f.call(()));

        let f = assert_fn_type(fixture2.bind_ref(Cell::new(5)));
        assert!(f.call((&true,)));
        assert!(!f.call((&true,)));
        let foo = false;
        let f = assert_fn_type(fixture2.bind_ref(Cell::new(5)).bind(&foo));
        assert!(!f.call(()));

        let f = assert_fn_type(fixture3.bind_ref(Cell::new(5)).bind("foo"));
        assert!(f.call(()));
    }
}
