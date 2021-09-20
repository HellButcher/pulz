use std::marker::PhantomData;

use crate::World;

use super::{
    param::{SystemParam, SystemParamFetch},
    ExclusiveSystem, IntoSystem, System, SystemDescriptor, SystemVariant,
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
    fn initialize(&mut self, world: &mut World) {
        self.prepared = Some(Param::prepare(world));
    }

    #[inline]
    fn run<'a>(&'a mut self, world: &'a World) {
        let func = &mut self.func;
        let prepared = self.prepared.as_mut().expect("uninitialized");
        SystemParamFn::call(func, prepared, world)
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
        }
    }
}

impl<F> ExclusiveSystem for ExclusiveSystemFn<F>
where
    F: ExclusiveSystemParamFn<()>,
{
    #[inline]
    fn run<'l>(&'l mut self, world: &'l mut World) {
        ExclusiveSystemParamFn::call(&mut self.func, world)
    }
}

pub trait SystemParamFn<Param: SystemParam, Out>: Send + Sync + 'static {
    fn call(&mut self, prepared: &mut Param::Prepared, world: &World) -> Out;
}

pub trait ExclusiveSystemParamFn<Out>: 'static {
    fn call(&mut self, world: &mut World) -> Out;
}

impl<Out, F> SystemParamFn<(), Out> for F
where
    F: FnMut() -> Out + Send + Sync + 'static,
{
    #[inline]
    fn call(&mut self, _prepared: &mut (), _world: &World) -> Out {
        self()
    }
}

impl<Out, F> ExclusiveSystemParamFn<Out> for F
where
    F: FnMut(&mut World) -> Out + Send + Sync + 'static,
{
    #[inline]
    fn call(&mut self, world: &mut World) -> Out {
        self(world)
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
          fn call(&mut self, prepared: &mut ($($name::Prepared,)*), world: &World) -> Out {
            #[inline]
            fn call_inner<Out, $($name,)*>(
                mut f: impl FnMut($($name,)*)->Out,
                args: ($($name,)*)
            )->Out{
                f($(args.$index,)*)
            }

            let fetched = <<($($name,)*) as SystemParam>::Fetch as SystemParamFetch<'_>>::get(prepared, world);
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
        query::exec::Query,
        resource::ResMut,
        system::{IntoSystem, SystemVariant},
        world::World,
    };

    struct A(usize);

    fn system_a() {}

    fn system_b(_a: ResMut<'_, A>) {}

    fn system_qry_init(world: &mut World) {
        world.spawn().insert(A(22));
        world.spawn().insert(A(11));
    }

    fn system_qry(mut q: Query<'_, &mut A>) {
        for e in q.iter() {
            e.0 *= 2;
        }
    }

    #[allow(unused)]
    fn trait_assertions() {
        let _: Box<dyn SystemParamFn<_, _>> = Box::new(system_a);
        let _ = SystemFn::new(system_a);
        let _ = IntoSystem::into_system(system_a);

        let _: Box<dyn SystemParamFn<_, _>> = Box::new(system_b);
        let _ = SystemFn::new(system_b);
        let _ = IntoSystem::into_system(system_b);

        let _: Box<dyn ExclusiveSystemParamFn<_>> = Box::new(system_qry_init);
        let _ = ExclusiveSystemFn::new(system_qry_init);
        let _ = IntoSystem::into_system(system_qry_init);

        let _: Box<dyn SystemParamFn<_, _>> = Box::new(system_qry);
        let _ = SystemFn::new(system_qry);
        let _ = IntoSystem::into_system(system_qry);
    }

    #[test]
    fn test_system_query() {
        let mut world = World::new();

        assert_eq!(0, world.entities().len());

        let mut init_sys = IntoSystem::into_system(system_qry_init);
        match init_sys.system_variant {
            SystemVariant::Exclusive(ref mut system) => {
                system.initialize(&mut world);
                system.run(&mut world);
            }
            _ => unreachable!("unexpected value"),
        }

        assert_eq!(2, world.entities().len());

        let mut sys = IntoSystem::into_system(system_qry);
        match sys.system_variant {
            SystemVariant::Concurrent(ref mut system) => {
                system.initialize(&mut world);
                system.run(&world);
            }
            _ => unreachable!("unexpected value"),
        }

        let mut sum = 0;
        for e in world.query::<&A>().iter() {
            sum += e.0;
        }

        assert_eq!(66, sum);
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

        let mut world = World::new();
        world.insert_resource(A(11));

        let mut sys = IntoSystem::into_system(sys_a);
        match sys.system_variant {
            SystemVariant::Concurrent(ref mut system) => {
                system.initialize(&mut world);
                system.run(&world);
            }
            _ => unreachable!("unexpected value"),
        }

        assert_eq!(11, value.clone().load(std::sync::atomic::Ordering::Acquire));

        assert_eq!(21, world.resources().borrow::<A>().unwrap().0);
    }
}
