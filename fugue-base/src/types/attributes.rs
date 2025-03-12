use std::borrow::Borrow;

use rustc_hash::FxHashMap;

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct AttributeMap(FxHashMap<String, serde_json::Value>);

impl From<AttributeMap> for FxHashMap<String, serde_json::Value> {
    fn from(value: AttributeMap) -> Self {
        value.0
    }
}

impl From<FxHashMap<String, serde_json::Value>> for AttributeMap {
    fn from(value: FxHashMap<String, serde_json::Value>) -> Self {
        Self(value)
    }
}

impl AttributeMap {
    pub fn new() -> Self {
        Self(Default::default())
    }

    pub fn get_attr<T>(&self, key: impl Borrow<str>) -> Option<T>
    where
        T: serde::de::DeserializeOwned,
    {
        self.0
            .get(key.borrow())
            .and_then(|val| serde_json::from_value(val.clone()).ok())
    }

    pub fn set_attr(&mut self, key: impl ToString, val: impl serde::Serialize) {
        self.0.insert(key.to_string(), serde_json::json!(val));
    }

    pub fn contains(&self, key: impl Borrow<str>) -> bool {
        self.0.contains_key(key.borrow())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[macro_export]
macro_rules! attributes {
    ( $($key:expr => $value:expr),* $(,)? ) => {
        {
            #[allow(unused_mut)]
            let mut attrs = $crate::types::attributes::AttributeMap::new();
            $(
                attrs.set_attr($key, $value);
            )*
            attrs
        }
    };
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use uuid::Uuid;

    #[test]
    fn test_attrs_macro() {
        let guid = Uuid::now_v7();
        let amap = attributes![
            "guid" => guid,
            "path" => "/path/to/my/executable.elf",
        ];

        assert!(amap.contains("path"));
        assert_eq!(
            amap.get_attr::<PathBuf>("path"),
            Some(PathBuf::from("/path/to/my/executable.elf"))
        );

        assert_eq!(amap.get_attr::<Uuid>("guid"), Some(guid));
    }
}
