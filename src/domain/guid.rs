//! Globally unique, stable 128-bit identifiers (`Guid`) compatible with RFC 4122
//! UUID v4 and buildingSMART IFC `IfcGloballyUniqueId` (22-character Base64).
//!
//! Every Trazo project and entity keeps its compact runtime `id: u64` alongside
//! a permanent `Guid` stored in [`GuidRegistry`] on [`crate::domain::model::Project`].
//! Newly created elements receive cryptographically random UUID v4 values via
//! `getrandom`, while legacy files opened without GUIDs are silently upgraded
//! with deterministic GUIDs derived from the project content and entity key.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// buildingSMART IFC `IfcGloballyUniqueId` 64-character alphabet (`0-9A-Za-z_$`).
const IFC_BASE64: &[u8; 64] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";

static FALLBACK_COUNTER: AtomicU64 = AtomicU64::new(1);

/// A 128-bit globally unique identifier (RFC 4122 UUID v4 / IFC `GlobalId`).
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Guid(u128);

impl Guid {
    /// The all-zero nil identifier (`00000000-0000-0000-0000-000000000000`).
    pub const NIL: Self = Self(0);

    /// Generates a fresh random UUID v4 using OS entropy (`getrandom`).
    pub fn new() -> Self {
        let mut bytes = [0u8; 16];
        if getrandom::fill(&mut bytes).is_err() {
            // Deterministic monotonic fallback if OS entropy is unavailable.
            let seq = FALLBACK_COUNTER.fetch_add(1, Ordering::Relaxed);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0x5452_415A_4F00_0001);
            return Self::seeded(now, seq);
        }
        Self::from_v4_bytes(bytes)
    }

    /// Generates a deterministic UUID v4 from `(seed, index)` for reproducible
    /// migration of legacy projects and deterministic tests.
    pub fn seeded(seed: u64, index: u64) -> Self {
        let hi = splitmix64(seed ^ 0x9E37_79B9_7F4A_7C15 ^ splitmix64(index));
        let lo = splitmix64(hi ^ index.wrapping_mul(0xBF58_476D_1CE4_E5B9));
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&hi.to_be_bytes());
        bytes[8..].copy_from_slice(&lo.to_be_bytes());
        Self::from_v4_bytes(bytes)
    }

    fn from_v4_bytes(mut bytes: [u8; 16]) -> Self {
        // RFC 4122 version 4 and variant 10xx.
        bytes[6] = (bytes[6] & 0x0F) | 0x40;
        bytes[8] = (bytes[8] & 0x3F) | 0x80;
        Self(u128::from_be_bytes(bytes))
    }

    pub const fn is_nil(&self) -> bool {
        self.0 == 0
    }

    #[allow(dead_code)]
    pub const fn from_u128(raw: u128) -> Self {
        Self(raw)
    }

    pub const fn as_u128(self) -> u128 {
        self.0
    }

    /// Encodes the 128-bit UUID into the exact 22-character buildingSMART IFC
    /// `IfcGloballyUniqueId` Base64 format (`2 + 21 * 6 = 128` bits).
    pub fn to_ifc_global_id(self) -> String {
        let mut out = String::with_capacity(22);
        let top = ((self.0 >> 126) & 0x03) as usize;
        out.push(IFC_BASE64[top] as char);
        for step in 1..22 {
            let shift = 126 - step * 6;
            let idx = ((self.0 >> shift) & 0x3F) as usize;
            out.push(IFC_BASE64[idx] as char);
        }
        out
    }

    /// Decodes a 22-character buildingSMART IFC `IfcGloballyUniqueId` back into a `Guid`.
    pub fn from_ifc_global_id(raw: &str) -> Option<Self> {
        let bytes = raw.as_bytes();
        if bytes.len() != 22 {
            return None;
        }
        let first = decode_ifc_digit(bytes[0])?;
        if first > 3 {
            return None;
        }
        let mut acc = u128::from(first);
        for &b in &bytes[1..] {
            let digit = decode_ifc_digit(b)?;
            acc = (acc << 6) | u128::from(digit);
        }
        Some(Self(acc))
    }

    /// Parses a `Guid` from canonical UUID (`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`),
    /// braced `{UUID}`, compact 32-char hex UUID, or 22-char IFC `GlobalId`.
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.len() == 22
            && let Some(ifc) = Self::from_ifc_global_id(trimmed)
        {
            return Some(ifc);
        }
        let inner = trimmed
            .strip_prefix('{')
            .and_then(|s| s.strip_suffix('}'))
            .unwrap_or(trimmed);
        if inner.len() == 36 {
            let b = inner.as_bytes();
            if b[8] != b'-' || b[13] != b'-' || b[18] != b'-' || b[23] != b'-' {
                return None;
            }
            let hex: String = inner.chars().filter(|&c| c != '-').collect();
            return u128::from_str_radix(&hex, 16).ok().map(Self);
        }
        if inner.len() == 32 && inner.bytes().all(|b| b.is_ascii_hexdigit()) {
            return u128::from_str_radix(inner, 16).ok().map(Self);
        }
        None
    }

    /// Formats the canonical lowercase RFC 4122 UUID string.
    pub fn to_uuid_string(self) -> String {
        let b = self.0.to_be_bytes();
        format!(
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            b[0],
            b[1],
            b[2],
            b[3],
            b[4],
            b[5],
            b[6],
            b[7],
            b[8],
            b[9],
            b[10],
            b[11],
            b[12],
            b[13],
            b[14],
            b[15]
        )
    }

    /// Compact 8-character prefix for inline UI badges.
    pub fn short_label(self) -> String {
        let full = self.to_uuid_string();
        full.chars().take(8).collect()
    }
}

