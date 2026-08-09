//! System set labels and the `CoreSystemSet` built-in sets.

use std::sync::LazyLock;

pub use pulz_schedule_macros::derive_label;

pub trait Label {
    type Id: LabelId;

    fn as_label(&self) -> Self::Id;

    #[inline]
    fn as_str(&self) -> &'static str {
        self.as_label().as_str()
    }
}

pub trait LabelId:
    Copy
    + Clone
    + PartialEq
    + Eq
    + PartialOrd
    + Ord
    + std::hash::Hash
    + std::fmt::Debug
    + std::fmt::Display
{
    fn as_str(&self) -> &'static str;
    fn from_static(arr: &'static str) -> Self;
    fn from_static_array<const N: usize>(arr: &'static [&'static str; N]) -> [Self; N];
}

#[macro_export]
macro_rules! define_label_type {
    (
        $(#[$label_attr:meta])*
        $label_name:ident,

        $(#[$id_attr:meta])*
        $id_name:ident $(,)?
    ) => {
        $(#[$id_attr])*
        #[repr(transparent)]
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $id_name(pub $crate::interned::InternedStr);

        impl ::core::fmt::Debug for $id_name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.debug_tuple(stringify!($id_name))
                    .field(&self.0.as_str())
                    .finish()
            }
        }

        impl ::core::fmt::Display for $id_name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::fmt::Display::fmt(&self.0, f)
            }
        }

        $(#[$label_attr])*
        pub trait $label_name: 'static {
            /// Converts this type into an opaque, strongly-typed label.
            fn as_label(&self) -> $id_name {
                $id_name($crate::interned::InternedStr::from_static(self.as_str()))
            }
            /// Returns the representation of this label as a string literal.
            fn as_str(&self) -> &'static str;
        }

        impl <L: $label_name> $crate::label::Label for L {
            type Id = $id_name;
            #[inline]
            fn as_label(&self) -> Self::Id {
                $label_name::as_label(self)
            }
        }

        impl $crate::label::LabelId for $id_name {
            #[inline]
            fn as_str(&self) -> &'static str { self.0.as_str() }

            #[inline]
            fn from_static(arr: &'static str) -> Self {
                Self($crate::interned::InternedStr::from_static(arr))
            }

            #[inline]
            fn from_static_array<const N: usize>(arr: &'static [&'static str; N]) -> [Self; N] {
                $crate::interned::InternedStr::from_static_array(arr).map(|s| Self(s))
            }
        }

        impl $label_name for $id_name {
            #[inline]
            fn as_label(&self) -> Self { *self }
            #[inline]
            fn as_str(&self) -> &'static str { self.0.as_str() }
        }

        // Blanket impl: any T that implements the label trait can be referenced.
        // Note: intentionally omitted to avoid conflicts with enum-specific impls from define_label_enum!
        impl $label_name for &'static str {
            #[inline]
            fn as_str(&self) -> Self { *self }
        }
    };
}

#[macro_export]
macro_rules! define_label_enum {
    (
        $(#[$label_attr:meta])*
        $v:vis enum $enum_name:ident : $label_type:path {
            $($item:ident),* $(,)?
        }
    ) => {
        $(#[$label_attr])*
        #[non_exhaustive]
        #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        $v enum $enum_name {
            $($item),*
        }

        impl $enum_name {

            const _COUNT: usize = {
                let mut count = 0;
                $(
                    #[cfg(false)]
                    let _ = $enum_name::$item;
                    count += 1;
                )*
                count
            };

            /// Cached interned label handles — one `LazyLock` per enum, one allocation total.
            fn __interned_labels() -> &'static [<Self as $crate::label::Label>::Id; Self::_COUNT] {
                static LABELS: LazyLock<[<$enum_name as $crate::label::Label>::Id; $enum_name::_COUNT]> = LazyLock::new(|| <$enum_name as $crate::label::Label>::Id::from_static_array(&[
                    $( concat!(stringify!($enum_name), "::", stringify!($item)), )*
                ]));
                &LABELS
            }
        }

        impl $label_type for $enum_name {
            fn as_str(&self) -> &'static str {
                match self {
                    $( Self::$item => concat!(stringify!($enum_name), "::", stringify!($item)), )*
                }
            }

            fn as_label(&self) -> <Self as $crate::label::Label>::Id {
                let labels = Self::__interned_labels();
                labels[*self as usize]
            }

        }
    };
}

define_label_type!(SystemSet, SystemSetId);

define_label_enum! {
    /// Built-in ordered system sets that bracket each schedule tick: `First`, `Update`, `Last`.
    pub enum CoreSystemSet: SystemSet {
        First,
        Update,
        Last,
    }
}

define_label_enum! {
    #[allow(unused)]
    pub(crate) enum UndefinedSystemSet: SystemSet {
        Undefined
    }
}

/// An opaque label uniquely identifying a system by its name.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct SystemLabel(pub(crate) &'static str);

impl SystemSet for SystemLabel {
    fn as_str(&self) -> &'static str {
        self.0
    }
}
