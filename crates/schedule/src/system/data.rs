use crate::resource::{ResourceAccess, Resources, ResourcesSend};

pub trait SystemDataFetch<'a> {
    type Data: 'static;
    fn init(res: &mut Resources) -> Self::Data;
    fn fetch(res: &'a Resources, data: &'a mut Self::Data) -> Self;
    fn update_access(res: &Resources, access: &mut ResourceAccess, data: &Self::Data);
}

pub trait SystemDataFetchSend<'a>: SystemDataFetch<'a, Data: Sync + Send> + Send {
    fn fetch_send(res: &'a ResourcesSend, data: &'a mut Self::Data) -> Self;
}

pub trait SystemData {
    type Data: 'static;
    type Fetch<'a>: SystemDataFetch<'a, Data = Self::Data>;
    type Arg<'a>: SystemData<Arg<'a> = Self::Arg<'a>>;
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Arg<'a>;
}

#[diagnostic::do_not_recommend]
impl SystemDataFetch<'_> for () {
    type Data = ();

    #[inline]
    fn init(_res: &mut Resources) -> Self::Data {}

    #[inline]
    fn fetch(_res: &Resources, _data: &mut Self::Data) -> Self {}

    #[inline]
    fn update_access(_res: &Resources, _access: &mut ResourceAccess, _data: &Self::Data) {}
}

#[diagnostic::do_not_recommend]
impl SystemDataFetchSend<'_> for () {
    #[inline]
    fn fetch_send(_res: &ResourcesSend, _data: &mut Self::Data) -> Self {}
}

#[diagnostic::do_not_recommend]
impl SystemData for () {
    type Data = ();
    type Fetch<'a> = ();
    type Arg<'a> = ();

    #[inline]
    fn get<'a>(_fetch: &'a mut Self::Fetch<'_>) -> Self::Arg<'a> {}
}

macro_rules! impl_system_data {
    ([$($(($name:ident,$index:tt)),+)?]) => (

        impl$(<'a$(,$name)+>)? SystemDataFetch<'a> for ($($($name,)+)?)
        $(
            where
                $($name : SystemDataFetch<'a>),+
        )?
        {
            type Data = ($($($name::Data,)+)?) ;

            #[inline]
            fn init(res: &mut Resources) -> Self::Data {
                $(($($name::init(res),)+))?
            }

            #[inline]
            fn fetch(res: &'a Resources, data: &'a mut Self::Data) -> Self {
                $(($($name::fetch(res, &mut data.$index),)+))?
            }

            #[inline]
            fn update_access(res: &Resources, access: &mut ResourceAccess, data: &Self::Data) {
                $($($name::update_access(res, access, &data.$index);)+)?
            }
        }
        impl$(<'a$(,$name)+>)? SystemDataFetchSend<'a> for ($($($name,)+)?)
        $(
            where
                $($name : SystemDataFetchSend<'a>),+
        )?
        {
            #[inline]
            fn fetch_send(res: &'a ResourcesSend, data: &'a mut Self::Data) -> Self {
                $(($($name::fetch_send(res, &mut data.$index),)+))?
            }
        }
        impl$(<$($name),+>)? SystemData for ($($($name,)+)?)
        $(
            where
                $($name : SystemData),+
        )?
        {
            type Data = ($($($name::Data,)+)?) ;
            type Fetch<'a> = ($($($name::Fetch<'a>,)+)?) ;
            type Arg<'a> =  ($($($name::Arg<'a>,)+)?);

            #[inline]
            fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Arg<'a> {
                $(($($name::get(&mut fetch.$index),)+))?
            }
        }
    )
}

pulz_functional_utils::generate_variadic_array! {[1..9 T,#] impl_system_data!{}}
