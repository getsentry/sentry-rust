use std::fmt;
use std::sync::{Arc, PoisonError, RwLock};

use crate::client::Client;

pub use sentry_core::{EventProcessor, Scope};

#[derive(Debug)]
pub struct Stack {
    layers: Vec<StackLayer>,
}

#[derive(Debug, Clone)]
pub struct StackLayer {
    pub client: Option<Arc<Client>>,
    pub scope: Arc<Scope>,
}

impl Stack {
    pub fn from_client_and_scope(client: Option<Arc<Client>>, scope: Arc<Scope>) -> Stack {
        Stack {
            layers: vec![StackLayer { client, scope }],
        }
    }

    pub fn push(&mut self) {
        let scope = self.layers[self.layers.len() - 1].clone();
        self.layers.push(scope);
    }

    pub fn pop(&mut self) {
        if self.layers.len() <= 1 {
            panic!("Pop from empty stack");
        }
        self.layers.pop().unwrap();
    }

    pub fn top(&self) -> &StackLayer {
        &self.layers[self.layers.len() - 1]
    }

    pub fn top_mut(&mut self) -> &mut StackLayer {
        let top = self.layers.len() - 1;
        &mut self.layers[top]
    }

    pub fn depth(&self) -> usize {
        self.layers.len()
    }
}

/// A scope guard.
#[derive(Default)]
pub struct ScopeGuard(pub(crate) Option<(Arc<RwLock<Stack>>, usize)>);

impl fmt::Debug for ScopeGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ScopeGuard")
    }
}

impl Drop for ScopeGuard {
    fn drop(&mut self) {
        if let Some((stack, depth)) = self.0.take() {
            let mut stack = stack.write().unwrap_or_else(PoisonError::into_inner);
            if stack.depth() != depth {
                panic!("Tried to pop guards out of order");
            }
            stack.pop();
        }
    }
}
