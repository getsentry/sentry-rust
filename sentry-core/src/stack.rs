use crate::{Client, Scope};

#[derive(Debug, Clone)]
pub struct StackLayer {
    pub client: Option<Client>,
    pub scope: Scope,
}

#[derive(Debug)]
pub struct Stack {
    layers: Vec<StackLayer>,
}

impl Stack {
    pub fn from_client_and_scope(client: Option<Client>, scope: Scope) -> Stack {
        Stack {
            layers: vec![StackLayer { client, scope }],
        }
    }

    pub fn push(&mut self) {
        let scope = self.top().clone();
        self.layers.push(scope);
    }

    pub fn pop(&mut self) {
        self.layers.pop().expect("Pop from empty stack");
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
