use crate::resource::Resources;

/// # Safety
/// The value ov IS_SEND must be correct: when it says true, then the type must be Send!
pub unsafe trait SystemParam: Sized {
    const IS_SEND: bool;

    type Prepared: Send + Sync;
    type Fetch: for<'a> SystemParamFetch<'a, Prepared = Self::Prepared>;
    fn prepare(resources: &mut Resources) -> Self::Prepared;
}

pub trait SystemParamFetch<'a>: SystemParam<Fetch = Self> {
    type Output;
    fn get(prepared: &'a mut Self::Prepared, resources: &'a Resources) -> Self::Output
    where
        Self: 'a;
}

unsafe impl SystemParam for () {
    const IS_SEND: bool = true;

    type Prepared = ();
    type Fetch = ();
    #[inline]
    fn prepare(_resources: &mut Resources) {}
}

impl SystemParamFetch<'_> for () {
    type Output = ();
    fn get(_prepared: &mut Self::Prepared, _resources: &Resources) -> Self::Output {}
}

macro_rules! tuple {
  () => ();
  ( $($name:ident.$index:tt,)+ ) => (

      unsafe impl<$($name),+> SystemParam for ($($name,)+)
      where
          $($name : SystemParam),+
      {
          const IS_SEND: bool = $($name::IS_SEND)&&+;

          type Prepared = ($($name::Prepared,)+) ;
          type Fetch = ($($name::Fetch,)+) ;
          #[inline]
          fn prepare(resources: &mut Resources) -> Self::Prepared {
              ($($name::prepare(resources),)+)
          }
      }

      impl<'a $(,$name)+> SystemParamFetch<'a> for ($($name,)+)
      where
          $($name : SystemParamFetch<'a>,)+
      {
          type Output =  ($($name::Output,)+);
          #[inline]
          fn get(prepared: &'a mut Self::Prepared, resources: &'a Resources) -> Self::Output where Self: 'a {
              ($($name::get(&mut prepared.$index, resources),)+)
          }
      }

      peel! { tuple [] $($name.$index,)+ }
  )
}

tuple! { T0.0, T1.1, T2.2, T3.3, T4.4, T5.5, T6.6, T7.7, T8.8, T9.9, T10.10, T11.11, }
