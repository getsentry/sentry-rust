use std::mem;
use std::thread;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use protocol::Breadcrumb;
use client::Client;

lazy_static! {
    static ref PROCESS_STACK: Mutex<Vec<StackLayer>> = Mutex::new(Vec::new());
}
thread_local! {
    static THREAD_STACK: RefCell<Vec<StackLayer>> = RefCell::new(Vec::new());
}

#[derive(Default)]
pub struct Scope {
    breadcrumbs: Vec<Breadcrumb>,
}

struct StackLayer {
    client: Arc<Client>,
    scope: Scope,
}


fn is_main_thread() -> bool {
    let thread = thread::current();
    let raw_id: u64 = unsafe { mem::transmute(thread.id()) };
    raw_id == 0
}

fn with_process_stack<F, R>(f: F) -> R
    where F: FnOnce(&mut Vec<StackLayer>) -> R
{
    let mut stack = PROCESS_STACK.lock().unwrap();
    f(&mut stack)
}

fn with_thread_stack<F, R>(f: F) -> R
    where F: FnOnce(&mut Vec<StackLayer>) -> R
{
    THREAD_STACK.with(|stack| {
        f(&mut *stack.borrow_mut())
    })
}

fn with_stack<F, R>(f: F, inner: bool) -> R
    where F: FnOnce(&mut Vec<StackLayer>) -> R
{
    let mut rv = None;
    let mut f = Some(f);

    macro_rules! try_stack {
        ($func:ident, $last:expr) => {{
            {
                let inner_rv = &mut rv;
                $func(|mut stack| {
                    if inner || $last || stack.len() > 0 {
                        *inner_rv = Some(f.take().unwrap()(stack));
                    }
                });
            }
            if let Some(rv) = rv {
                return rv;
            }
        }}
    }

    try_stack!(with_thread_stack, false);
    try_stack!(with_process_stack, true);
    unreachable!();
}

pub fn push_scope(client: Arc<Client>) {
    if is_main_thread() {
        with_thread_stack(|stack| {
            let scope = Scope::default();
            stack.push(StackLayer { client, scope });
        });
    }
}
