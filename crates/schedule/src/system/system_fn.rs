use super::{data::SystemDataFetch, IntoExclusiveSystem, IntoSystem};
use crate::{
    resource::{ResourceAccess, Resources},
    system::{
        data::{SystemData, SystemDataState},
        ExclusiveSystem, System,
    },
};

#[doc(hidden)]
pub struct SystemFnImpl<Args, P: SystemData, F> {
    func: F,
    is_send: bool,
    state: Option<P::State>,
    _phantom: std::marker::PhantomData<fn(Args)>,
}

#[doc(hidden)]
pub struct ExclusiveSystemFnImpl<Args,F> {
    func: F,
    _phantom: std::marker::PhantomData<fn(Args)>,
}

impl<Args, P, F> SystemFnImpl<Args, P, F>
where
    P: SystemData,
    F: SystemFn<Args, P, ()>,
{
    #[inline]
    pub fn new(func: F) -> Self {
        Self {
            func,
            is_send: false,
            state: None,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<Args, F> ExclusiveSystemFnImpl<Args,F>
where
    F: ExclusiveSystemFn<Args, ()>,
{
    #[inline]
    pub fn new(func: F) -> Self {
        Self { 
            func,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<Args, P, F> IntoSystem<Args, P> for F
where
    P: SystemData,
    F: SystemFn<Args, P, ()>,
{
    type System = SystemFnImpl<Args, P, F>;
    #[inline]
    fn into_system(self) -> Self::System {
        SystemFnImpl::<Args, P, F>::new(self)
    }
}

// TODO: analyze safety
unsafe impl<Args, P, F> System<Args> for SystemFnImpl<Args, P, F>
where
    P: SystemData,
    F: SystemFn<Args, P, ()>,
{
    #[inline]
    fn init(&mut self, resources: &mut Resources) {
        if self.state.is_none() {
            self.state = Some(P::State::init(resources));
        }
        self.is_send = false; // TODO
    }

    #[inline]
    fn run(&mut self, resources: &Resources, args: Args) {
        let state = self.state.as_mut().expect("not initialized");
        let mut params = <P::Fetch<'_> as SystemDataFetch<'_>>::fetch(resources, state);
        SystemFn::call(&mut self.func, args, P::get(&mut params));
    }

    fn is_send(&self) -> bool {
        self.is_send
    }

    #[inline]
    fn update_access(&self, resources: &Resources, access: &mut ResourceAccess) {
        let state = self.state.as_ref().expect("not initialized");
        state.update_access(resources, access);
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<F>()
    }
}

impl<Args,F> IntoExclusiveSystem<Args, ()> for F
where
    F: ExclusiveSystemFn<Args, ()> + 'static,
{
    type System = ExclusiveSystemFnImpl<Args,F>;
    #[inline]
    fn into_exclusive_system(self) -> Self::System {
        ExclusiveSystemFnImpl::<Args, F>::new(self)
    }
}

impl<Args,F> ExclusiveSystem<Args> for ExclusiveSystemFnImpl<Args,F>
where
    F: ExclusiveSystemFn<Args, ()>,
{
    fn init(&mut self, _resources: &mut Resources) {}
    #[inline]
    fn run(&mut self, resources: &mut Resources, args: Args) {
        ExclusiveSystemFn::call(&mut self.func, args, resources)
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<F>()
    }
}

pub trait SystemFn<Args, P: SystemData, Out>: Send + Sync + 'static {
    fn call(&mut self, arg: Args, params: P::Item<'_>) -> Out;
}

pub struct ExclusiveResources<'a>(&'a mut Resources);

impl std::ops::Deref for ExclusiveResources<'_> {
    type Target = Resources;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0
    }
}
impl std::ops::DerefMut for ExclusiveResources<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}


pub trait ExclusiveSystemFn<Args, Out>: 'static {
    fn call(&mut self, args: Args, resources: &mut Resources) -> Out;
}

macro_rules! impl_system_fn_sub {
    ( [$(($arg_name:ident,$arg_index:tt)),*] [$(($name:ident,$index:tt)),*]) => (

        impl<Out, F $(,$name)* $(,$arg_name)*> SystemFn<($($arg_name,)*), ($($name,)*),Out> for F
        where
            F: Send + Sync + 'static,
            $($name: SystemData,)*
            F:
              FnMut($($arg_name,)* $($name),*) -> Out +
              FnMut($($arg_name,)* $($name::Item<'_>),*) -> Out,
        {
            #[inline]
            fn call(&mut self, _arg: ($($arg_name,)*), _params: ($($name::Item<'_>,)*)) -> Out {
              self($(_arg.$arg_index,)* $(_params.$index,)*)
            }
        }
    )
}

macro_rules! impl_exclusive_system_fn {
    ( [$(($arg_name:ident,$arg_index:tt)),*]) => (

        impl<Out, F $(,$arg_name)*> ExclusiveSystemFn<($($arg_name,)*),Out> for F
        where
            F: FnMut($($arg_name,)* ExclusiveResources<'_>) -> Out + 'static,
        {
            #[inline]
            fn call(&mut self, _arg: ($($arg_name,)*), _res: &mut Resources) -> Out {
              self($(_arg.$arg_index,)* ExclusiveResources(_res))
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
pulz_functional_utils::generate_variadic_array! {[0..9 A,#] impl_exclusive_system_fn!{}}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        ExclusiveSystemFnImpl, SystemFnImpl,
        SystemFn, ExclusiveResources,
    };
    use crate::{
        resource::{ResourceAccess, Resources},
        system::{IntoSystem, System, SystemDescriptor, SystemVariant}, prelude::IntoExclusiveSystem,
    };

    struct A(usize);
    struct B(usize);

    fn system_0() {}

    fn system_1(mut a: &mut A) {
        a.0 += 1;
    }

    fn system_2(a: &A, b: &mut B) {
        b.0 += a.0;
    }

    fn system_qry_init(mut resources: ExclusiveResources<'_>) {
        resources.insert(A(22));
        resources.insert(B(11));
    }

    #[allow(unused)]
    fn trait_assertions() {
        let _: Box<dyn SystemFn<_, _, _>> = Box::new(system_0);
        let _ = SystemFnImpl::new(system_0);
        let _ = IntoSystem::<(),_>::into_system(system_0);

        let _: Box<dyn SystemFn<(), _, _>> = Box::new(system_1);
        let _ = SystemFnImpl::<(), _, _>::new(system_1);
        let _ = IntoSystem::<(),_>::into_system(system_1);

        let _ = ExclusiveSystemFnImpl::new(system_qry_init);
        let _ = IntoExclusiveSystem::<(),_>::into_exclusive_system(system_qry_init);

        let _: Box<dyn SystemFn<(), _, _>> = Box::new(system_2);
        let _ = SystemFnImpl::<(), _, _>::new(system_2);
        let _ = IntoSystem::<(),_>::into_system(system_2);
    }

    #[test]
    fn test_system_query() {
        let mut resources = Resources::new();
        resources.insert(A(11));
        resources.insert(B(12));

        resources.run(system_qry_init);

        assert_eq!(22, resources.get_mut::<A>().unwrap().0);
        assert_eq!(11, resources.get_mut::<B>().unwrap().0);

        resources.run(system_1);

        assert_eq!(23, resources.get_mut::<A>().unwrap().0);
        assert_eq!(11, resources.get_mut::<B>().unwrap().0);

        resources.run(system_2);

        assert_eq!(23, resources.get_mut::<A>().unwrap().0);
        assert_eq!(34, resources.get_mut::<B>().unwrap().0);
    }

    #[test]
    fn test_system_fn() {
        let value = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let sys_a = {
            let value = value.clone();
            move |mut a: &mut A| {
                value.store(a.0, std::sync::atomic::Ordering::Release);
                a.0 += 10;
            }
        };

        let mut resources = Resources::new();
        resources.insert(A(11));

        resources.run(sys_a);

        assert_eq!(11, value.load(std::sync::atomic::Ordering::Acquire));

        assert_eq!(21, resources.get_mut::<A>().unwrap().0);
    }

    #[test]
    fn test_system_with_arg() {
        let value = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let sys_a = {
            let value = value.clone();
            move |a: &mut usize, mut b: &mut A| {
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
