use serde::de::DeserializeOwned;
use serde::Serialize;
use std::marker::PhantomData;

#[cfg(feature = "iterator")]
use crate::bound::{Bound, PrefixBound};
#[cfg(feature = "iterator")]
use crate::de::KeyDeserialize;
use crate::helpers::query_raw;
#[cfg(feature = "iterator")]
use crate::iter_helpers::{deserialize_kv, deserialize_v};
#[cfg(feature = "iterator")]
use crate::keys::Prefixer;
use crate::keys::{Key, PrimaryKey};
use crate::path::Path;
#[cfg(feature = "iterator")]
use crate::prefix::{namespaced_prefix_range, Prefix};
use cosmwasm_std::{from_slice, Addr, CustomQuery, QuerierWrapper, StdError, StdResult, Storage};

'a IS THE NAMESPACE
K IS THE KEY_TYPE
T IS THE DATA_TYPE
#[derive(Debug, Clone)]
pub struct Map<'a, K, T> {
    namespace: &'a [u8],
    // see https://doc.rust-lang.org/std/marker/struct.PhantomData.html#unused-type-parameters for why this is needed
    key_type: PhantomData<K>,
    data_type: PhantomData<T>,
}

FIRST IMPLEMENTATION OF THE STRUCTURE ABOVE
ADDING A new FUNCTION THAT RETURNS THE SAME STRUCTURE
impl<'a, K, T> Map<'a, K, T> {
    pub const fn new(namespace: &'a str) -> Self {
        Map {
            namespace: namespace.as_bytes(),
            data_type: PhantomData,
            key_type: PhantomData,
        }
    }

    pub fn namespace(&self) -> &'a [u8] {
        self.namespace
    }
}


impl<'a, K, T> Map<'a, K, T>
where
    T: Serialize + DeserializeOwned,
    K: PrimaryKey<'a>,
{
    THE KEY FUNCTION IS USED TO CREATE OR RETURN A KEY
    IN OUR namespace OR BUCKET
    pub fn key(&self, k: K) -> Path<T> {
        Path::new(
            self.namespace,
            &k.key().iter().map(Key::as_ref).collect::<Vec<_>>(),
        )
    }

    CALLS PREFIX WHICH THEN CREATES A PREFIX WITH EMPTY SUBNAMES (PREFIXES) (&[]) FOR THE NAMESPACE
    #[cfg(feature = "iterator")]
    pub(crate) fn no_prefix_raw(&self) -> Prefix<Vec<u8>, T, K> {
        Prefix::new(self.namespace, &[])
    }

    CALL key THEN CALL cw-storage-plus path.save
    WHICH CALLS storage.set ON THE GIVEN KEY FOUND BY THE KEY FUNC
    pub fn save(&self, store: &mut dyn Storage, k: K, data: &T) -> StdResult<()> {
        self.key(k).save(store, data)
    }

    CALL key THEN CALL cw-storage-plus path.remove
    storage IS Trait, IM UNSURE OF WHAT THAT IS EXACTLY,
    I NEED TO READ MORE ABOUT Rust IN THIS TOPIC.
    IT'S INTERESTING THAT IN IndexedMap.rs THE OPERATION SETS THE ITEM TO None
    BUT THE INDEX REMAINS
    pub fn remove(&self, store: &mut dyn Storage, k: K) {
        self.key(k).remove(store)
    }

    CALLS storage.get BUT THEN CALLS must_deserialize
    WHICH CALLS from_slice THAT CALLS serde_json_wasm::from_slice
    I WONDER IN WHICH FORMAT IS THE JSON STORED FULL JSON OBJ LIKE {"A:TYPE":"B"}
    OR JUST "A:TYPE":"VALUE" 🤔
    /// load will return an error if no data is set at the given key, or on parse error
    pub fn load(&self, store: &dyn Storage, k: K) -> StdResult<T> {
        self.key(k).load(store)
    }

    CALLS storage.get BUT THEN CALLS may_deserialize
    /// may_load will parse the data stored at the key if present, returns Ok(None) if no data there.
    /// returns an error on issues parsing
    pub fn may_load(&self, store: &dyn Storage, k: K) -> StdResult<Option<T>> {
        self.key(k).may_load(store)
    }

    AH THIS IS A DISCOVERY FOR ME
    /// has returns true or false if any data is at this key, without parsing or interpreting the
    /// contents.
    pub fn has(&self, store: &dyn Storage, k: K) -> bool {
        self.key(k).has(store)
    }

    CALLS self.may_load RETURNS Action(Some(value)) OR Action(None) FROM THE may_load RESULT
    IF NO RECORD FOUND IT CREATES IT
    /// Loads the data, perform the specified action, and store the result
    /// in the database. This is shorthand for some common sequences, which may be useful.
    ///
    /// If the data exists, `action(Some(value))` is called. Otherwise `action(None)` is called.
    pub fn update<A, E>(&self, store: &mut dyn Storage, k: K, action: A) -> Result<T, E>
    where
        A: FnOnce(Option<T>) -> Result<T, E>,
        E: From<StdError>,
    {
        self.key(k).update(store, action)
    }

    AND ANOTHER INTERESTING DISCOVERY
    I NEED TO READ FURTHER ON THIS
    /// If you import the proper Map from the remote contract, this will let you read the data
    /// from a remote contract in a type-safe way using WasmQuery::RawQuery
    pub fn query<Q: CustomQuery>(
        &self,
        querier: &QuerierWrapper<Q>,
        remote_contract: Addr,
        k: K,
    ) -> StdResult<Option<T>> {
        let key = self.key(k).storage_key.into();
        let result = query_raw(querier, remote_contract, key)?;
        if result.is_empty() {
            Ok(None)
        } else {
            from_slice(&result).map(Some)
        }
    }
}

