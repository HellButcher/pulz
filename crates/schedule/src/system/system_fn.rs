use crate::{
    resource::{FromResources, ResourceId, Resources},
    system::{
        param::{SystemParam, SystemParamFetch, SystemParamItem, SystemParamState},
        ExclusiveSystem, IntoSystemDescriptor, System, SystemDescriptor, SystemVariant,
    },
};

struct SystemFnState<P: SystemParam> {
    param_state: P::Fetch,
    is_send: bool,
}

impl<P: SystemParam> FromResources for SystemFnState<P> {
    #[inline]
    fn from_resources(resources: &mut Resources) -> Self {
        let param_state = <P::Fetch as SystemParamState>::init(resources);
        Self {
            param_state,
            is_send: false, // TODO
        }
    }
}

pub struct SystemFn<P: SystemParam, F> {
    func: F,
    is_send: bool,
    state_resource_id: Option<ResourceId<SystemFnState<P>>>,
}

pub struct ExclusiveSystemFn<F> {
    func: F,
}

impl<P, F> SystemFn<P, F>
where
    P: SystemParam,
    F: SystemParamFn<P, ()>,
{
    #[inline]
    pub fn new(func: F) -> Self {
        Self {
            func,
            is_send: false,
            state_resource_id: None,
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
impl<P, F> IntoSystemDescriptor<(SystemFnMarker, P)> for F
where
    P: SystemParam + 'static,
    F: SystemParamFn<P, ()>,
{
    fn into_system_descriptor(self) -> SystemDescriptor {
        SystemDescriptor {
            system_variant: SystemVariant::Concurrent(Box::new(SystemFn::<P, F>::new(self))),
            dependencies: Vec::new(),
            label: None,
            before: Vec::new(),
            after: Vec::new(),
            is_initialized: false,
            is_send: false,
        }
    }
}

// TODO: analyze safety
unsafe impl<P, F> System for SystemFn<P, F>
where
    P: SystemParam + 'static,
    F: SystemParamFn<P, ()>,
{
    #[inline]
    fn init(&mut self, resources: &mut Resources) {
        let id = *self
            .state_resource_id
            .get_or_insert_with(|| resources.init::<SystemFnState<P>>());
        // TODO: check this:
        self.is_send = resources.borrow_res_id(id).unwrap().is_send;
    }

    #[inline]
    fn run<'a>(&'a mut self, resources: &'a Resources) {
        let state_resource_id = self.state_resource_id.expect("not initialized");
        let mut state = resources
            .borrow_res_mut_id(state_resource_id)
            .expect("state unavailable");
        let params = <P::Fetch as SystemParamFetch>::fetch(&mut state.param_state, resources);
        SystemParamFn::call(&mut self.func, params)
    }

    fn is_send(&self) -> bool {
        self.is_send
    }
}

#[doc(hidden)]
pub struct ExclusiveSystemFnMarker;
impl<F> IntoSystemDescriptor<ExclusiveSystemFnMarker> for F
where
    F: ExclusiveSystemParamFn<()> + 'static,
{
    fn into_system_descriptor(self) -> SystemDescriptor {
        SystemDescriptor {
            system_variant: SystemVariant::Exclusive(Box::new(ExclusiveSystemFn { func: self })),
            dependencies: Vec::new(),
            label: None,
            before: Vec::new(),
            after: Vec::new(),
            is_initialized: false,
            is_send: false,
        }
    }
}

impl<F> ExclusiveSystem for ExclusiveSystemFn<F>
where
    F: ExclusiveSystemParamFn<()>,
{
    fn init(&mut self, _resources: &mut Resources) {}
    #[inline]
    fn run<'l>(&'l mut self, resources: &'l mut Resources) {
        ExclusiveSystemParamFn::call(&mut self.func, resources)
    }
}

pub trait SystemParamFn<P: SystemParam, Out>: Send + Sync + 'static {
    fn call(&mut self, params: SystemParamItem<'_, P>) -> Out;
}

pub trait ExclusiveSystemParamFn<Out>: 'static {
    fn call(&mut self, resources: &mut Resources) -> Out;
}

impl<Out, F> SystemParamFn<(), Out> for F
where
    F: Send + Sync + 'static,
    F: FnMut() -> Out,
{
    #[inline]
    fn call(&mut self, _params: ()) -> Out {
        self()
    }
}

impl<Out, F> ExclusiveSystemParamFn<Out> for F
where
    F: 'static,
    F: FnMut(&mut Resources) -> Out,
{
    #[inline]
    fn call(&mut self, resources: &mut Resources) -> Out {
        self(resources)
    }
}

