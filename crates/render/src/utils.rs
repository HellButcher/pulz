pub mod hash {

    #[derive(Default)]
    pub struct SimpleU64Hasher(u64);
    pub type SimpleU64BuildHasher = std::hash::BuildHasherDefault<SimpleU64Hasher>;

    impl std::hash::Hasher for SimpleU64Hasher {
        #[inline]
        fn finish(&self) -> u64 {
            self.0
        }

        #[inline]
        fn write(&mut self, _bytes: &[u8]) {
            panic!("PreHashedHasherHasher should only be used with u64")
        }

        #[inline]
        fn write_u64(&mut self, i: u64) {
            self.0 = i;
        }

        #[inline]
        fn write_i64(&mut self, i: i64) {
            self.0 = i as u64;
        }
    }

    pub type U64HashMap<V> = std::collections::HashMap<u64, V, SimpleU64BuildHasher>;
    pub type TypeIdHashMap<V> =
        std::collections::HashMap<std::any::TypeId, V, SimpleU64BuildHasher>;
}

pub mod serde_slots {
    use std::{
        any::{Any, TypeId},
        cell::RefCell,
        marker::PhantomData,
    };

    use super::hash::{TypeIdHashMap, U64HashMap};

    thread_local! {
        static CURENT_MAPPER: RefCell<TypeIdHashMap<Box<dyn Any>>> = RefCell::new(TypeIdHashMap::default());
    }

    struct TypedEntry<K>(U64HashMap<K>);

    impl<K> TypedEntry<K> {
        fn new() -> Self {
            Self(U64HashMap::default())
        }
        fn new_box_any() -> Box<dyn Any>
        where
            K: 'static,
        {
            Box::new(Self::new())
        }
        fn define(&mut self, k: u64, v: K) -> Option<K> {
            self.0.insert(k, v)
        }
        fn resolve(&self, k: u64) -> Option<&K> {
            self.0.get(&k)
        }
    }
    pub struct SlotDeserializationMapper {
        _private: (),
    }

    impl SlotDeserializationMapper {
        const INSTANCE: Self = Self { _private: () };

