//! Response types and helpers for dear-imgui-reflect.
//!
//! This module defines [`ReflectResponse`] and [`ReflectEvent`], a lightweight
//! analogue to ImReflect's `ImResponse` that focuses on container-structure
//! changes (insert/remove/reorder/rename). Each response belongs to one
//! [`crate::Inspector`] pass.

/// High-level response information collected during a reflection-driven UI pass.
///
/// This is a lightweight, ImReflect-style response object that records
/// container-level structural edits (insert/remove/reorder/rename) while a
/// reflected editor is rendered. One [`crate::Inspector`] pass owns the response
/// and returns it alongside the pass-wide boolean indicating whether any value
/// changed.
#[derive(Default, Debug)]
pub struct ReflectResponse {
    events: Vec<ReflectEvent>,
}

impl ReflectResponse {
    /// Returns `true` if no events were recorded during the last input pass.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Returns a slice of all events recorded so far.
    pub fn events(&self) -> &[ReflectEvent] {
        &self.events
    }

    /// Clears all recorded events.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub(crate) fn push(&mut self, event: ReflectEvent) {
        self.events.push(event);
    }
}

/// A single structural change observed while rendering reflected UI.
///
/// These events focus on container structure (insert/remove/reorder/rename)
/// rather than low-level pointer or interaction details, providing a
/// simplified analogue to ImReflect's richer `ImResponse` type.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum ReflectEvent {
    /// A vector had an element inserted at the given index.
    VecInserted {
        /// Logical field path associated with the vector, if known.
        path: Option<String>,
        /// Index where the new element was inserted.
        index: usize,
    },
    /// A vector element was removed from the given index.
    VecRemoved {
        /// Logical field path associated with the vector, if known.
        path: Option<String>,
        /// Index from which the element was removed.
        index: usize,
    },
    /// A vector element was moved from `from` to `to` (indices in the final layout).
    VecReordered {
        /// Logical field path associated with the vector, if known.
        path: Option<String>,
        /// Original index of the moved element.
        from: usize,
        /// Final index of the moved element after reordering.
        to: usize,
    },
    /// All elements were removed from a vector that previously contained `previous_len` items.
    VecCleared {
        /// Logical field path associated with the vector, if known.
        path: Option<String>,
        /// Number of elements that were present before the clear operation.
        previous_len: usize,
    },
    /// A fixed-size array had two elements swapped.
    ArrayReordered {
        /// Logical field path associated with the array, if known.
        path: Option<String>,
        /// First index in the swap operation.
        from: usize,
        /// Second index in the swap operation.
        to: usize,
    },
    /// A map entry with the given key was inserted.
    MapInserted {
        /// Logical field path associated with the map, if known.
        path: Option<String>,
        /// Key for the newly inserted entry.
        key: String,
    },
    /// A map entry with the given key was removed.
    MapRemoved {
        /// Logical field path associated with the map, if known.
        path: Option<String>,
        /// Key for the removed entry.
        key: String,
    },
    /// A map entry key was renamed from `from` to `to`.
    MapRenamed {
        /// Logical field path associated with the map, if known.
        path: Option<String>,
        /// Original key of the entry.
        from: String,
        /// New key assigned to the entry.
        to: String,
    },
    /// All entries were removed from a map that previously contained `previous_len` items.
    MapCleared {
        /// Logical field path associated with the map, if known.
        path: Option<String>,
        /// Number of entries that were present before the clear operation.
        previous_len: usize,
    },
}