fn decode_ifc_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'Z' => Some(byte - b'A' + 10),
        b'a'..=b'z' => Some(byte - b'a' + 36),
        b'_' => Some(62),
        b'$' => Some(63),
        _ => None,
    }
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Stable 64-bit FNV-1a hash helper used to seed deterministic GUID migration.
pub(crate) fn fnv1a_seed(parts: &[&[u8]]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for &byte in *part {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_uuid_string())
    }
}

impl fmt::Debug for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Guid({})", self.to_uuid_string())
    }
}

impl Serialize for Guid {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_uuid_string())
    }
}

impl<'de> Deserialize<'de> for Guid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Guid::parse(&raw).ok_or_else(|| serde::de::Error::custom(format!("GUID inválido: '{raw}'")))
    }
}

/// Resolved target of a GUID lookup across all project namespaces.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuidTarget {
    Project,
    Element(u64),
    Layer(u64),
    View(String),
    Sheet(u64),
    Resource(String),
}

/// Persistent GUID table stored alongside a [`crate::domain::model::Project`].
///
/// Keeping GUIDs in a dedicated registry avoids bloating every geometric struct
/// while guaranteeing 100% coverage of all elements, layers, views, sheets and
/// library/block/reference resources, including inactive multi-storey floors.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GuidRegistry {
    /// Stable GUID of the project itself (`IfcProject`).
    #[serde(default, skip_serializing_if = "Guid::is_nil")]
    pub project: Guid,
    /// Stable GUIDs for all entities allocated via `Project::next_id`
    /// (walls, rooms, openings, terrain, lots, fixtures, drawings, groups,
    /// constraints, dimensions, notes, circulations, floors, conduits, beams,
    /// slabs, roofs, balance beams, wall foundations, cantilevers).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub elements: BTreeMap<u64, Guid>,
    /// Stable GUIDs for CAD layers (`Project::layers`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub layers: BTreeMap<u64, Guid>,
    /// Stable GUIDs for sections (`sec:<id>`), elevations (`elv:<id>`) and details (`det:<id>`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub views: BTreeMap<String, Guid>,
    /// Stable GUIDs for sheet items (`Project::sheet_set.sheets`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sheets: BTreeMap<u64, Guid>,
    /// Stable GUIDs for library items, blocks, block instances, underlays, xrefs and masonry material.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub resources: BTreeMap<String, Guid>,
}

impl GuidRegistry {
    pub fn is_empty(&self) -> bool {
        self.project.is_nil()
            && self.elements.is_empty()
            && self.layers.is_empty()
            && self.views.is_empty()
            && self.sheets.is_empty()
            && self.resources.is_empty()
    }

    /// Registers a brand-new random GUID for a newly allocated entity `id`.
    /// Also initializes `self.project` if it has not been assigned yet.
    pub fn register_new_element(&mut self, id: u64) -> Guid {
        if self.project.is_nil() {
            self.project = Guid::new();
        }
        let guid = Guid::new();
        self.elements.insert(id, guid);
        guid
    }

    /// Ensures `id` has a non-nil GUID, assigning a deterministic seeded one
    /// when missing (used during migration of legacy projects).
    pub fn ensure_element(&mut self, id: u64, seed: u64) -> Guid {
        let entry = self.elements.entry(id).or_insert(Guid::NIL);
        if entry.is_nil() {
            *entry = Guid::seeded(seed ^ 0x454C_454D_0000_0001, id);
        }
        *entry
    }

    pub fn ensure_layer(&mut self, id: u64, seed: u64) -> Guid {
        let entry = self.layers.entry(id).or_insert(Guid::NIL);
        if entry.is_nil() {
            *entry = Guid::seeded(seed ^ 0x4C41_5945_5200_0002, id);
        }
        *entry
    }

    pub fn ensure_view(&mut self, key: String, seed: u64) -> Guid {
        let hash = fnv1a_seed(&[b"view", key.as_bytes()]);
        let entry = self.views.entry(key).or_insert(Guid::NIL);
        if entry.is_nil() {
            *entry = Guid::seeded(seed ^ 0x5649_4557_0000_0003, hash);
        }
        *entry
    }

    pub fn ensure_sheet(&mut self, id: u64, seed: u64) -> Guid {
        let entry = self.sheets.entry(id).or_insert(Guid::NIL);
        if entry.is_nil() {
            *entry = Guid::seeded(seed ^ 0x5348_4545_5400_0004, id);
        }
        *entry
    }

