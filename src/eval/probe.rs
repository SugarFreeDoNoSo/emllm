//! Instrumentación exclusivamente de pruebas; aislamiento por hilo y restauración
//! durante unwind. El binario de medición no contiene contadores ni ramas extra.
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Calls {
    pub original: usize,
    pub planned: usize,
    pub kernel: usize,
}

#[derive(Clone, Copy, Default)]
struct State {
    reject: bool,
    calls: Calls,
}

thread_local! {
    static STATE: Cell<State> = Cell::new(State::default());
}

pub(crate) enum Entry {
    Original,
    Planned,
    Kernel,
}

pub(crate) fn enter(entry: Entry) {
    let mut state = STATE.get();
    match entry {
        Entry::Original => state.calls.original += 1,
        Entry::Planned => state.calls.planned += 1,
        Entry::Kernel => state.calls.kernel += 1,
    }
    STATE.set(state);
    assert!(!state.reject, "numeric boundary invoked while forbidden");
}

pub(crate) fn observe<T>(reject: bool, work: impl FnOnce() -> T) -> (T, Calls) {
    struct Restore(State);
    impl Drop for Restore {
        fn drop(&mut self) {
            STATE.set(self.0);
        }
    }
    let _restore = Restore(STATE.replace(State {
        reject,
        ..State::default()
    }));
    let result = work();
    (result, STATE.get().calls)
}
