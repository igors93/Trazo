//! Persistent chunked copy-on-write storage (`Arc<Chunk>`), allocation-tracked
//! `Shared<T>` / `Identity<T>` handles and per-element revision columns (T-04).
//!
//! Together these primitives provide O(1) structural sharing across snapshots
//! (such as undo/redo history and background analysis copies) and O(changed chunks)
//! change detection without retaining old element objects in derived caches.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};

use serde::{Deserialize, Serialize};

/// Number of numeric entity IDs grouped into a single persistent chunk.
pub const CHUNK_SIZE: u64 = 1024;

static REVISION_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Maps an entity `id` to its chunk key (`id / CHUNK_SIZE`).
#[inline]
pub const fn chunk_of(id: u64) -> u64 {
    id / CHUNK_SIZE
}

fn next_revision_token() -> u64 {
    REVISION_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Copy-on-write shared value backed by `Arc<T>`.
///
/// Cloning is an O(1) atomic reference bump; mutable dereferencing detaches via
/// `Arc::make_mut` whenever another clone or weak [`Identity`] observes the
/// allocation.
#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Shared<T>(Arc<T>);

/// Weak allocation identity for a [`Shared<T>`] value.
///
/// Holding an `Identity<T>` detects whether a definition was mutated or replaced
/// without retaining the old value `T` once all strong handles are dropped.
#[allow(dead_code)]
pub struct Identity<T>(Weak<T>);

#[allow(dead_code)]
impl<T> Identity<T> {
    pub fn is_of(&self, value: &Shared<T>) -> bool {
        std::ptr::eq(self.0.as_ptr(), Arc::as_ptr(&value.0))
    }
}

impl<T> Clone for Identity<T> {
    fn clone(&self) -> Self {
        Self(Weak::clone(&self.0))
    }
}

impl<T> Shared<T> {
    pub fn new(value: T) -> Self {
        Self(Arc::new(value))
    }

    pub fn same_allocation(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    #[allow(dead_code)]
    pub fn identity(&self) -> Identity<T> {
        Identity(Arc::downgrade(&self.0))
    }
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T: Default> Default for Shared<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> From<T> for Shared<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> Deref for Shared<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Clone> DerefMut for Shared<T> {
    fn deref_mut(&mut self) -> &mut T {
        Arc::make_mut(&mut self.0)
    }
}

impl<T: PartialEq> PartialEq for Shared<T> {
    fn eq(&self, other: &Self) -> bool {
        self.same_allocation(other) || self.0 == other.0
    }
}

impl<T: Eq> Eq for Shared<T> {}

/// Opaque runtime revision stamp attached to an element ID.
///
/// Preserved across snapshot clones and incremented on every write/mutation so
/// caches can compute exact per-element diffs even across undo/redo branches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Revision(u64);

impl Revision {
    pub fn fresh() -> Self {
        Self(next_revision_token())
    }
}

type RevisionColumn = Arc<Vec<(u64, Revision)>>;

/// Metadata-only, copy-on-write per-element revision table sharing unchanged
/// 1024-ID chunks via `Arc`.
#[derive(Clone, Debug, Default)]
pub struct RevisionSnapshot {
    chunks: BTreeMap<u64, RevisionColumn>,
}

impl RevisionSnapshot {
    #[allow(dead_code)]
    pub fn get(&self, id: u64) -> Option<Revision> {
        let column = self.chunks.get(&chunk_of(id))?;
        column
            .binary_search_by_key(&id, |(entry_id, _)| *entry_id)
            .ok()
            .and_then(|idx| column.get(idx))
            .map(|(_, rev)| *rev)
    }

    /// Stamps `id` with `revision`, splitting only its 1024-ID chunk via CoW.
    pub fn touch(&mut self, id: u64, revision: Revision) {
        let column = Arc::make_mut(self.chunks.entry(chunk_of(id)).or_default());
        match column.binary_search_by_key(&id, |(entry_id, _)| *entry_id) {
            Ok(idx) => {
                if let Some(slot) = column.get_mut(idx) {
                    slot.1 = revision;
                }
            }
            Err(idx) => column.insert(idx, (id, revision)),
        }
    }

    /// Removes `id` from the revision table, pruning empty chunks.
    pub fn remove(&mut self, id: u64) {
        let key = chunk_of(id);
        let Some(column) = self.chunks.get_mut(&key) else {
            return;
        };
        if let Ok(idx) = column.binary_search_by_key(&id, |(entry_id, _)| *entry_id) {
            Arc::make_mut(column).remove(idx);
        }
        if column.is_empty() {
            self.chunks.remove(&key);
        }
    }

    /// Returns all element IDs that were added, removed, or modified since
    /// `before`, skipping unchanged chunks in O(1) via `Arc::ptr_eq`.
    pub fn changed_since(&self, before: &Self) -> BTreeSet<u64> {
        let keys: BTreeSet<u64> = self
            .chunks
            .keys()
            .chain(before.chunks.keys())
            .copied()
            .collect();
        let mut changed = BTreeSet::new();
        for key in keys {
            let (now, old) = (self.chunks.get(&key), before.chunks.get(&key));
            if let (Some(now), Some(old)) = (now, old)
                && Arc::ptr_eq(now, old)
            {
                continue;
            }
            let now_slice = now.map_or(&[][..], |c| c.as_slice());
            let old_slice = old.map_or(&[][..], |c| c.as_slice());
            for (id, rev) in now_slice {
                let prev = old_slice
                    .binary_search_by_key(id, |(entry_id, _)| *entry_id)
                    .ok()
                    .and_then(|idx| old_slice.get(idx))
                    .map(|(_, r)| *r);
                if prev != Some(*rev) {
                    changed.insert(*id);
                }
            }
            for (id, _) in old_slice {
                if now_slice
                    .binary_search_by_key(id, |(entry_id, _)| *entry_id)
                    .is_err()
                {
                    changed.insert(*id);
                }
            }
        }
        changed
    }

    /// Approximate heap bytes retained by this revision metadata table.
    #[allow(dead_code)]
    pub fn retained_bytes(&self) -> usize {
        self.chunks
            .values()
            .fold(std::mem::size_of::<Self>(), |acc, column| {
                acc.saturating_add(
                    column
                        .capacity()
                        .saturating_mul(std::mem::size_of::<(u64, Revision)>()),
                )
                .saturating_add(64)
            })
    }
}

/// Persistent chunked element collection sharing immutable 1024-ID chunks
/// (`Arc<Vec<(u64, T)>>`) across clones and tracking per-element [`Revision`]s.
///
/// Preserves exact original insertion order when elements are not monotonically
/// ordered by `id` (via an optional `Shared<Vec<u64>>` permutation), while
/// remaining zero-overhead when sorted by `id` (the common case).
#[derive(Clone, Debug)]
pub struct ChunkedStore<T> {
    chunks: BTreeMap<u64, Arc<Vec<(u64, T)>>>,
    revisions: RevisionSnapshot,
    len: usize,
    /// Only populated when the source slice was not strictly ascending by `id`,
    /// guaranteeing 100% lossless vector reconstruction.
    custom_order: Option<Shared<Vec<u64>>>,
}

impl<T> Default for ChunkedStore<T> {
    fn default() -> Self {
        Self {
            chunks: BTreeMap::new(),
            revisions: RevisionSnapshot::default(),
            len: 0,
            custom_order: None,
        }
    }
}

impl<T> ChunkedStore<T> {
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.len
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[allow(dead_code)]
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Number of 1024-ID chunks whose underlying `Arc` allocation is physically
    /// shared (`Arc::ptr_eq`) with `other`.
    pub fn shared_chunks_with(&self, other: &Self) -> usize {
        self.chunks
            .iter()
            .filter(|(key, chunk)| {
                other
                    .chunks
                    .get(key)
                    .is_some_and(|other_chunk| Arc::ptr_eq(chunk, other_chunk))
            })
            .count()
    }

    pub fn revisions(&self) -> &RevisionSnapshot {
        &self.revisions
    }

    pub fn get(&self, id: u64) -> Option<&T> {
        let chunk = self.chunks.get(&chunk_of(id))?;
        chunk
            .binary_search_by_key(&id, |(entry_id, _)| *entry_id)
            .ok()
            .and_then(|idx| chunk.get(idx))
            .map(|(_, val)| val)
    }

    /// Approximate heap bytes owned exclusively or referenced by this store's
    /// chunks (counting only chunks NOT physically shared with `base`, when given).
    pub fn incremental_bytes_over(&self, base: Option<&Self>) -> usize {
        let mut total = 0usize;
        for (key, chunk) in &self.chunks {
            let is_shared = base
                .and_then(|b| b.chunks.get(key))
                .is_some_and(|b_chunk| Arc::ptr_eq(chunk, b_chunk));
            if !is_shared {
                total = total
                    .saturating_add(chunk.len().saturating_mul(std::mem::size_of::<(u64, T)>()))
                    .saturating_add(64);
            }
        }
        total
    }
}

impl<T: Clone> ChunkedStore<T> {
    /// Inserts or replaces `value` at `id`, splitting only its 1024-ID chunk
    /// via CoW and assigning a fresh [`Revision`].
    #[allow(dead_code)]
    pub fn insert(&mut self, id: u64, value: T) {
        let chunk = Arc::make_mut(self.chunks.entry(chunk_of(id)).or_default());
        match chunk.binary_search_by_key(&id, |(entry_id, _)| *entry_id) {
            Ok(idx) => {
                if let Some(slot) = chunk.get_mut(idx) {
                    slot.1 = value;
                }
            }
            Err(idx) => {
                chunk.insert(idx, (id, value));
                self.len += 1;
            }
        }
        self.revisions.touch(id, Revision::fresh());
        self.custom_order = None;
    }

    /// Grants copy-on-write mutable access to `id`, splitting only its chunk
    /// and advancing its revision stamp.
    #[allow(dead_code)]
    pub fn get_mut(&mut self, id: u64) -> Option<&mut T> {
        let key = chunk_of(id);
        let idx = self
            .chunks
            .get(&key)?
            .binary_search_by_key(&id, |(entry_id, _)| *entry_id)
            .ok()?;
        self.revisions.touch(id, Revision::fresh());
        let chunk = Arc::make_mut(self.chunks.get_mut(&key)?);
        chunk.get_mut(idx).map(|(_, val)| val)
    }

    /// Removes `id`, pruning empty chunks and updating revision metadata.
    #[allow(dead_code)]
    pub fn remove(&mut self, id: u64) -> Option<T> {
        let key = chunk_of(id);
        let idx = self
            .chunks
            .get(&key)?
            .binary_search_by_key(&id, |(entry_id, _)| *entry_id)
            .ok()?;
        let removed = {
            let chunk = Arc::make_mut(self.chunks.get_mut(&key)?);
            chunk.remove(idx).1
        };
        if self.chunks.get(&key).is_some_and(|c| c.is_empty()) {
            self.chunks.remove(&key);
        }
        self.len = self.len.saturating_sub(1);
        self.revisions.remove(id);
        self.custom_order = None;
        Some(removed)
    }

    /// Reconstructs a flat `Vec<T>` in exact original order.
    pub fn to_vec(&self) -> Vec<T> {
        if let Some(order) = &self.custom_order {
            let mut out = Vec::with_capacity(order.len());
            for &id in order.iter() {
                if let Some(item) = self.get(id) {
                    out.push(item.clone());
                }
            }
            return out;
        }
        let mut out = Vec::with_capacity(self.len);
        for chunk in self.chunks.values() {
            for (_, item) in chunk.iter() {
                out.push(item.clone());
            }
        }
        out
    }
}

impl<T: Clone + PartialEq> ChunkedStore<T> {
    /// Builds a `ChunkedStore<T>` from `items`, structurally sharing (`Arc::clone`)
    /// every 1024-ID chunk (and its revision column!) that is unchanged compared
    /// to `base`.
    pub fn from_slice_sharing(items: &[T], id_of: impl Fn(&T) -> u64, base: Option<&Self>) -> Self {
        if items.is_empty() {
            return Self::default();
        }
        let mut strictly_ascending = true;
        let mut prev_id = None;
        let mut grouped: BTreeMap<u64, Vec<(u64, &T)>> = BTreeMap::new();
        for item in items {
            let id = id_of(item);
            if let Some(prev) = prev_id
                && id <= prev
            {
                strictly_ascending = false;
            }
            prev_id = Some(id);
            grouped.entry(chunk_of(id)).or_default().push((id, item));
        }

        let epoch = Revision::fresh();
        let mut chunks: BTreeMap<u64, Arc<Vec<(u64, T)>>> = BTreeMap::new();
        let mut rev_chunks: BTreeMap<u64, RevisionColumn> = BTreeMap::new();
        let mut unique_len = 0usize;

        for (chunk_key, mut entries) in grouped {
            if !strictly_ascending {
                entries.sort_by_key(|(id, _)| *id);
                entries.dedup_by_key(|(id, _)| *id);
            }
            unique_len += entries.len();

            if let Some(base_store) = base
                && let Some(base_chunk) = base_store.chunks.get(&chunk_key)
                && base_chunk.len() == entries.len()
                && base_chunk
                    .iter()
                    .zip(entries.iter())
                    .all(|((bid, bval), (id, val))| bid == id && bval == *val)
            {
                // Entire 1024-ID chunk is identical: zero-copy Arc share!
                chunks.insert(chunk_key, Arc::clone(base_chunk));
                if let Some(base_rev) = base_store.revisions.chunks.get(&chunk_key) {
                    rev_chunks.insert(chunk_key, Arc::clone(base_rev));
                }
                continue;
            }

            let base_chunk = base.and_then(|b| b.chunks.get(&chunk_key));
            let base_rev = base.and_then(|b| b.revisions.chunks.get(&chunk_key));

            let mut owned_chunk = Vec::with_capacity(entries.len());
            let mut owned_rev = Vec::with_capacity(entries.len());

            for (id, item) in entries {
                let unchanged_rev = base_chunk
                    .and_then(|bc| {
                        bc.binary_search_by_key(&id, |(bid, _)| *bid)
                            .ok()
                            .and_then(|idx| bc.get(idx))
                            .filter(|(_, bval)| bval == item)
                    })
                    .and_then(|_| {
                        base_rev.and_then(|br| {
                            br.binary_search_by_key(&id, |(bid, _)| *bid)
                                .ok()
                                .and_then(|idx| br.get(idx))
                                .map(|(_, r)| *r)
                        })
                    });
                owned_chunk.push((id, item.clone()));
                owned_rev.push((id, unchanged_rev.unwrap_or(epoch)));
            }

            chunks.insert(chunk_key, Arc::new(owned_chunk));
            rev_chunks.insert(chunk_key, Arc::new(owned_rev));
        }

        let custom_order = if strictly_ascending {
            None
        } else {
            let ids: Vec<u64> = items.iter().map(&id_of).collect();
            if let Some(base_order) = base.and_then(|b| b.custom_order.as_ref())
                && base_order.as_slice() == ids.as_slice()
            {
                Some(base_order.clone())
            } else {
                Some(Shared::new(ids))
            }
        };

        Self {
            chunks,
            revisions: RevisionSnapshot { chunks: rev_chunks },
            len: unique_len,
            custom_order,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    #[test]
    fn shared_cow_and_weak_identity_detect_mutations_without_retaining_values() {
        let mut def = Shared::new(vec![10u32, 20]);
        let clone = def.clone();
        assert!(def.same_allocation(&clone));
        let id = def.identity();
        assert!(id.is_of(&def));
        assert!(id.is_of(&clone));

        // Mutating detaches cleanly.
        def.push(30);
        assert!(!def.same_allocation(&clone));
        assert!(!id.is_of(&def));
        assert!(id.is_of(&clone));
        assert_eq!(clone.as_slice(), &[10, 20]);
        assert_eq!(def.as_slice(), &[10, 20, 30]);

        // Serde transparency round-trip.
        let json = serde_json::to_string(&def).unwrap();
        assert_eq!(json, "[10,20,30]");
        let back: Shared<Vec<u32>> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, def);

        // Identity does not retain dropped payloads.
        struct DropCounter(Arc<AtomicUsize>);
        impl Drop for DropCounter {
            fn drop(&mut self) {
                self.0.fetch_add(1, AtomicOrdering::Relaxed);
            }
        }
        let drops = Arc::new(AtomicUsize::new(0));
        let handle = Shared::new(DropCounter(Arc::clone(&drops)));
        let weak_id = handle.identity();
        assert!(weak_id.is_of(&handle));
        drop(handle);
        assert_eq!(drops.load(AtomicOrdering::Relaxed), 1);
        drop(weak_id);
        assert_eq!(drops.load(AtomicOrdering::Relaxed), 1);
    }

    #[test]
    fn chunked_store_shares_unchanged_chunks_and_reports_exact_revision_diffs() {
        let mut items: Vec<(u64, u64)> = (1..=3500).map(|i| (i, i * 10)).collect();
        let snap1 = ChunkedStore::from_slice_sharing(&items, |(id, _)| *id, None);
        assert_eq!(snap1.len(), 3500);
        assert_eq!(snap1.chunk_count(), 4); // 0..1023, 1024..2047, 2048..3071, 3072..3500

        // Edit one item in chunk 1 (id 1500) and append one item in chunk 3 (id 3501).
        items[1499].1 = 999_999;
        items.push((3501, 35_010));
        let snap2 = ChunkedStore::from_slice_sharing(&items, |(id, _)| *id, Some(&snap1));

        // Chunks 0 and 2 must be physically shared via Arc::ptr_eq!
        assert_eq!(snap2.shared_chunks_with(&snap1), 2);
        let diff = snap2.revisions().changed_since(snap1.revisions());
        assert_eq!(diff, BTreeSet::from([1500, 3501]));
        assert_eq!(snap2.to_vec(), items);

        // CoW single-item mutation and removal on a clone leaves original untouched.
        let mut snap3 = snap2.clone();
        assert_eq!(snap3.shared_chunks_with(&snap2), 4);
        *snap3.get_mut(10).unwrap() = (10, 777);
        snap3.remove(20);
        assert_eq!(snap3.shared_chunks_with(&snap2), 3);
        assert_eq!(snap2.get(10), Some(&(10, 100)));
        assert_eq!(snap3.get(10), Some(&(10, 777)));
        assert_eq!(
            snap3.revisions().changed_since(snap2.revisions()),
            BTreeSet::from([10, 20])
        );
    }
}