GENERATE PREFIX OR SUBPREFIX TO BE USED LATER ON.
THIS TWO RETURN AND OBJECT WITH deserialization
PREFIXES ARE ENCODED USING:
let length_bytes = (namespace.len() as u32).to_be_bytes();
#[cfg(feature = "iterator")]
impl<'a, K, T> Map<'a, K, T>
where
    T: Serialize + DeserializeOwned,
    K: PrimaryKey<'a>,
{
    pub fn sub_prefix(&self, p: K::SubPrefix) -> Prefix<K::SuperSuffix, T, K::SuperSuffix> {
        Prefix::new(self.namespace, &p.prefix())
    }

    pub fn prefix(&self, p: K::Prefix) -> Prefix<K::Suffix, T, K::Suffix> {
        Prefix::new(self.namespace, &p.prefix())
    }
}

// short-cut for simple keys, rather than .prefix(()).range_raw(...)
#[cfg(feature = "iterator")]
impl<'a, K, T> Map<'a, K, T>
where
    T: Serialize + DeserializeOwned,
    // TODO: this should only be when K::Prefix == ()
    // Other cases need to call prefix() first
    K: PrimaryKey<'a>,
{
    MY FIRST INTRODUCTION TO THE Box TYPE, INTERESTING.
    THE MAIN DIFFERENCE BETWEEN prefix_range_raw AND range_raw
    IS THE let prefix = namespaces_with_key(&[namespace], &[]);
    WHICH IF IM NOT WRONG APPENDS THE PREFIX TO THE KEYS NAMES
    OF THE DATA
    /// While `range_raw` over a `prefix` fixes the prefix to one element and iterates over the
    /// remaining, `prefix_range_raw` accepts bounds for the lowest and highest elements of the `Prefix`
    /// itself, and iterates over those (inclusively or exclusively, depending on `PrefixBound`).
    /// There are some issues that distinguish these two, and blindly casting to `Vec<u8>` doesn't
    /// solve them.
    pub fn prefix_range_raw<'c>(
        &self,
        store: &'c dyn Storage,
        min: Option<PrefixBound<'a, K::Prefix>>,
        max: Option<PrefixBound<'a, K::Prefix>>,
        order: cosmwasm_std::Order,
    ) -> Box<dyn Iterator<Item = StdResult<cosmwasm_std::Record<T>>> + 'c>
    where
        T: 'c,
        'a: 'c,
    {
        let mapped =
            namespaced_prefix_range(store, self.namespace, min, max, order).map(deserialize_v);
        Box::new(mapped)
    }
}

