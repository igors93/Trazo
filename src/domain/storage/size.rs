//! Allocation-free estimates of nested serialized values, without formatting
//! numbers, escaping strings or copying image payloads on every undo step.

use serde::{Serialize, Serializer, ser};

pub(crate) fn approximate_value_bytes<T: Serialize + ?Sized>(value: &T) -> usize {
    value
        .serialize(Size)
        .unwrap_or(usize::MAX / 2)
        .saturating_mul(2)
}

type Result<T> = std::result::Result<T, serde_json::Error>;

struct Size;
struct Compound(usize);

impl Compound {
    fn field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.0 = self.0.saturating_add(value.serialize(Size)?);
        Ok(())
    }
}

macro_rules! scalars {
    ($($method:ident($ty:ty)),* $(,)?) => {
        $(fn $method(self, _value: $ty) -> Result<usize> {
            Ok(std::mem::size_of::<$ty>())
        })*
    };
}

impl Serializer for Size {
    type Ok = usize;
    type Error = serde_json::Error;
    type SerializeSeq = Compound;
    type SerializeTuple = Compound;
    type SerializeTupleStruct = Compound;
    type SerializeTupleVariant = Compound;
    type SerializeMap = Compound;
    type SerializeStruct = Compound;
    type SerializeStructVariant = Compound;

    scalars!(
        serialize_bool(bool),
        serialize_i8(i8),
        serialize_i16(i16),
        serialize_i32(i32),
        serialize_i64(i64),
        serialize_i128(i128),
        serialize_u8(u8),
        serialize_u16(u16),
        serialize_u32(u32),
        serialize_u64(u64),
        serialize_u128(u128),
        serialize_f32(f32),
        serialize_f64(f64),
        serialize_char(char)
    );

    fn serialize_str(self, value: &str) -> Result<usize> {
        Ok(24usize.saturating_add(value.len()))
    }
    fn serialize_bytes(self, value: &[u8]) -> Result<usize> {
        Ok(24usize.saturating_add(value.len()))
    }
    fn serialize_none(self) -> Result<usize> {
        Ok(1)
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<usize> {
        Ok(value.serialize(self)?.saturating_add(8))
    }
    fn serialize_unit(self) -> Result<usize> {
        Ok(0)
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<usize> {
        Ok(0)
    }
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
    ) -> Result<usize> {
        Ok(8)
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<usize> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<usize> {
        Ok(value.serialize(self)?.saturating_add(8))
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<Compound> {
        Ok(Compound(24))
    }
    fn serialize_tuple(self, _len: usize) -> Result<Compound> {
        Ok(Compound(0))
    }
    fn serialize_tuple_struct(self, _name: &'static str, _len: usize) -> Result<Compound> {
        Ok(Compound(0))
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Compound> {
        Ok(Compound(8))
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<Compound> {
        Ok(Compound(24))
    }
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Compound> {
        Ok(Compound(0))
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Compound> {
        Ok(Compound(8))
    }
}

macro_rules! sequence {
    ($trait:ident, $method:ident) => {
        impl ser::$trait for Compound {
            type Ok = usize;
            type Error = serde_json::Error;
            fn $method<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
                self.field(value)
            }
            fn end(self) -> Result<usize> {
                Ok(self.0)
            }
        }
    };
}
sequence!(SerializeSeq, serialize_element);
sequence!(SerializeTuple, serialize_element);
sequence!(SerializeTupleStruct, serialize_field);
sequence!(SerializeTupleVariant, serialize_field);

impl ser::SerializeMap for Compound {
    type Ok = usize;
    type Error = serde_json::Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<()> {
        self.0 = self.0.saturating_add(48); // tree/hash entry bookkeeping
        self.field(key)
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.field(value)
    }
    fn end(self) -> Result<usize> {
        Ok(self.0)
    }
}

macro_rules! structure {
    ($trait:ident) => {
        impl ser::$trait for Compound {
            type Ok = usize;
            type Error = serde_json::Error;
            fn serialize_field<T: Serialize + ?Sized>(
                &mut self,
                _key: &'static str,
                value: &T,
            ) -> Result<()> {
                self.field(value)
            }
            fn end(self) -> Result<usize> {
                Ok(self.0)
            }
        }
    };
}
structure!(SerializeStruct);
structure!(SerializeStructVariant);
