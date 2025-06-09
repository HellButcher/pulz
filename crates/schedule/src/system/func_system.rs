use std::marker::PhantomData;

use pulz_functional_utils::{
    func::{FuncMut, FuncOnce},
    tuple::Tuple,
};

use super::{
    SendSystem, System, SystemInit,
    data::{SystemData, SystemDataSend},
};
use crate::{
    resource::{Resources, ResourcesSend},
    system::IntoSystem,
};
pub struct FuncSystem<F, D>
where
    D: SystemData + Tuple,
{
    func: F,
    args_data: Option<D::Data>,
}

impl<F, D> FuncSystem<F, D>
where
    F: FuncMut<D, Output = ()>,
    D: SystemData + Tuple,
{
    pub const fn new(func: F) -> Self {
        Self {
            func,
            args_data: None,
        }
    }
}

#[diagnostic::do_not_recommend]
impl<F, D> SystemInit for FuncSystem<F, D>
where
    F: 'static,
    D: SystemData + Tuple + 'static,
{
    #[inline]
    fn init(&mut self, resources: &mut Resources) {
        self.args_data = Some(D::init(resources));
    }

    #[inline]
    fn system_type_name(&self) -> &'static str {
        std::any::type_name::<F>()
    }

    #[inline]
    fn system_type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<F>()
    }
}

#[diagnostic::do_not_recommend]
impl<F, D> System for FuncSystem<F, D>
where
    F: FuncMut<D, Output = ()> + 'static,
    for<'a> &'a mut F: FuncOnce<D::Arg<'a>, Output = ()> + 'a,
    D: SystemData + Tuple + 'static,
    for<'a> D::Arg<'a>: Tuple,
{
    fn run<'a>(&'a mut self, resources: &'a Resources) {
        let data = self.args_data.as_mut().expect("not initialized");
        let args = D::get(resources, data);
        <&mut F>::call_once(&mut self.func, args);
    }

    #[inline]
    fn update_access(&self, res: &Resources, access: &mut crate::resource::ResourceAccess) {
        D::update_access(
            res,
            access,
            self.args_data.as_ref().expect("not initialized"),
        );
    }
}

#[diagnostic::do_not_recommend]
impl<F, D> SendSystem for FuncSystem<F, D>
where
    F: FuncMut<D, Output = ()> + Send + Sync + 'static,
    for<'a> &'a mut F: FuncOnce<D::Arg<'a>, Output = ()> + 'a,
    D: SystemDataSend + Tuple + 'static,
    for<'a> D::Arg<'a>: Tuple,
{
    fn run_send<'a>(&'a mut self, resources: &'a ResourcesSend) {
        let data = self.args_data.as_mut().expect("not initialized");
        let args = D::get_send(resources, data);
        <&mut F>::call_once(&mut self.func, args);
    }
}

#[doc(hidden)]
pub struct NonExclusiveSystemMarker<D>(PhantomData<fn(D)>);

#[diagnostic::do_not_recommend]
impl<F, D> IntoSystem<NonExclusiveSystemMarker<D>> for F
where
    D: SystemData + Tuple,
    F: FuncMut<D, Output = ()>,
    FuncSystem<F, D>: System,
{
    type System = FuncSystem<F, D>;

    #[inline]
    fn into_system(self) -> Self::System {
        FuncSystem::<F, D>::new(self)
    }
}
