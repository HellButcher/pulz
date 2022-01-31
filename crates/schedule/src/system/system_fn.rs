use std::marker::PhantomData;

use crate::{
    resource::Resources,
    system::{
        param::{SystemParam, SystemParamFetch},
        ExclusiveSystem, IntoSystem, System, SystemDescriptor, SystemVariant,
    },
};

pub struct SystemFn<Param, F>
where
    Param: SystemParam,
{
    func: F,
    prepared: Option<Param::Prepared>,
    is_send: bool,
    marker: PhantomData<fn() -> (Param,)>,
}

pub struct ExclusiveSystemFn<F> {
    func: F,
}

impl<Param, F> SystemFn<Param, F>
where
    Param: SystemParam,
    F: SystemParamFn<Param, ()>,
{
    #[inline]
    pub fn new(func: F) -> Self {
        Self {
            func,
            prepared: None,
            is_send: Param::IS_SEND,
            marker: PhantomData,
        }
    }
}

impl<F> ExclusiveSystemFn<F>
where
    F: ExclusiveSystemParamFn<()>,
{
    #[inline]
    pub fn new(func: F) -> Self {
        Self { func }
    }
}

#[doc(hidden)]
pub struct SystemFnMarker;
impl<Param, F> IntoSystem<(SystemFnMarker, Param)> for F
where
    Param: SystemParam + 'static,
    F: SystemParamFn<Param, ()>,
{
    fn into_system(self) -> SystemDescriptor {
        SystemDescriptor {
            system_variant: SystemVariant::Concurrent(Box::new(SystemFn::<Param, F>::new(self))),
            dependencies: Vec::new(),
            initialized: false,
            label: None,
            before: Vec::new(),
            after: Vec::new(),
        }
    }
}

// TODO: analyze safety
unsafe impl<Param, F> System for SystemFn<Param, F>
where
    Param: SystemParam + 'static,
    F: SystemParamFn<Param, ()>,
{
    #[inline]
    fn initialize(&mut self, resources: &mut Resources) {
        self.prepared = Some(Param::prepare(resources));
    }

    #[inline]
    fn run<'a>(&'a mut self, resources: &'a Resources) {
        let func = &mut self.func;
        let prepared = self.prepared.as_mut().expect("uninitialized");
        SystemParamFn::call(func, prepared, resources)
    }

    fn is_send(&self) -> bool {
        self.is_send
    }
}

#[doc(hidden)]
pub struct ExclusiveSystemFnMarker;
impl<F> IntoSystem<ExclusiveSystemFnMarker> for F
where
    F: ExclusiveSystemParamFn<()> + 'static,
{
    fn into_system(self) -> SystemDescriptor {
        SystemDescriptor {
            system_variant: SystemVariant::Exclusive(Box::new(ExclusiveSystemFn { func: self })),
            dependencies: Vec::new(),
            initialized: false,
            label: None,
            before: Vec::new(),
            after: Vec::new(),
        }
    }
}

impl<F> ExclusiveSystem for ExclusiveSystemFn<F>
where
    F: ExclusiveSystemParamFn<()>,
{
    #[inline]
    fn run<'l>(&'l mut self, resources: &'l mut Resources) {
        ExclusiveSystemParamFn::call(&mut self.func, resources)
    }
}

pub trait SystemParamFn<Param: SystemParam, Out>: Send + Sync + 'static {
    fn call(&mut self, prepared: &mut Param::Prepared, resources: &Resources) -> Out;
}

pub trait ExclusiveSystemParamFn<Out>: 'static {
    fn call(&mut self, resources: &mut Resources) -> Out;
}

impl<Out, F> SystemParamFn<(), Out> for F
where
    F: FnMut() -> Out + Send + Sync + 'static,
{
    #[inline]
    fn call(&mut self, _prepared: &mut (), _resources: &Resources) -> Out {
        self()
    }
}

impl<Out, F> ExclusiveSystemParamFn<Out> for F
where
    F: FnMut(&mut Resources) -> Out + Send + Sync + 'static,
{
    #[inline]
    fn call(&mut self, resources: &mut Resources) -> Out {
        self(resources)
    }
}

