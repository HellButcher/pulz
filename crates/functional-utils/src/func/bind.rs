use super::{Func, FuncMut, FuncOnce};

pub struct BindFn<Bound, F: ?Sized>(pub(super) Bound, pub(super) F);
pub struct BindFnMut<Bound, F: ?Sized>(pub(super) Bound, pub(super) F);
pub struct BindFnRef<Bound, F: ?Sized>(pub(super) Bound, pub(super) F);

macro_rules! impl_bind_fn {
    ([$(($big:ident,$index:tt)),*]) => {

        maybe_tuple_doc! {
            $($big)* @

            /// This trait is implemented for argument lists up to 20 items long
            impl<'a, B: 'a, F, O $(,$big)*> FuncOnce<($($big,)*)> for BindFn<B, F>
            where
                F: FuncOnce<(B, $($big,)*), Output=O>,
            {
                type Output = O;
                #[inline]
                fn call_once(self, _args: ($($big,)*)) -> O {
                    self.1.call_once((self.0, $(_args.$index,)*))
                }
            }
        }

        maybe_tuple_doc! {
            $($big)* @

            /// This trait is implemented for argument lists up to 20 items long
            impl<'a, B: ?Sized + 'a, F, O $(,$big)*> FuncMut<($($big,)*)> for BindFn<&'a mut B, F>
            where
                F: for<'b> FuncMut<(&'b mut B, $($big,)*), Output=O>,
            {
                #[inline]
                fn call_mut(&mut self, _args: ($($big,)*)) -> O {
                    self.1.call_mut((self.0, $(_args.$index,)*))
                }
            }
        }

        maybe_tuple_doc! {
            $($big)* @

            /// This trait is implemented for argument lists up to 20 items long
            impl<'a, B: ?Sized + 'a, F, O $(,$big)*> FuncMut<($($big,)*)> for BindFn<&'a B, F>
            where
                F: FuncMut<(&'a B, $($big,)*), Output=O>,
            {
                #[inline]
                fn call_mut(&mut self, _args: ($($big,)*)) -> O {
                    self.1.call_mut((self.0, $(_args.$index,)*))
                }
            }
        }

        maybe_tuple_doc! {
            $($big)* @

            /// This trait is implemented for argument lists up to 20 items long
            impl<'a, B: ?Sized + 'a, F, O $(,$big)*> Func<($($big,)*)> for BindFn<&'a B, F>
            where
                F: Func<(&'a B, $($big,)*), Output=O>,
            {
                #[inline]
                fn call(&self, _args: ($($big,)*)) -> O {
                    self.1.call((self.0, $(_args.$index,)*))
                }
            }
        }

        maybe_tuple_doc! {
            $($big)* @

            /// This trait is implemented for argument lists up to 20 items long
            impl<B, F, O $(,$big)*> FuncOnce<($($big,)*)> for BindFnMut<B, F>
            where
                for<'b> F: FuncOnce<(&'b mut B, $($big,)*), Output=O>,
            {
                type Output = O;
                #[inline]
                fn call_once(mut self, _args: ($($big,)*)) -> O {
                    self.1.call_once((&mut self.0, $(_args.$index,)*))
                }
            }
        }

        maybe_tuple_doc! {
            $($big)* @

            /// This trait is implemented for argument lists up to 20 items long
            impl<B, F, O $(,$big)*> FuncMut<($($big,)*)> for BindFnMut<B, F>
            where
                for<'b> F: FuncMut<(&'b mut B, $($big,)*), Output=O>,
            {
                #[inline]
                fn call_mut(&mut self, _args: ($($big,)*)) -> O {
                    self.1.call_mut((&mut self.0, $(_args.$index,)*))
                }
            }
        }

        maybe_tuple_doc! {
            $($big)* @

            /// This trait is implemented for argument lists up to 20 items long
            impl<B, F, O $(,$big)*> FuncOnce<($($big,)*)> for BindFnRef<B, F>
            where
                for<'b> F: FuncOnce<(&'b B, $($big,)*), Output=O>,
            {
                type Output = O;
                #[inline]
                fn call_once(self, _args: ($($big,)*)) -> O {
                    self.1.call_once((&self.0, $(_args.$index,)*))
                }
            }
        }

        maybe_tuple_doc! {
            $($big)* @

            /// This trait is implemented for argument lists up to 20 items long
            impl<B, F, O $(,$big)*> FuncMut<($($big,)*)> for BindFnRef<B, F>
            where
                for<'b> F: FuncMut<(&'b B, $($big,)*), Output=O>,
            {
                #[inline]
                fn call_mut(&mut self, _args: ($($big,)*)) -> O {
                    self.1.call_mut((&self.0, $(_args.$index,)*))
                }
            }
        }

        maybe_tuple_doc! {
            $($big)* @

            /// This trait is implemented for argument lists up to 20 items long
            impl<B, F, O $(,$big)*> Func<($($big,)*)> for BindFnRef<B, F>
            where
                for<'b> F: Func<(&'b B, $($big,)*), Output=O>,
            {
                #[inline]
                fn call(&self, _args: ($($big,)*)) -> O {
                    self.1.call((&self.0, $(_args.$index,)*))
                }
            }
        }

    };
}

crate::generate_variadic_array! {[T,#] impl_bind_fn!{}}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::func::tests::*;

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