// impl<Out, F, T0, T1> SystemParamFn<(T0,T1),Out> for F
// where
//     F: Send + Sync + 'static,
//     T0: SystemParam,
//     T1: SystemParam,
//     F: FnMut(SystemParamItem<'_, '_, T0>, SystemParamItem<'_, '_, T1>) -> Out,
// {
//     #[inline]
//     fn call(&mut self, params: SystemParamItem<'_, '_, (T0, T1)>) -> Out {
//         // #[inline]
//         // fn call_inner<Out, $($name,)*>(
//         //     mut f: impl FnMut($($name,)*)->Out,
//         //     args: ($($name,)*)
//         // )->Out{
//         //     f($(args.$index,)*)
//         // }
//         // let fetched = <<($($name,)*) as SystemParam>::Fetch as SystemParamFetch<'_>>::fetch(state, resources);
//         // call_inner(self, fetched)
//         let (p0, p1) = params;
//         self(p0, p1)
//     }
// }

macro_rules! tuple {
  () => ();
  ( $($name:ident.$index:tt,)+ ) => (

      impl<Out, F $(,$name)*> SystemParamFn<($($name,)*),Out> for F
      where
          F: Send + Sync + 'static,
          $($name: SystemParam,)*
          F:
          FnMut($($name),*) -> Out +
            FnMut($(SystemParamItem<'_, $name>),*) -> Out,
      {
          #[inline]
          fn call(&mut self, params: SystemParamItem<'_, ($($name,)*)>) -> Out {
            // #[inline]
            // fn call_inner<Out, $($name,)*>(
            //     mut f: impl FnMut($($name,)*)->Out,
            //     args: ($($name,)*)
            // )->Out{
            //     f($(args.$index,)*)
            // }

            // let fetched = <<($($name,)*) as SystemParam>::Fetch as SystemParamFetch<'_>>::fetch(state, resources);
            // call_inner(self, fetched)
            self($(params.$index,)*)
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
        system::{IntoSystemDescriptor, SystemDescriptor, SystemVariant},
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
        let _ = IntoSystemDescriptor::into_system_descriptor(system_0);

        let _: Box<dyn SystemParamFn<_, _>> = Box::new(system_1);
        let _ = SystemFn::new(system_1);
        let _ = IntoSystemDescriptor::into_system_descriptor(system_1);

        let _: Box<dyn ExclusiveSystemParamFn<_>> = Box::new(system_qry_init);
        let _ = ExclusiveSystemFn::new(system_qry_init);
        let _ = IntoSystemDescriptor::into_system_descriptor(system_qry_init);

        let _: Box<dyn SystemParamFn<_, _>> = Box::new(system_2);
        let _ = SystemFn::new(system_2);
        let _ = IntoSystemDescriptor::into_system_descriptor(system_2);
    }

    fn run_system(sys: &mut SystemDescriptor, resources: &mut Resources) {
        match sys.system_variant {
            SystemVariant::Exclusive(ref mut system) => {
                system.init(resources);
                system.run(resources);
            }
            SystemVariant::Concurrent(ref mut system) => {
                system.init(resources);
                system.run(resources);
            }
        }
    }

    #[test]
    fn test_system_query() {
        let mut resources = Resources::new();

        let mut init_sys = IntoSystemDescriptor::into_system_descriptor(system_qry_init);
        run_system(&mut init_sys, &mut resources);

        assert_eq!(22, resources.get_mut::<A>().unwrap().0);
        assert_eq!(11, resources.get_mut::<B>().unwrap().0);

        let mut sys1 = IntoSystemDescriptor::into_system_descriptor(system_1);
        run_system(&mut sys1, &mut resources);

        assert_eq!(23, resources.get_mut::<A>().unwrap().0);
        assert_eq!(11, resources.get_mut::<B>().unwrap().0);

        let mut sys2 = IntoSystemDescriptor::into_system_descriptor(system_2);
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

        let mut sys = IntoSystemDescriptor::into_system_descriptor(sys_a);
        match sys.system_variant {
            SystemVariant::Concurrent(ref mut system) => {
                system.init(&mut resources);
                system.run(&resources);
            }
            _ => unreachable!("unexpected value"),
        }

        assert_eq!(11, value.load(std::sync::atomic::Ordering::Acquire));

        assert_eq!(21, resources.get_mut::<A>().unwrap().0);
    }
}