    pub fn ensure_resource(&mut self, key: String, seed: u64) -> Guid {
        let hash = fnv1a_seed(&[b"res", key.as_bytes()]);
        let entry = self.resources.entry(key).or_insert(Guid::NIL);
        if entry.is_nil() {
            *entry = Guid::seeded(seed ^ 0x5245_534F_5552_0005, hash);
        }
        *entry
    }

    pub fn element_guid(&self, id: u64) -> Option<Guid> {
        self.elements.get(&id).copied().filter(|g| !g.is_nil())
    }

    pub fn find_element_id(&self, guid: Guid) -> Option<u64> {
        if guid.is_nil() {
            return None;
        }
        self.elements
            .iter()
            .find_map(|(&id, &g)| (g == guid).then_some(id))
    }

    /// Resolves either a numeric `id` string (e.g. `"12"`) or a UUID / 22-char
    /// IFC GlobalId string into an element's numeric `id`.
    pub fn resolve_element_ref(&self, raw: &str) -> Option<u64> {
        let trimmed = raw.trim();
        if let Ok(id) = trimmed.parse::<u64>() {
            return Some(id);
        }
        let guid = Guid::parse(trimmed)?;
        self.find_element_id(guid)
    }

    /// Finds which entity or resource owns `guid` across every namespace.
    #[allow(dead_code)]
    pub fn lookup_any(&self, guid: Guid) -> Option<GuidTarget> {
        if guid.is_nil() {
            return None;
        }
        if self.project == guid {
            return Some(GuidTarget::Project);
        }
        if let Some(id) = self.find_element_id(guid) {
            return Some(GuidTarget::Element(id));
        }
        if let Some(id) = self
            .layers
            .iter()
            .find_map(|(&id, &g)| (g == guid).then_some(id))
        {
            return Some(GuidTarget::Layer(id));
        }
        if let Some(key) = self
            .views
            .iter()
            .find_map(|(k, &g)| (g == guid).then(|| k.clone()))
        {
            return Some(GuidTarget::View(key));
        }
        if let Some(id) = self
            .sheets
            .iter()
            .find_map(|(&id, &g)| (g == guid).then_some(id))
        {
            return Some(GuidTarget::Sheet(id));
        }
        self.resources
            .iter()
            .find_map(|(k, &g)| (g == guid).then(|| GuidTarget::Resource(k.clone())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_guids_are_unique_and_rfc4122_v4() {
        let a = Guid::new();
        let b = Guid::new();
        assert!(!a.is_nil());
        assert!(!b.is_nil());
        assert_ne!(a, b);
        let text = a.to_uuid_string();
        assert_eq!(text.len(), 36);
        assert_eq!(&text[14..15], "4");
        assert!(matches!(&text[19..20], "8" | "9" | "a" | "b"));
        assert_eq!(Guid::parse(&text), Some(a));
        assert_eq!(
            Guid::parse(&format!("{{{}}}", text.to_uppercase())),
            Some(a)
        );
    }

    #[test]
    fn ifc_global_id_round_trips_bijectively() {
        let samples = [
            Guid::NIL,
            Guid::from_u128(u128::MAX),
            Guid::new(),
            Guid::seeded(42, 7),
            Guid::from_u128(0x0123_4567_89AB_CDEF_FEDC_BA98_7654_3210),
        ];
        for g in samples {
            let ifc = g.to_ifc_global_id();
            assert_eq!(ifc.len(), 22, "IFC GlobalId must always be 22 chars");
            assert!(
                matches!(ifc.as_bytes()[0], b'0'..=b'3'),
                "Leading IFC GlobalId char must be 0..=3"
            );
            assert_eq!(Guid::from_ifc_global_id(&ifc), Some(g));
            assert_eq!(Guid::parse(&ifc), Some(g));
        }
        assert!(Guid::from_ifc_global_id("too_short").is_none());
        assert!(Guid::from_ifc_global_id("4000000000000000000000").is_none());
    }

    #[test]
    fn serde_round_trips_uuid_and_accepts_ifc_global_id() {
        let g = Guid::seeded(1234, 99);
        let json = serde_json::to_string(&g).unwrap();
        let back: Guid = serde_json::from_str(&json).unwrap();
        assert_eq!(back, g);

        let ifc_json = serde_json::to_string(&g.to_ifc_global_id()).unwrap();
        let from_ifc: Guid = serde_json::from_str(&ifc_json).unwrap();
        assert_eq!(from_ifc, g);
    }

    #[test]
    fn registry_resolves_numeric_uuid_and_ifc_references() {
        let mut reg = GuidRegistry::default();
        let g = reg.register_new_element(17);
        assert!(!reg.project.is_nil());
        assert_eq!(reg.resolve_element_ref("17"), Some(17));
        assert_eq!(reg.resolve_element_ref(&g.to_uuid_string()), Some(17));
        assert_eq!(reg.resolve_element_ref(&g.to_ifc_global_id()), Some(17));
        assert_eq!(reg.lookup_any(g), Some(GuidTarget::Element(17)));
        assert_eq!(reg.lookup_any(reg.project), Some(GuidTarget::Project));
    }
}
