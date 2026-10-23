//! JSON replies byte-identical to `serde_json::to_vec(&serde_json::to_value(x)?)`.
//!
//! The JSON-lines protocols have always built replies through `serde_json::Value`,
//! whose objects sort their keys. Building that tree allocates every key and node,
//! which costs several times the simulation step for large observations. This
//! writer emits the same bytes directly: leaves go through serde_json itself, and
//! an object whose keys arrive out of order is reordered once, when it ends.
use serde::ser::{self, Serialize};
use serde_json::Error;

pub(crate) fn to_vec<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, Error> {
    let mut writer = Writer::default();
    value.serialize(&mut writer)?;
    Ok(writer.out)
}

#[derive(Default)]
struct Writer {
    out: Vec<u8>,
    /// Members of every unfinished object, innermost last.
    members: Vec<Member>,
    scratch: Vec<u8>,
}

/// One `"key":value` member; `start..end` excludes its separating comma.
struct Member {
    key: Key,
    start: usize,
    end: usize,
}

enum Key {
    Static(&'static str),
    Owned(String),
}
impl Key {
    fn as_str(&self) -> &str {
        match self {
            Key::Static(key) => key,
            Key::Owned(key) => key,
        }
    }
}

impl Writer {
    fn leaf<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        serde_json::to_writer(&mut self.out, value)
    }

    fn open_object(&mut self) -> Object<'_> {
        self.out.push(b'{');
        Object {
            start: self.out.len() - 1,
            first: self.members.len(),
            pending: None,
            writer: self,
        }
    }
}

struct Object<'a> {
    writer: &'a mut Writer,
    /// Offset of `{` in the output.
    start: usize,
    /// This object's first entry in `Writer::members`.
    first: usize,
    pending: Option<String>,
}
impl<'a> Object<'a> {
    fn member<T: Serialize + ?Sized>(&mut self, key: Key, value: &T) -> Result<(), Error> {
        let w = &mut *self.writer;
        if w.members.len() > self.first {
            w.out.push(b',');
        }
        let start = w.out.len();
        w.leaf(key.as_str())?;
        w.out.push(b':');
        value.serialize(&mut *w)?;
        let end = w.out.len();
        w.members.push(Member { key, start, end });
        Ok(())
    }

    fn finish(self) -> Result<&'a mut Writer, Error> {
        let w = self.writer;
        let members = &mut w.members[self.first..];
        let sorted = members
            .windows(2)
            .all(|pair| pair[0].key.as_str() < pair[1].key.as_str());
        if sorted {
            w.out.push(b'}');
        } else {
            // Stable, so of equal keys the last inserted wins, as in a map.
            members.sort_by(|a, b| a.key.as_str().cmp(b.key.as_str()));
            w.scratch.clear();
            w.scratch.push(b'{');
            for (index, member) in members.iter().enumerate() {
                if members
                    .get(index + 1)
                    .is_some_and(|next| next.key.as_str() == member.key.as_str())
                {
                    continue;
                }
                if w.scratch.len() > 1 {
                    w.scratch.push(b',');
                }
                w.scratch
                    .extend_from_slice(&w.out[member.start..member.end]);
            }
            w.scratch.push(b'}');
            w.out.truncate(self.start);
            w.out.extend_from_slice(&w.scratch);
        }
        w.members.truncate(self.first);
        Ok(w)
    }
}

/// A one-member object wrapping an enum variant's content.
struct Variant<'a> {
    writer: &'a mut Writer,
    first: bool,
}
impl Variant<'_> {
    fn open(writer: &mut Writer, variant: &'static str, open: u8) -> Result<(), Error> {
        writer.out.push(b'{');
        writer.leaf(variant)?;
        writer.out.push(b':');
        writer.out.push(open);
        Ok(())
    }
}

struct Array<'a> {
    writer: &'a mut Writer,
    first: bool,
}
impl Array<'_> {
    fn element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        if !self.first {
            self.writer.out.push(b',');
        }
        self.first = false;
        value.serialize(&mut *self.writer)
    }
}

impl<'a> ser::Serializer for &'a mut Writer {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Array<'a>;
    type SerializeTuple = Array<'a>;
    type SerializeTupleStruct = Array<'a>;
    type SerializeTupleVariant = Variant<'a>;
    type SerializeMap = Object<'a>;
    type SerializeStruct = Object<'a>;
    type SerializeStructVariant = VariantObject<'a>;

