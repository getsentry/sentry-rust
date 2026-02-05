//! This module contains code for stack-like storage for `HubSwitchGuard`s keyed
//! by tracing span ID.

use std::collections::hash_map::Entry;
use std::collections::HashMap;

use sentry_core::HubSwitchGuard;
use tracing_core::span::Id as SpanId;

/// Holds per-span stacks of `HubSwitchGuard`s to handle span re-entrancy.
///
/// Each time a span is entered, we should push a new guard onto the stack.
/// When the span exits, we should pop the guard from the stack.
pub(super) struct SpanGuardStack {
    /// The map of span IDs to their respective guard stacks.
    guards: HashMap<SpanId, Vec<HubSwitchGuard>>,
}

impl SpanGuardStack {
    /// Creates an empty guard stack map.
    pub(super) fn new() -> Self {
        Self {
            guards: HashMap::new(),
        }
    }

    /// Pushes a guard for the given span ID, creating the stack if needed.
    pub(super) fn push(&mut self, id: SpanId, guard: HubSwitchGuard) {
        self.guards.entry(id).or_default().push(guard);
    }

    /// Pops the most recent guard for the span ID, removing the stack when empty.
    pub(super) fn pop(&mut self, id: SpanId) -> Option<HubSwitchGuard> {
        match self.guards.entry(id) {
            Entry::Occupied(mut entry) => {
                let stack = entry.get_mut();
                let guard = stack.pop();
                if stack.is_empty() {
                    entry.remove();
                }
                guard
            }
            Entry::Vacant(_) => None,
        }
    }

    /// Removes all guards for the span ID without returning them.
    ///
    /// This function guarantees that the guards are dropped in LIFO order.
    /// That way, the hub which was active when the span was first entered
    /// will be the one active after this function returns.
    ///
    /// Typically, remove should only get called once the span is fully
    /// exited, so this removal order guarantee is mostly just defensive.
    pub(super) fn remove(&mut self, id: &SpanId) {
        self.guards
            .remove(id)
            .into_iter()
            .flatten()
            .rev() // <- we drop in reverse order
            .for_each(drop);
    }
}

#[cfg(test)]
mod tests {
    use super::SpanGuardStack;
    use sentry_core::{Hub, HubSwitchGuard};
    use std::sync::Arc;
    use tracing_core::span::Id as SpanId;

    #[test]
    fn pop_is_lifo() {
        let initial = Hub::current();
        let hub_a = Arc::new(Hub::new_from_top(initial.clone()));
        let hub_b = Arc::new(Hub::new_from_top(hub_a.clone()));

        let mut stack = SpanGuardStack::new();
        let id = SpanId::from_u64(1);

        stack.push(id.clone(), HubSwitchGuard::new(hub_a.clone()));
        assert!(Arc::ptr_eq(&Hub::current(), &hub_a));

        stack.push(id.clone(), HubSwitchGuard::new(hub_b.clone()));
        assert!(Arc::ptr_eq(&Hub::current(), &hub_b));

        drop(stack.pop(id.clone()).expect("guard for hub_b"));
        assert!(Arc::ptr_eq(&Hub::current(), &hub_a));

        drop(stack.pop(id.clone()).expect("guard for hub_a"));
        assert!(Arc::ptr_eq(&Hub::current(), &initial));

        assert!(stack.pop(id).is_none());
    }

    #[test]
    fn remove_drops_all_guards_in_lifo_order() {
        let initial = Hub::current();
        let hub_a = Arc::new(Hub::new_from_top(initial.clone()));
        let hub_b = Arc::new(Hub::new_from_top(hub_a.clone()));

        assert!(!Arc::ptr_eq(&hub_b, &initial));
        assert!(!Arc::ptr_eq(&hub_a, &initial));
        assert!(!Arc::ptr_eq(&hub_a, &hub_b));

        let mut stack = SpanGuardStack::new();
        let id = SpanId::from_u64(2);

        stack.push(id.clone(), HubSwitchGuard::new(hub_a.clone()));
        assert!(Arc::ptr_eq(&Hub::current(), &hub_a));

        stack.push(id.clone(), HubSwitchGuard::new(hub_b.clone()));
        assert!(Arc::ptr_eq(&Hub::current(), &hub_b));

        stack.remove(&id);
        assert!(Arc::ptr_eq(&Hub::current(), &initial));
    }
}
