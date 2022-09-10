use super::{IntoExclusiveSystem, IntoSystem};
use crate::{
    resource::{FromResources, ResourceAccess, ResourceId, Resources},
    system::{
        param::{SystemParam, SystemParamFetch, SystemParamItem, SystemParamState},
        ExclusiveSystem, System,
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

pub struct SystemFn<Args, P: SystemParam, F> {
    func: F,
    is_send: bool,
    state_resource_id: Option<ResourceId<SystemFnState<P>>>,
    _phantom: std::marker::PhantomData<fn(Args)>,
}

pub struct ExclusiveSystemFn<F> {
    func: F,
}

impl<Args, P, F> SystemFn<Args, P, F>
where
    P: SystemParam,
    F: SystemParamFn<Args, P, ()>,
{
    #[inline]
    pub fn new(func: F) -> Self {
        Self {
            func,
            is_send: false,
            state_resource_id: None,
            _phantom: std::marker::PhantomData,
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

impl<Args, P, F> IntoSystem<Args, P> for F
where
    P: SystemParam + 'static,
    F: SystemParamFn<Args, P, ()>,
{
    type System = SystemFn<Args, P, F>;
    #[inline]
    fn into_system(self) -> Self::System {
        SystemFn::<Args, P, F>::new(self)
    }
}

// TODO: analyze safety
unsafe impl<Args, P, F> System<Args> for SystemFn<Args, P, F>
where
    P: SystemParam + 'static,
    F: SystemParamFn<Args, P, ()>,
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
    fn run(&mut self, resources: &Resources, args: Args) {
        let state_resource_id = self.state_resource_id.expect("not initialized");
        let mut state = resources
            .borrow_res_mut_id(state_resource_id)
            .expect("state unavailable");
        let params = <P::Fetch as SystemParamFetch>::fetch(&mut state.param_state, resources);
        SystemParamFn::call(&mut self.func, args, params)
    }

    fn is_send(&self) -> bool {
        self.is_send
    }

    #[inline]
    fn update_access(&self, resources: &Resources, access: &mut ResourceAccess) {
        let state_resource_id = self.state_resource_id.expect("not initialized");
        let state = resources
            .borrow_res_id(state_resource_id)
            .expect("state unavailable");
        state.param_state.update_access(resources, access);
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<F>()
    }
}

impl<F> IntoExclusiveSystem<(), ()> for F
where
    F: ExclusiveSystemParamFn<()> + 'static,
{
    type System = ExclusiveSystemFn<F>;
    #[inline]
    fn into_exclusive_system(self) -> Self::System {
        ExclusiveSystemFn { func: self }
    }
}

impl<F> ExclusiveSystem<()> for ExclusiveSystemFn<F>
where
    F: ExclusiveSystemParamFn<()>,
{
    fn init(&mut self, _resources: &mut Resources) {}
    #[inline]
    fn run(&mut self, resources: &mut Resources, _args: ()) {
        ExclusiveSystemParamFn::call(&mut self.func, resources)
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<F>()
    }
}

pub trait SystemParamFn<Args, P: SystemParam, Out>: Send + Sync + 'static {
    fn call(&mut self, arg: Args, params: SystemParamItem<'_, P>) -> Out;
}

pub trait ExclusiveSystemParamFn<Out>: 'static {
    fn call(&mut self, resources: &mut Resources) -> Out;
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

macro_rules! impl_system_fn_sub {
    ( [$(($arg_name:ident,$arg_index:tt)),*] [$(($name:ident,$index:tt)),*]) => (

        impl<Out, F $(,$name)* $(,$arg_name)*> SystemParamFn<($($arg_name,)*), ($($name,)*),Out> for F
        where
            F: Send + Sync + 'static,
            $($name: SystemParam,)*
            F:
              FnMut($($arg_name,)* $($name),*) -> Out +
              FnMut($($arg_name,)* $(SystemParamItem<'_, $name>),*) -> Out,
        {
            #[inline]
            fn call(&mut self, _arg: ($($arg_name,)*), _params: SystemParamItem<'_, ($($name,)*)>) -> Out {
              self($(_arg.$arg_index,)* $(_params.$index,)*)
            }
        }
    )
}

macro_rules! impl_system_fn {
    ([$($args:tt)*]) => (
        pulz_functional_utils::generate_variadic_array! {[B,#] impl_system_fn_sub!{[$($args)*]}}
    )
}

pulz_functional_utils::generate_variadic_array! {[0..9 A,#] impl_system_fn!{}}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{ExclusiveSystemFn, ExclusiveSystemParamFn, SystemFn, SystemParamFn};
    use crate::{
        resource::{Res, ResMut, ResourceAccess, Resources},
        system::{IntoSystem, IntoSystemDescriptor, System, SystemDescriptor, SystemVariant},
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
        let _: Box<dyn SystemParamFn<_, _, _>> = Box::new(system_0);
        let _ = SystemFn::new(system_0);
        let _ = IntoSystemDescriptor::into_system_descriptor(system_0);

        let _: Box<dyn SystemParamFn<(), _, _>> = Box::new(system_1);
        let _ = SystemFn::<(), _, _>::new(system_1);
        let _ = IntoSystemDescriptor::into_system_descriptor(system_1);

        let _: Box<dyn ExclusiveSystemParamFn<_>> = Box::new(system_qry_init);
        let _ = ExclusiveSystemFn::new(system_qry_init);
        let _ = IntoSystemDescriptor::into_system_descriptor(system_qry_init);

        let _: Box<dyn SystemParamFn<(), _, _>> = Box::new(system_2);
        let _ = SystemFn::<(), _, _>::new(system_2);
        let _ = IntoSystemDescriptor::into_system_descriptor(system_2);
    }

    fn run_system(sys: &mut SystemDescriptor, resources: &mut Resources) {
        match sys.system_variant {
            SystemVariant::Exclusive(ref mut system) => {
                system.init(resources);
                system.run(resources, ());
            }
            SystemVariant::Concurrent(ref mut system, ref mut access) => {
                system.init(resources);
                system.update_access(resources, access);
                system.run(resources, ());
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
            SystemVariant::Concurrent(ref mut system, ref mut access) => {
                system.init(&mut resources);
                system.update_access(&resources, access);
                system.run(&resources, ());
            }
            _ => unreachable!("unexpected value"),
        }

        assert_eq!(11, value.load(std::sync::atomic::Ordering::Acquire));

        assert_eq!(21, resources.get_mut::<A>().unwrap().0);
    }

    #[test]
    fn test_system_with_arg() {
        let value = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let sys_a = {
            let value = value.clone();
            move |a: &mut usize, mut b: ResMut<'_, A>| {
                *a += 5;
                value.store(b.0, std::sync::atomic::Ordering::Release);
                b.0 += 10;
            }
        };

        let mut resources = Resources::new();
        resources.insert(A(11));

        let mut access = ResourceAccess::new();
        let mut a = 2;
        {
            let mut sys = IntoSystem::<(&'_ mut usize,), _>::into_system(sys_a);
            sys.init(&mut resources);
            sys.update_access(&resources, &mut access);
            sys.run(&resources, (&mut a,));
        }

        assert_eq!(11, value.load(std::sync::atomic::Ordering::Acquire));

        assert_eq!(21, resources.get_mut::<A>().unwrap().0);
        assert_eq!(7, a);
    }
}
