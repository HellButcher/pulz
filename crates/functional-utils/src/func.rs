use crate::tuple::Tuple;

#[cfg(feature = "func-bind")]
pub mod bind;
pub mod future;

#[diagnostic::on_unimplemented(
    message = "expected a `FuncOnce<{Args}>` closure, found `{Self}`",
    label = "expected an `FuncOnce<{Args}>` closure, found `{Self}`"
)]
#[must_use = "closures are lazy and do nothing unless called"]
pub trait FuncOnce<Args: Tuple> {
    type Output;

    fn call_once(self, args: Args) -> Self::Output;

    #[cfg(feature = "func-bind")]
    #[inline]
    fn bind<B>(self, arg: B) -> bind::BindFn<B, Self>
    where
        Self: Sized,
    {
        bind::BindFn(arg, self)
    }
}
#[diagnostic::on_unimplemented(
    message = "expected a `FuncMut<{Args}>` closure, found `{Self}`",
    label = "expected an `FuncMut<{Args}>` closure, found `{Self}`"
)]
#[must_use = "closures are lazy and do nothing unless called"]
pub trait FuncMut<Args: Tuple>: FuncOnce<Args> {
    fn call_mut(&mut self, args: Args) -> Self::Output;

    #[cfg(feature = "func-bind")]
    #[inline]
    fn bind_mut<B>(self, arg: B) -> bind::BindFnMut<B, Self>
    where
        Self: Sized,
    {
        bind::BindFnMut(arg, self)
    }
}
#[diagnostic::on_unimplemented(
    message = "expected a `Func<{Args}>` closure, found `{Self}`",
    label = "expected an `Func<{Args}>` closure, found `{Self}`"
)]
#[must_use = "closures are lazy and do nothing unless called"]
pub trait Func<Args: Tuple>: FuncMut<Args> {
    fn call(&self, args: Args) -> Self::Output;

    #[cfg(feature = "func-bind")]
    #[inline]
    fn bind_ref<B>(self, arg: B) -> bind::BindFnRef<B, Self>
    where
        Self: Sized,
    {
        bind::BindFnRef(arg, self)
    }
}

macro_rules! impl_func {
    ([$(($big:ident,$index:tt)),*]) => {
        impl<F, O $(, $big)*> FuncOnce<($($big,)*)> for F
        where
            F: FnOnce($($big),*) -> O,
        {
            type Output = O;
            #[inline]
            fn call_once(self, _args: ($($big,)*)) -> O {
                self($(_args.$index),*)
            }
        }

        impl<F, O $(, $big)*> FuncMut<($($big,)*)> for F
        where
            F: FnMut($($big),*) -> O,
        {
            #[inline]
            fn call_mut(&mut self, _args: ($($big,)*)) -> O {
                self($(_args.$index),*)
            }
        }

        impl<F, O $(, $big)*> Func<($($big,)*)> for F
        where
            F: Fn($($big),*) -> O,
        {
            #[inline]
            fn call(&self, _args: ($($big,)*)) -> O {
                self($(_args.$index),*)
            }
        }
    };
}

crate::generate_variadic_array! {[T,#] impl_func!{}}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    pub fn assert_fn_type<Args: Tuple, O>(
        f: impl Func<Args, Output = O>,
    ) -> impl Func<Args, Output = O> {
        f
    }

    pub fn assert_fnmut_type<Args: Tuple, O>(
        f: impl FuncMut<Args, Output = O>,
    ) -> impl FuncMut<Args, Output = O> {
        f
    }

    pub fn assert_fnonce_type<Args: Tuple, O>(
        f: impl FuncOnce<Args, Output = O>,
    ) -> impl FuncOnce<Args, Output = O> {
        f
    }

    pub struct NonCopy(pub u32);

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
    fn test_fnonce() {
        fn assertion(a: u32, b: &str, c: NonCopy) -> bool {
            c.0 == 123 && a > 4 && !b.is_empty()
        }
        let c = NonCopy(123);
        let f = assert_fnonce_type(move |a: u32, b: &str| assertion(a, b, c));

        assert!(f.call_once((5, "a")));
    }
}
