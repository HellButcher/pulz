use pulz_schedule::resource::{ResourceAccess, Resources, ResourcesSend};

use crate::{archetype::Archetype, component::Components};

/// Cached per-query-type metadata: `ResourceId`s and `ComponentId`s looked up once at
/// query registration and reused across all invocations of the same query type.
pub trait QueryFetchState: Send + Sync + 'static {
    /// Initialises this state using the component registry and any resource ids needed.
    fn init(res: &Resources, components: &Components) -> Self;
    /// Registers the resource accesses required by this fetch state.
    fn update_access(&self, access: &mut ResourceAccess);
    /// Returns `true` if the archetype contains the components required by this fetch.
    fn matches_archetype(&self, archetype: &Archetype) -> bool;
}

/// Declares the `Fetch` type for a query and connects it to a `State`.
/// Pure filters (`With<T>`, `Without<T>`) implement only this trait — no data fetching.
pub trait QueryFilter {
    /// The per-query-type cached state.
    type State: QueryFetchState;
    /// The per-invocation fetch handle borrowing storage resources.
    type Fetch<'w>: QueryFetch<'w, State = Self::State>;
}

/// Declares the item type yielded per entity. Implemented by user-facing query types:
/// `&T`, `&mut T`, `(A, B, ...)`, `Option<Q>`, `Entity`.
///
/// Purely type-level — no methods. Runtime work lives in `QueryFetch`.
pub trait QueryData: QueryFilter {
    /// The type of item yielded for each matching entity.
    type Item<'a>
    where
        Self: 'a;
}

/// Holds borrowed storage handles and extracts per-entity items during iteration.
///
/// - `fetch`: acquires resource borrows once per `Query` construction
/// - `set_archetype`: prepares internal state for iterating an archetype (no-op for most types)
/// - `get`: extracts one item at a slot index; `'a` re-borrows from `self`
///
/// Archetype matching lives on `QueryFetchState::matches_archetype` — no `'w` needed there.
pub trait QueryFetch<'w>: Sized + Send {
    /// The cached state type that matches the state used by this fetch.
    type State: QueryFetchState;
    /// The per-entity item type borrowed from this fetch for lifetime `'a`.
    type Item<'a>
    where
        Self: 'a;

    /// Borrows all required storage resources for the lifetime `'w`.
    fn fetch(res: &'w ResourcesSend, state: &Self::State) -> Self;
    /// Prepares internal pointers for iterating the given archetype.
    fn set_archetype(&mut self, state: &Self::State, archetype: &Archetype);
    /// Extracts the item at `index` within the current archetype.
    fn get<'a>(&'a mut self, archetype: &Archetype, index: usize) -> Self::Item<'a>;
}

/// The item type yielded by iterating `Query<'w, Q>`.
pub type QueryItem<'w, 'a, Q> = <<Q as QueryFilter>::Fetch<'w> as QueryFetch<'w>>::Item<'a>;

// --- () impls ---

impl QueryFetchState for () {
    #[inline]
    fn init(_res: &Resources, _components: &Components) -> Self {}
    #[inline]
    fn update_access(&self, _access: &mut ResourceAccess) {}
    #[inline]
    fn matches_archetype(&self, _archetype: &Archetype) -> bool {
        true
    }
}

#[diagnostic::do_not_recommend]
impl QueryFilter for () {
    type State = ();
    type Fetch<'w> = ();
}

#[diagnostic::do_not_recommend]
impl QueryData for () {
    type Item<'a> = ();
}

#[diagnostic::do_not_recommend]
impl<'w> QueryFetch<'w> for () {
    type State = ();
    type Item<'a> = ();
    #[inline]
    fn fetch(_res: &'w ResourcesSend, _state: &()) -> Self {}
    #[inline]
    fn set_archetype(&mut self, _state: &(), _archetype: &Archetype) {}
    #[inline]
    fn get<'a>(&'a mut self, _archetype: &Archetype, _index: usize) -> Self::Item<'a> {}
}

// --- tuple impls ---

macro_rules! impl_query_tuple {
    ([$(($name:ident, $index:tt)),+]) => {

        impl<$($name: QueryFetchState),+> QueryFetchState for ($($name,)+) {
            #[inline]
            fn init(res: &Resources, components: &Components) -> Self {
                ($($name::init(res, components),)+)
            }
            #[inline]
            fn update_access(&self, access: &mut ResourceAccess) {
                $(self.$index.update_access(access);)+
            }
            #[inline]
            fn matches_archetype(&self, archetype: &Archetype) -> bool {
                $(self.$index.matches_archetype(archetype))&&+
            }
        }

        #[diagnostic::do_not_recommend]
        impl<$($name: QueryFilter),+> QueryFilter for ($($name,)+) {
            type State = ($($name::State,)+);
            type Fetch<'w> = ($($name::Fetch<'w>,)+);
        }

        #[diagnostic::do_not_recommend]
        impl<$($name: QueryData),+> QueryData for ($($name,)+) {
            type Item<'a> = ($($name::Item<'a>,)+) where Self: 'a;
        }

        #[diagnostic::do_not_recommend]
        impl<'w, $($name: QueryFetch<'w>),+> QueryFetch<'w> for ($($name,)+)
        where
            ($($name::State,)+): QueryFetchState,
        {
            type State = ($($name::State,)+);
            type Item<'a> = ($($name::Item<'a>,)+) where Self: 'a;

            #[inline]
            fn fetch(res: &'w ResourcesSend, state: &Self::State) -> Self {
                ($($name::fetch(res, &state.$index),)+)
            }
            #[inline]
            fn set_archetype(&mut self, state: &Self::State, archetype: &Archetype) {
                $(self.$index.set_archetype(&state.$index, archetype);)+
            }
            #[inline]
            fn get<'a>(&'a mut self, archetype: &Archetype, index: usize) -> Self::Item<'a> {
                ($(self.$index.get(archetype, index),)+)
            }
        }
    };
}

pulz_functional_utils::generate_variadic_array! {[1..9 T,#] impl_query_tuple!{}}