    fn serialize_bool(self, v: bool) -> Result<(), Error> {
        self.leaf(&v)
    }
    fn serialize_i8(self, v: i8) -> Result<(), Error> {
        self.leaf(&v)
    }
    fn serialize_i16(self, v: i16) -> Result<(), Error> {
        self.leaf(&v)
    }
    fn serialize_i32(self, v: i32) -> Result<(), Error> {
        self.leaf(&v)
    }
    fn serialize_i64(self, v: i64) -> Result<(), Error> {
        self.leaf(&v)
    }
    fn serialize_i128(self, v: i128) -> Result<(), Error> {
        // `Value` numbers hold at most 64 bits.
        if let Ok(v) = u64::try_from(v) {
            self.leaf(&v)
        } else if let Ok(v) = i64::try_from(v) {
            self.leaf(&v)
        } else {
            Err(ser::Error::custom("number out of range"))
        }
    }
    fn serialize_u8(self, v: u8) -> Result<(), Error> {
        self.leaf(&v)
    }
    fn serialize_u16(self, v: u16) -> Result<(), Error> {
        self.leaf(&v)
    }
    fn serialize_u32(self, v: u32) -> Result<(), Error> {
        self.leaf(&v)
    }
    fn serialize_u64(self, v: u64) -> Result<(), Error> {
        self.leaf(&v)
    }
    fn serialize_u128(self, v: u128) -> Result<(), Error> {
        match u64::try_from(v) {
            Ok(v) => self.leaf(&v),
            Err(_) => Err(ser::Error::custom("number out of range")),
        }
    }
    fn serialize_f32(self, v: f32) -> Result<(), Error> {
        // `Value` widens f32 to f64 before formatting.
        self.leaf(&f64::from(v))
    }
    fn serialize_f64(self, v: f64) -> Result<(), Error> {
        self.leaf(&v)
    }
    fn serialize_char(self, v: char) -> Result<(), Error> {
        self.leaf(&v)
    }
    fn serialize_str(self, v: &str) -> Result<(), Error> {
        self.leaf(v)
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<(), Error> {
        self.leaf(v)
    }
    fn serialize_none(self) -> Result<(), Error> {
        self.leaf(&())
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<(), Error> {
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<(), Error> {
        self.leaf(&())
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), Error> {
        self.leaf(&())
    }
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<(), Error> {
        self.leaf(variant)
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.out.push(b'{');
        self.leaf(variant)?;
        self.out.push(b':');
        value.serialize(&mut *self)?;
        self.out.push(b'}');
        Ok(())
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<Array<'a>, Error> {
        self.out.push(b'[');
        Ok(Array {
            writer: self,
            first: true,
        })
    }
    fn serialize_tuple(self, len: usize) -> Result<Array<'a>, Error> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_struct(self, _name: &'static str, len: usize) -> Result<Array<'a>, Error> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Variant<'a>, Error> {
        Variant::open(self, variant, b'[')?;
        Ok(Variant {
            writer: self,
            first: true,
        })
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<Object<'a>, Error> {
        Ok(self.open_object())
    }
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Object<'a>, Error> {
        Ok(self.open_object())
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<VariantObject<'a>, Error> {
        self.out.push(b'{');
        self.leaf(variant)?;
        self.out.push(b':');
        Ok(VariantObject(self.open_object()))
    }
}

impl ser::SerializeSeq for Array<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.element(value)
    }
    fn end(self) -> Result<(), Error> {
        self.writer.out.push(b']');
        Ok(())
    }
}
impl ser::SerializeTuple for Array<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.element(value)
    }
    fn end(self) -> Result<(), Error> {
        ser::SerializeSeq::end(self)
    }
}
impl ser::SerializeTupleStruct for Array<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.element(value)
    }
    fn end(self) -> Result<(), Error> {
        ser::SerializeSeq::end(self)
    }
}
impl ser::SerializeTupleVariant for Variant<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        if !self.first {
            self.writer.out.push(b',');
        }
        self.first = false;
        value.serialize(&mut *self.writer)
    }
    fn end(self) -> Result<(), Error> {
        self.writer.out.extend_from_slice(b"]}");
        Ok(())
    }
}
impl ser::SerializeMap for Object<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Error> {
        self.pending = Some(key.serialize(MapKey)?);
        Ok(())
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        let key = self
            .pending
            .take()
            .expect("serialize_value called before serialize_key");
        self.member(Key::Owned(key), value)
    }
    fn end(self) -> Result<(), Error> {
        self.finish().map(drop)
    }
}
impl ser::SerializeStruct for Object<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.member(Key::Static(key), value)
    }
    fn end(self) -> Result<(), Error> {
        self.finish().map(drop)
    }
}