macro_rules! tuple {
  () => ();
  ( $($name:ident.$index:tt,)+ ) => (

      impl<Out, F $(,$name)*> SystemParamFn<($($name,)*),Out> for F
      where
          F: Send + Sync + 'static,
          $($name: SystemParam,)*
          for<'a> &'a mut F: FnMut($($name),*) -> Out
          + FnMut($(<$name::Fetch as SystemParamFetch<'a>>::Output),*) -> Out,
      {
          #[inline]
          fn call(&mut self, prepared: &mut ($($name::Prepared,)*), resources: &Resources) -> Out {
            #[inline]
            fn call_inner<Out, $($name,)*>(
                mut f: impl FnMut($($name,)*)->Out,
                args: ($($name,)*)
            )->Out{
                f($(args.$index,)*)
            }

            let fetched = <<($($name,)*) as SystemParam>::Fetch as SystemParamFetch<'_>>::get(prepared, resources);
            call_inner(self, fetched)
          }
      }

      peel! { tuple [] $($name.$index,)+ }
  )
}

tuple! { T0.0, T1.1, T2.2, T3.3, T4.4, T5.5, T6.6, T7.7, T8.8, T9.9, T10.10, T11.11, }

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{ExclusiveSystemFn, ExclusiveSystemParamFn, SystemFn, SystemParamFn};
    use crate::{
        resource::{Res, ResMut, Resources},
        system::{IntoSystem, SystemDescriptor, SystemVariant},
    };

    struct A(usize);
    struct B(usize);

    fn system_0() {}

    fn system_1(mut a: ResMut<'_, A>) {
        a.0 += 1;
    }

    fn system_2(a: Res<'_, A>, mut b: ResMut<'_, B>) {
        b.0 += a.0;
    }

    fn system_qry_init(resources: &mut Resources) {
        resources.insert(A(22));
        resources.insert(B(11));
    }

    #[allow(unused)]
    fn trait_assertions() {
        let _: Box<dyn SystemParamFn<_, _>> = Box::new(system_0);
        let _ = SystemFn::new(system_0);
        let _ = IntoSystem::into_system(system_0);

        let _: Box<dyn SystemParamFn<_, _>> = Box::new(system_1);
        let _ = SystemFn::new(system_1);
        let _ = IntoSystem::into_system(system_1);

        let _: Box<dyn ExclusiveSystemParamFn<_>> = Box::new(system_qry_init);
        let _ = ExclusiveSystemFn::new(system_qry_init);
        let _ = IntoSystem::into_system(system_qry_init);

        let _: Box<dyn SystemParamFn<_, _>> = Box::new(system_2);
        let _ = SystemFn::new(system_2);
        let _ = IntoSystem::into_system(system_2);
    }

    fn run_system(sys: &mut SystemDescriptor, resources: &mut Resources) {
        match sys.system_variant {
            SystemVariant::Exclusive(ref mut system) => {
                system.initialize(resources);
                system.run(resources);
            }
            SystemVariant::Concurrent(ref mut system) => {
                system.initialize(resources);
                system.run(resources);
            }
        }
    }

    #[test]
    fn test_system_query() {
        let mut resources = Resources::new();

        let mut init_sys = IntoSystem::into_system(system_qry_init);
        run_system(&mut init_sys, &mut resources);

        assert_eq!(22, resources.get_mut::<A>().unwrap().0);
        assert_eq!(11, resources.get_mut::<B>().unwrap().0);

        let mut sys1 = IntoSystem::into_system(system_1);
        run_system(&mut sys1, &mut resources);

        assert_eq!(23, resources.get_mut::<A>().unwrap().0);
        assert_eq!(11, resources.get_mut::<B>().unwrap().0);

        let mut sys2 = IntoSystem::into_system(system_2);
        run_system(&mut sys2, &mut resources);

        assert_eq!(23, resources.get_mut::<A>().unwrap().0);
        assert_eq!(34, resources.get_mut::<B>().unwrap().0);
    }

    #[test]
    fn test_system_fn() {
        let value = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let sys_a = {
            let value = value.clone();
            move |mut a: ResMut<'_, A>| {
                value.store(a.0, std::sync::atomic::Ordering::Release);
                a.0 += 10;
            }
        };

        let mut resources = Resources::new();
        resources.insert(A(11));

        let mut sys = IntoSystem::into_system(sys_a);
        match sys.system_variant {
            SystemVariant::Concurrent(ref mut system) => {
                system.initialize(&mut resources);
                system.run(&resources);
            }
            _ => unreachable!("unexpected value"),
        }

        assert_eq!(11, value.clone().load(std::sync::atomic::Ordering::Acquire));

        assert_eq!(21, resources.get_mut::<A>().unwrap().0);
    }
}
