use super::{Func, FuncMut, FuncOnce};
use crate::tuple::Tuple;

#[diagnostic::on_unimplemented(
    message = "expected a `AsyncFuncOnce<{Args}>` async closure, found `{Self}`",
    label = "expected an `AsyncFuncOnce<{Args}>` async closure, found `{Self}`"
)]
#[must_use = "async closures are lazy and do nothing unless called"]
pub trait AsyncFuncOnce<Args: Tuple> {
    type Future: Future<Output = Self::Output>;
    type Output;

    fn async_call_once(self, args: Args) -> Self::Future;
}

#[diagnostic::on_unimplemented(
    message = "expected a `AsyncFuncMut<{Args}>` async closure, found `{Self}`",
    label = "expected an `AsyncFuncMut<{Args}>` async closure, found `{Self}`"
)]
#[must_use = "async closures are lazy and do nothing unless called"]
pub trait AsyncFuncMut<Args: Tuple>: AsyncFuncOnce<Args> {
    fn async_call_mut(&mut self, args: Args) -> Self::Future;
}

#[diagnostic::on_unimplemented(
    message = "expected a `AsyncFunc<{Args}>` async closure, found `{Self}`",
    label = "expected an `AsyncFunc<{Args}>` async closure, found `{Self}`"
)]
#[must_use = "async closures are lazy and do nothing unless called"]
pub trait AsyncFunc<Args: Tuple>: AsyncFuncMut<Args> {
    fn async_call(&self, args: Args) -> Self::Future;
}

impl<F, Args> AsyncFuncOnce<Args> for F
where
    F: FuncOnce<Args>,
    F::Output: Future,
    Args: Tuple,
{
    type Future = F::Output;
    type Output = <Self::Future as Future>::Output;

    #[inline]
    fn async_call_once(self, args: Args) -> Self::Future {
        FuncOnce::call_once(self, args)
    }
}

impl<F, Args> AsyncFuncMut<Args> for F
where
    F: FuncMut<Args>,
    F::Output: Future,
    Args: Tuple,
{
    #[inline]
    fn async_call_mut(&mut self, args: Args) -> Self::Future {
        FuncMut::call_mut(self, args)
    }
}

impl<F, Args> AsyncFunc<Args> for F
where
    F: Func<Args>,
    F::Output: Future,
    Args: Tuple,
{
    #[inline]
    fn async_call(&self, args: Args) -> Self::Future {
        Func::call(self, args)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::func::tests::*;

    pub fn assert_asyncfn_type<Args: Tuple, O>(
        f: impl AsyncFunc<Args, Output = O>,
    ) -> impl AsyncFunc<Args, Output = O> {
        f
    }

    pub fn assert_asyncfnmut_type<Args: Tuple, O>(
        f: impl AsyncFuncMut<Args, Output = O>,
    ) -> impl AsyncFuncMut<Args, Output = O> {
        f
    }

    pub fn assert_asyncfnonce_type<Args: Tuple, O>(
        f: impl AsyncFuncOnce<Args, Output = O>,
    ) -> impl AsyncFuncOnce<Args, Output = O> {
        f
    }

    #[pollster::test]
    async fn test_fn() {
        async fn fixture1(a: u32, b: &str) -> bool {
            a > 4 && !b.is_empty()
        }
        let c = Cell::new(5);
        let f1 = assert_asyncfn_type(fixture1);
        let f2 = assert_asyncfn_type(async |a: u32, b: &str| a > 4 && !b.is_empty() && c.get() > 4);

        assert!(f1.async_call((5, "a")).await);
        assert!(!f1.async_call((3, "")).await);
        assert!(f2.async_call((5, "a")).await);
        assert!(!f2.async_call((3, "")).await);
        c.set(3);
        assert!(!f2.async_call((5, "a")).await);
    }

    /*
    TODO: async mut doesn't work yet, as its needs to capture the mutable reference
    #[pollster::test]
    async fn test_fnmut() {
        let mut c = 0;
        let mut f = assert_asyncfnmut_type(async |a: u32, b: &str| {
            c += 1;
            a > 4 && !b.is_empty()
        });

        assert!(f.async_call_mut((5, "a")).await);
        assert!(!f.async_call_mut((3, "")).await);
        drop(f);
        assert_eq!(2, c);
    }
    */

    #[pollster::test]
    async fn test_fnonce() {
        fn assertion(a: u32, b: &str, c: NonCopy) -> bool {
            c.0 == 123 && a > 4 && !b.is_empty()
        }
        let c = NonCopy(123);
        let f = assert_asyncfnonce_type(async move |a: u32, b: &str| assertion(a, b, c));

        assert!(f.async_call_once((5, "a")).await);
    }
}