#[cfg(feature = "iterator")]
impl<'a, K, T> Map<'a, K, T>
where
    T: Serialize + DeserializeOwned,
    K: PrimaryKey<'a> + KeyDeserialize,
{
    HERE WE SEE THE SAME AS ABOVE BUT IN THIS CASE MAPS
    THE KEY VALUE STORED
    /// While `range` over a `prefix` fixes the prefix to one element and iterates over the
    /// remaining, `prefix_range` accepts bounds for the lowest and highest elements of the
    /// `Prefix` itself, and iterates over those (inclusively or exclusively, depending on
    /// `PrefixBound`).
    /// There are some issues that distinguish these two, and blindly casting to `Vec<u8>` doesn't
    /// solve them.
    pub fn prefix_range<'c>(
        &self,
        store: &'c dyn Storage,
        min: Option<PrefixBound<'a, K::Prefix>>,
        max: Option<PrefixBound<'a, K::Prefix>>,
        order: cosmwasm_std::Order,
    ) -> Box<dyn Iterator<Item = StdResult<(K::Output, T)>> + 'c>
    where
        T: 'c,
        'a: 'c,
        K: 'c,
        K::Output: 'static,
    {

        let mapped = namespaced_prefix_range(store, self.namespace, min, max, order)
            .map(deserialize_kv::<K, T>);
        Box::new(mapped)
    }

    fn no_prefix(&self) -> Prefix<K, T, K> {
        Prefix::new(self.namespace, &[])
    }
}

#[cfg(feature = "iterator")]
impl<'a, K, T> Map<'a, K, T>
where
    T: Serialize + DeserializeOwned,
    K: PrimaryKey<'a>,
{
    GET DATA IN A RANGE WITHOUT USING ANY PREFIX (self.namespace)
    no_prefix_raw CALLS Prefix::new using the self.namespace of Map
    CALLS range_with_prefix WHICH CALLS calc_start_bound AND RETURNS A Vec<u8>
    IF THE STARTING BOUNDARIES TO READ FROM THE BLOCKCHAIN STATE
    RETURNS A Box WITH THE FOUND DATA FOUND IN THE MIN MAX Bound RANGES
    WHICH IS RAW "UNSERIALIZED" DATA
    pub fn range_raw<'c>(
        &self,
        store: &'c dyn Storage,
        min: Option<Bound<'a, K>>,
        max: Option<Bound<'a, K>>,
        order: cosmwasm_std::Order,
    ) -> Box<dyn Iterator<Item = StdResult<cosmwasm_std::Record<T>>> + 'c>
    where
        T: 'c,
    {
        self.no_prefix_raw().range_raw(store, min, max, order)
    }

    SAME AS A BOVE BUT FOR THE KEYS ONLY
    pub fn keys_raw<'c>(
        &self,
        store: &'c dyn Storage,
        min: Option<Bound<'a, K>>,
        max: Option<Bound<'a, K>>,
        order: cosmwasm_std::Order,
    ) -> Box<dyn Iterator<Item = Vec<u8>> + 'c>
    where
        T: 'c,
    {
        self.no_prefix_raw().keys_raw(store, min, max, order)
    }
}

#[cfg(feature = "iterator")]
impl<'a, K, T> Map<'a, K, T>
where
    T: Serialize + DeserializeOwned,
    K: PrimaryKey<'a> + KeyDeserialize,
{
    GET DATA VIA RANGES
    USING DESERIALIZATION VIA
    let de_fn = self.de_fn_kv;
    AND THEN
    .map(move |kv| (de_fn)(store, &pk_name, kv));
    pub fn range<'c>(
        &self,
        store: &'c dyn Storage,
        min: Option<Bound<'a, K>>,
        max: Option<Bound<'a, K>>,
        order: cosmwasm_std::Order,
    ) -> Box<dyn Iterator<Item = StdResult<(K::Output, T)>> + 'c>
    where
        T: 'c,
        K::Output: 'static,
    {
        self.no_prefix().range(store, min, max, order)
    }

    GET KEYS IN RANGE
    pub fn keys<'c>(
        &self,
        store: &'c dyn Storage,
        min: Option<Bound<'a, K>>,
        max: Option<Bound<'a, K>>,
        order: cosmwasm_std::Order,
    ) -> Box<dyn Iterator<Item = StdResult<K::Output>> + 'c>
    where
        T: 'c,
        K::Output: 'static,
    {
        self.no_prefix().keys(store, min, max, order)
    }
}