        pub fn define<K: slotmap::Key + 'static>(&mut self, old: u64, new: K) -> Option<K> {
            CURENT_MAPPER.with(|m| {
                m.borrow_mut()
                    .entry(TypeId::of::<TypedEntry<K>>())
                    .or_insert_with(TypedEntry::<K>::new_box_any)
                    .downcast_mut::<TypedEntry<K>>()
                    .expect("inconsistend typeid map")
                    .define(old, new)
            })
        }
        pub fn resolve<K: slotmap::Key + 'static>(&self, old: u64) -> Option<K> {
            CURENT_MAPPER.with(|m| {
                m.borrow()
                    .get(&TypeId::of::<TypedEntry<K>>())?
                    .downcast_ref::<TypedEntry<K>>()?
                    .resolve(old)
                    .copied()
            })
        }

        pub fn with<F, R>(f: F) -> R
        where
            F: FnOnce(&mut Self) -> R,
        {
            let is_empty = CURENT_MAPPER.with(|m| m.borrow().is_empty());
            assert!(is_empty, "nested calls are not allowed");
            let mut tmp = Self::INSTANCE;
            let r = f(&mut tmp);
            // free the map
            CURENT_MAPPER.with(|m| std::mem::take(&mut *m.borrow_mut()));
            r
        }
    }

    struct SlotVisitor<K>(PhantomData<K>);

    impl<'de, K: slotmap::Key + 'static> serde::de::Visitor<'de> for SlotVisitor<K> {
        type Value = K;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("an integer between 0 and 2^64")
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.visit_u64(value as u64)
        }

        fn visit_u64<E>(self, old_value: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if let Some(new_value) = SlotDeserializationMapper::INSTANCE.resolve(old_value) {
                Ok(new_value)
            } else {
                Err(E::custom(format!(
                    "The reference {} for {} was not defined",
                    old_value,
                    std::any::type_name::<K>()
                )))
            }
        }
    }

    struct OptionVisitor<T>(PhantomData<T>);
    impl<'de, T> serde::de::Visitor<'de> for OptionVisitor<T>
    where
        T: slotmap::Key + 'static,
    {
        type Value = Option<T>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("option")
        }

        #[inline]
        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }

        #[inline]
        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }

        #[inline]
        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::de::Deserializer<'de>,
        {
            deserialize(deserializer).map(Some)
        }
    }

    struct VecVisitor<T>(PhantomData<T>);

    impl<'de, T> serde::de::Visitor<'de> for VecVisitor<T>
    where
        T: slotmap::Key + 'static,
    {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a sequence")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut values = Vec::with_capacity(seq.size_hint().unwrap_or(0));

            while let Some(SerdeSlotKey(value)) = seq.next_element()? {
                values.push(value);
            }

            Ok(values)
        }
    }

    pub struct SerdeSlotKey<K: slotmap::Key>(pub K);

    impl<K: slotmap::Key + 'static> serde::Serialize for SerdeSlotKey<K> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serialize(&self.0, serializer)
        }
    }

    impl<'de, K: slotmap::Key + 'static> serde::Deserialize<'de> for SerdeSlotKey<K> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            deserialize(deserializer).map(SerdeSlotKey)
        }
    }

    #[inline]
    pub fn serialize<S, K>(value: &K, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
        K: slotmap::Key + 'static,
    {
        s.serialize_u64(value.data().as_ffi())
    }

    #[inline]
    pub fn deserialize<'de, D, K>(d: D) -> Result<K, D::Error>
    where
        D: serde::Deserializer<'de>,
        K: slotmap::Key + 'static,
    {
        d.deserialize_u64(SlotVisitor(PhantomData::<K>))
    }

    pub mod option {
        use std::marker::PhantomData;

        #[inline]
        pub fn serialize<S, K>(value: &Option<K>, s: S) -> Result<S::Ok, S::Error>
        where
            S: serde::ser::Serializer,
            K: slotmap::Key + 'static,
        {
            if let Some(value) = value {
                s.serialize_some(&super::SerdeSlotKey(*value))
            } else {
                s.serialize_none()
            }
        }

        #[inline]
        pub fn deserialize<'de, D, K>(d: D) -> Result<Option<K>, D::Error>
        where
            D: serde::Deserializer<'de>,
            K: slotmap::Key + 'static,
        {
            d.deserialize_u64(super::OptionVisitor(PhantomData::<K>))
        }
    }

    pub mod slice {
        use serde::ser::SerializeSeq;

        #[inline]
        pub fn serialize<S, K>(value: &[K], s: S) -> Result<S::Ok, S::Error>
        where
            S: serde::ser::Serializer,
            K: slotmap::Key + 'static,
        {
            let mut seq = s.serialize_seq(Some(value.len()))?;
            for item in value {
                seq.serialize_element(&super::SerdeSlotKey(*item))?;
            }
            seq.end()
        }
    }

    pub mod vec {
        use std::marker::PhantomData;

        pub use super::slice::serialize;

        #[inline]
        pub fn deserialize<'de, D, K>(d: D) -> Result<Vec<K>, D::Error>
        where
            D: serde::Deserializer<'de>,
            K: slotmap::Key + 'static,
        {
            d.deserialize_seq(super::VecVisitor(PhantomData::<K>))
        }
    }

    pub mod cow_vec {
        pub use super::slice::serialize;

        #[inline]
        pub fn deserialize<'de, D, K>(d: D) -> Result<std::borrow::Cow<'static, [K]>, D::Error>
        where
            D: serde::Deserializer<'de>,
            K: slotmap::Key + 'static,
        {
            let vec = super::vec::deserialize(d)?;
            Ok(std::borrow::Cow::Owned(vec))
        }
    }
}
