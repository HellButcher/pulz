use std::hash::Hash;

#[macro_export]
macro_rules! define_label_type {
    (
        $(#[$label_attr:meta])*
        $label_name:ident,

        $(#[$id_attr:meta])*
        $id_name:ident $(,)?
    ) => {
        $(#[$id_attr])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $id_name(::core::any::TypeId, &'static str);

        impl ::core::fmt::Debug for $id_name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                write!(f, "{}", self.1)
            }
        }

        $(#[$label_attr])*
        pub trait $label_name: 'static {
            /// Converts this type into an opaque, strongly-typed label.
            fn as_label(&self) -> $id_name {
                let id = self.type_id();
                let label = self.as_str();
                $id_name(id, label)
            }
            /// Returns the [`TypeId`] used to differentiate labels.
            fn type_id(&self) -> ::core::any::TypeId {
                ::core::any::TypeId::of::<Self>()
            }
            /// Returns the representation of this label as a string literal.
            ///
            /// In cases where you absolutely need a label to be determined at runtime,
            /// you can use [`Box::leak`] to get a `'static` reference.
            fn as_str(&self) -> &'static str;
        }

        impl $label_name for $id_name {
            fn as_label(&self) -> Self {
                *self
            }
            fn type_id(&self) -> ::core::any::TypeId {
                self.0
            }
            fn as_str(&self) -> &'static str {
                self.1
            }
        }

        impl<L: $label_name> $label_name for &'static L {
            #[inline]
            fn as_str(&self) -> &'static str {
                L::as_str(self)
            }
        }

        impl $label_name for &'static str {
            fn as_str(&self) -> Self {
                self
            }
        }
    };
}

#[macro_export]
macro_rules! define_label_enum {
    (
        $(#[$label_attr:meta])*
        $v:vis enum $enum_name:ident : $label_type:path {
            $( $item:ident ),* $(,)?
        }
    ) => {
        $(#[$label_attr])*
        #[non_exhaustive]
        #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        $v enum $enum_name {
            $( $item ),*
        }

        impl $label_type for $enum_name {
            fn as_str(&self) -> &'static str {
                match self {
                    $(
                        Self::$item => concat!(stringify!($enum_name),"::",stringify!($item)),
                    )*
                }
            }
        }
    };
}

define_label_type!(SystemSet, SystemSetId);

define_label_enum! {
    pub enum CoreSystemSet: SystemSet {
        First,
        Update,
        Last,
    }
}

define_label_enum! {
    pub(crate) enum UndefinedSystemSet: SystemSet {
        Undefined
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct SystemLabel(pub(crate) ::core::any::TypeId, pub(crate) &'static str);

impl SystemSet for SystemLabel {
    fn type_id(&self) -> ::core::any::TypeId {
        self.0
    }
    fn as_str(&self) -> &'static str {
        self.1
    }
}
