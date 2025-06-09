pub use pulz_schedule_macros::SystemData;

use crate::resource::{ResourceAccess, Resources, ResourcesSend};

pub trait SystemData {
    type Data: 'static;
    type Arg<'a>: SystemData<Data = Self::Data, Arg<'a> = Self::Arg<'a>>;

    fn init(res: &mut Resources) -> Self::Data;
    fn update_access(res: &Resources, access: &mut ResourceAccess, data: &Self::Data);
    fn get<'a>(res: &'a Resources, data: &'a mut Self::Data) -> Self::Arg<'a>;
}

pub trait SystemDataSend: SystemData<Data: Sync + Send> + Send {
    fn get_send<'a>(res: &'a ResourcesSend, data: &'a mut Self::Data) -> Self::Arg<'a>;
}

#[diagnostic::do_not_recommend]
impl SystemData for () {
    type Data = ();
    type Arg<'a> = ();

    #[inline]
    fn init(_res: &mut Resources) -> Self::Data {}
    #[inline]
    fn update_access(_res: &Resources, _access: &mut ResourceAccess, _data: &Self::Data) {}
    #[inline]
    fn get<'a>(_res: &'a Resources, _data: &'a mut Self::Data) -> Self::Arg<'a> {}
}

#[diagnostic::do_not_recommend]
impl SystemDataSend for () {
    #[inline]
    fn get_send<'a>(_res: &'a ResourcesSend, _data: &'a mut Self::Data) -> Self::Arg<'a> {}
}

macro_rules! impl_system_data {
    ([$(($args_name:ident,$args_index:tt)),*]) => (
        #[diagnostic::do_not_recommend]
        impl<$($args_name: SystemData),*> SystemData for ($($args_name,)*) {
            type Data = ($($args_name::Data,)*);
            type Arg<'a> = ($($args_name::Arg<'a>,)*);
            #[inline]
            fn init(res: &mut Resources) -> Self::Data {
                ($($args_name::init(res),)*)
            }
            #[inline]
            fn update_access(res: &Resources, access: &mut ResourceAccess, data: &Self::Data) {
                $($args_name::update_access(res, access, &data.$args_index);)*
            }
            fn get<'a>(res: &'a Resources, data: &'a mut Self::Data) -> Self::Arg<'a> {
                ($($args_name::get(res, &mut data.$args_index),)*)
            }
        }

        #[diagnostic::do_not_recommend]
        impl<$($args_name: SystemDataSend),*> SystemDataSend for ($($args_name,)*) {
            #[inline]
            fn get_send<'a>(res: &'a ResourcesSend, data: &'a mut Self::Data) -> Self::Arg<'a> {
                ($($args_name::get_send(res, &mut data.$args_index),)*)
            }
        }
    )
}

pulz_functional_utils::generate_variadic_array! {[1..9 T,#] impl_system_data!{}}