struct VariantObject<'a>(Object<'a>);
impl ser::SerializeStructVariant for VariantObject<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.0.member(Key::Static(key), value)
    }
    fn end(self) -> Result<(), Error> {
        self.0.finish()?.out.push(b'}');
        Ok(())
    }
}

/// Object keys as `Value` accepts them: strings, or scalars rendered as strings.
struct MapKey;
impl ser::Serializer for MapKey {
    type Ok = String;
    type Error = Error;
    type SerializeSeq = ser::Impossible<String, Error>;
    type SerializeTuple = ser::Impossible<String, Error>;
    type SerializeTupleStruct = ser::Impossible<String, Error>;
    type SerializeTupleVariant = ser::Impossible<String, Error>;
    type SerializeMap = ser::Impossible<String, Error>;
    type SerializeStruct = ser::Impossible<String, Error>;
    type SerializeStructVariant = ser::Impossible<String, Error>;

    fn serialize_bool(self, v: bool) -> Result<String, Error> {
        Ok(v.to_string())
    }
    fn serialize_i8(self, v: i8) -> Result<String, Error> {
        Ok(v.to_string())
    }
    fn serialize_i16(self, v: i16) -> Result<String, Error> {
        Ok(v.to_string())
    }
    fn serialize_i32(self, v: i32) -> Result<String, Error> {
        Ok(v.to_string())
    }
    fn serialize_i64(self, v: i64) -> Result<String, Error> {
        Ok(v.to_string())
    }
    fn serialize_i128(self, v: i128) -> Result<String, Error> {
        Ok(v.to_string())
    }
    fn serialize_u8(self, v: u8) -> Result<String, Error> {
        Ok(v.to_string())
    }
    fn serialize_u16(self, v: u16) -> Result<String, Error> {
        Ok(v.to_string())
    }
    fn serialize_u32(self, v: u32) -> Result<String, Error> {
        Ok(v.to_string())
    }
    fn serialize_u64(self, v: u64) -> Result<String, Error> {
        Ok(v.to_string())
    }
    fn serialize_u128(self, v: u128) -> Result<String, Error> {
        Ok(v.to_string())
    }
    fn serialize_f32(self, v: f32) -> Result<String, Error> {
        float_key(serde_json::to_string(&v)?, v.is_finite())
    }
    fn serialize_f64(self, v: f64) -> Result<String, Error> {
        float_key(serde_json::to_string(&v)?, v.is_finite())
    }
    fn serialize_char(self, v: char) -> Result<String, Error> {
        Ok(v.to_string())
    }
    fn serialize_str(self, v: &str) -> Result<String, Error> {
        Ok(v.to_owned())
    }
    fn serialize_bytes(self, _v: &[u8]) -> Result<String, Error> {
        Err(key_must_be_a_string())
    }
    fn serialize_none(self) -> Result<String, Error> {
        Err(key_must_be_a_string())
    }
    fn serialize_some<T: Serialize + ?Sized>(self, _value: &T) -> Result<String, Error> {
        Err(key_must_be_a_string())
    }
    fn serialize_unit(self) -> Result<String, Error> {
        Err(key_must_be_a_string())
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<String, Error> {
        Err(key_must_be_a_string())
    }
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<String, Error> {
        Ok(variant.to_owned())
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<String, Error> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<String, Error> {
        Err(key_must_be_a_string())
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Error> {
        Err(key_must_be_a_string())
    }
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Error> {
        Err(key_must_be_a_string())
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        Err(key_must_be_a_string())
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        Err(key_must_be_a_string())
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Err(key_must_be_a_string())
    }
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Error> {
        Err(key_must_be_a_string())
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        Err(key_must_be_a_string())
    }
}

fn key_must_be_a_string() -> Error {
    ser::Error::custom("key must be a string")
}

fn float_key(text: String, finite: bool) -> Result<String, Error> {
    if finite {
        Ok(text)
    } else {
        Err(ser::Error::custom("float key must be finite"))
    }
}

#[cfg(test)]
mod tests {
    use super::to_vec;
    use crate::{Action, Config, Simulator};
    use serde::{Serialize, Serializer};
    use std::collections::{BTreeMap, HashMap};

    fn via_value<T: Serialize + ?Sized>(value: &T) -> Vec<u8> {
        serde_json::to_vec(&serde_json::to_value(value).unwrap()).unwrap()
    }

    fn same<T: Serialize + ?Sized>(value: &T) {
        assert_eq!(
            String::from_utf8(to_vec(value).unwrap()).unwrap(),
            String::from_utf8(via_value(value)).unwrap()
        );
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    enum Shape {
        Unit,
        Newtype(f32),
        Tuple(u8, Option<String>),
        Struct { zeta: i128, alpha: u128, mid: char },
    }

    struct Bytes(Vec<u8>);
    impl Serialize for Bytes {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_bytes(&self.0)
        }
    }

    /// Duplicate keys: a map keeps the last value.
    struct Repeated;
    impl Serialize for Repeated {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            use serde::ser::SerializeMap;
            let mut map = serializer.serialize_map(None)?;
            map.serialize_entry("b", &1)?;
            map.serialize_entry("a", &2)?;
            map.serialize_entry("b", &3)?;
            map.serialize_entry("a", &[4, 5])?;
            map.end()
        }
    }

    /// Non-string keys that `Value` renders as strings.
    struct ScalarKeys(f64);
    impl Serialize for ScalarKeys {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            use serde::ser::SerializeMap;
            let mut map = serializer.serialize_map(Some(6))?;
            map.serialize_entry(&self.0, &0)?;
            map.serialize_entry(&2.5f32, &1)?;
            map.serialize_entry(&'k', &2)?;
            map.serialize_entry(&Shape::Unit, &3)?;
            map.serialize_entry(&-17i128, &4)?;
            map.serialize_entry(&u128::MAX, &5)?;
            map.end()
        }
    }

    #[derive(Serialize)]
    struct Unordered<'a> {
        z: f64,
        y: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        skipped: Option<u8>,
        b: &'a str,
        a: Vec<Shape>,
        m: HashMap<u64, bool>,
        n: BTreeMap<bool, i8>,
        f: BTreeMap<String, f32>,
        nested: Option<Box<Unordered<'a>>>,
        bytes: Bytes,
        repeated: Repeated,
        unit: (),
        tuple: (i64, u16, f32),
    }

