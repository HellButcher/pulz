use super::Tuple;
pub trait TuplePushFront<T>: Tuple {
    type Pushed: TuplePopFront<Front = T, Rest = Self>;
    fn push_front(self, value: T) -> Self::Pushed;
}

pub trait TuplePushBack<T>: Tuple {
    type Pushed: TuplePopBack<Back = T, Rest = Self>;
    fn push_back(self, value: T) -> Self::Pushed;
}

pub trait TuplePopFront: Tuple {
    type Front;
    type Rest: TuplePushFront<Self::Front, Pushed = Self>;
    fn pop_front(self) -> (Self::Front, Self::Rest);
}

pub trait TuplePopBack: Tuple {
    type Back;
    type Rest: TuplePushBack<Self::Back, Pushed = Self>;
    fn pop_back(self) -> (Self::Rest, Self::Back);
}

macro_rules! impl_push_pop {
    ([$(($big:ident,$small:ident,$index:tt)),*]) => {

        maybe_tuple_doc! {
            $($big)* @

            /// This trait is implemented for tuples up to 20 items long
            impl<$($big,)* P> TuplePushFront<P> for ( $($big,)* ) {
                type Pushed = (P, $($big,)* );
                #[inline(always)]
                fn push_front(self, value: P) -> Self::Pushed {
                    (value, $(self.$index, )*)
                }
            }
        }


        maybe_tuple_doc! {
            $($big)* @

            /// This trait is implemented for tuples up to 20 items long
            impl<$($big,)* P> TuplePushBack<P> for ( $($big,)* ) {
                type Pushed = ($($big,)* P,);
                #[inline(always)]
                fn push_back(self, value: P) -> Self::Pushed {
                    ($(self.$index, )* value, )
                }
            }
        }

        maybe_tuple_doc! {
            P $($big)* @

            /// This trait is implemented for tuples up to 21 items long
            impl<P $(,$big)* > TuplePopFront for (P, $($big,)* ) {
                type Front = P;
                type Rest = ($($big,)* );
                #[inline(always)]
                fn pop_front(self) -> (P, Self::Rest) {
                    let (f, $($small, )*) = self;
                    (f, ($($small,)*))
                }
            }
        }

        maybe_tuple_doc! {
            $($big)* P @

            /// This trait is implemented for tuples up to 21 items long
            impl<$($big,)* P> TuplePopBack for ($($big,)* P, ) {
                type Back = P;
                type Rest = ($($big,)* );
                #[inline(always)]
                fn pop_back(self) -> (Self::Rest, P) {
                    let ($($small, )* l,) = self;
                    (($($small,)*), l)
                }
            }
        }
    };
}

crate::generate_variadic_array! {[20 T,t,#] impl_push_pop!{}}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push() {
        let b = ().push_front(1u32);
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
        let ((), b) = b.pop_back();
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
}