    #[test]
    fn writer_matches_value_bytes() {
        let leaf = Unordered {
            z: f64::NAN,
            y: Some(f64::INFINITY),
            skipped: None,
            b: "quote \" slash \\ tab \t nul \u{0} é \u{1F40D} \u{2028}",
            a: vec![
                Shape::Unit,
                Shape::Newtype(0.1),
                Shape::Newtype(f32::NAN),
                Shape::Tuple(7, None),
                Shape::Struct {
                    zeta: -(1 << 62),
                    alpha: u64::MAX as u128,
                    mid: '"',
                },
            ],
            m: (0..20).map(|i| (i * 7919, i % 3 == 0)).collect(),
            n: [(true, -1), (false, 1)].into_iter().collect(),
            f: [("b".into(), 1.5), ("a".into(), -0.0), ("".into(), 1e30)]
                .into_iter()
                .collect(),
            nested: None,
            bytes: Bytes(vec![0, 127, 255]),
            repeated: Repeated,
            unit: (),
            tuple: (i64::MIN, 9, 3.25),
        };
        same(&leaf);
        same(&Unordered {
            nested: Some(Box::new(leaf)),
            skipped: Some(1),
            m: HashMap::new(),
            ..Unordered {
                z: 1e-310,
                y: None,
                skipped: None,
                b: "",
                a: vec![],
                m: HashMap::new(),
                n: BTreeMap::new(),
                f: BTreeMap::new(),
                nested: None,
                bytes: Bytes(vec![]),
                repeated: Repeated,
                unit: (),
                tuple: (0, 0, 0.0),
            }
        });
        same(&ScalarKeys(f64::MIN_POSITIVE));
        assert!(to_vec(&ScalarKeys(f64::NAN)).is_err());
        assert!(serde_json::to_value(ScalarKeys(f64::NAN)).is_err());
        assert!(
            to_vec(&Shape::Struct {
                zeta: i128::MAX,
                alpha: 0,
                mid: 'x'
            })
            .is_err()
        );
        assert!(to_vec(&[(vec![1], 2)].into_iter().collect::<BTreeMap<_, _>>()).is_err());
    }

    #[test]
    fn writer_matches_value_bytes_for_worlds() {
        for (config, agents) in [(Config::sandbox(), 6), (Config::official(40), 40)] {
            let mut world = Simulator::new(3, config).unwrap();
            for slot in 0..agents {
                world.add(slot.to_string(), slot, agents).unwrap();
            }
            for tick in 0..400 {
                let actions: Vec<_> = world
                    .snakes
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        s.alive.then_some(Action {
                            heading: (i * 37 + tick / 50) as f64,
                            boost: (tick + i) % 90 < 20,
                        })
                    })
                    .collect();
                world.step(&actions).unwrap();
            }
            same(&world.observe_all());
            same(&world.deaths);
            same(&world.config);
            same(&world.prey);
            same(&world.food);
            same(&world.snakes);
        }
    }
}
