use bevy_app::{App, Plugin, PreUpdate};
use bevy_ecs::{component::Component, query::Added, schedule::IntoSystemConfigs, system::Query};
use std::{any::Any, collections::VecDeque, ops::Deref, sync::Arc};

pub struct IslandsUiPlugin;

impl Plugin for IslandsUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, (initial_compose, recompose).chain());
    }
}

// ===
// Scope
// ===

#[derive(Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct ScopeId(usize);

pub struct Scope<'a> {
    id: ScopeId,
    composer: Arc<dyn AnyCompose + 'a>,
    state_index: usize,
    states: Vec<DynState>,
    child_index: usize,
    children: Vec<Scope<'a>>,
}

impl Default for Scope<'_> {
    fn default() -> Self {
        Self {
            id: Default::default(),
            composer: Arc::new(()),
            state_index: Default::default(),
            states: Default::default(),
            child_index: Default::default(),
            children: Default::default(),
        }
    }
}

impl Scope<'_> {
    fn new(id: ScopeId, composer: Arc<dyn AnyCompose>) -> Self {
        Self {
            id,
            composer,
            state_index: 0,
            states: Vec::new(),
            child_index: 0,
            children: Vec::new(),
        }
    }

    pub fn use_state<T: Any + Send + Sync>(&mut self, initial_value: T) -> State<T> {
        if let Some(existing_state) = self.states.get(self.state_index) {
            self.state_index += 1;
            let message = "Found an existing state, but it doesn't match the specified type. Make sure you're not using hooks conditionally.";
            return existing_state
                .to_state::<T>()
                .unwrap_or_else(|| panic!("{}", message));
        }

        let value = Arc::new(initial_value);

        let state = DynState {
            // TODO: These two should be random and unique
            id: StateId(self.state_index),
            changed: StateChanged::Unchanged,
            scope_id: self.id,
            value: value.clone(),
        };

        self.states.push(state);
        self.state_index += 1;

        State {
            id: StateId(self.states.len() - 1),
            scope_id: self.id,
            value,
        }
    }

    pub fn set_state<T: Send + Sync + 'static>(&mut self, state: &State<T>, value: T) {
        let state_id = state.id;

        let state = self
            .states
            .iter_mut()
            .find(|s| s.id == state_id)
            .unwrap_or_else(|| panic!("State not found."));

        if !state.value.is::<T>() {
            panic!("State value type mismatch.");
        }

        state.value = Arc::new(value);
        state.changed = StateChanged::Queued;
    }
}

// ===
// State
// ===

#[derive(Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct StateId(usize);

enum StateChanged {
    Unchanged,
    Queued,
    Changed,
}

struct DynState {
    id: StateId,
    changed: StateChanged,
    scope_id: ScopeId,
    value: Arc<dyn Any + Send + Sync>,
}

impl DynState {
    fn to_state<T: Any + Send + Sync>(&self) -> Option<State<T>> {
        self.value.clone().downcast::<T>().ok().map(|value| State {
            id: self.id,
            scope_id: self.scope_id,
            value,
        })
    }
}

pub struct State<T> {
    id: StateId,
    scope_id: ScopeId,
    value: Arc<T>,
}

impl<T> Deref for State<T> {
    type Target = Arc<T>;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

// ===
// Compose
// ===

pub trait Compose: Send + Sync {
    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a;

    /// Whether the compose should stop rendering further nodes or not.
    fn ignore_children(&self) -> bool {
        false
    }
}

impl Compose for () {
    fn compose<'a>(&self, _: &mut Scope) -> impl Compose + 'a {}

    fn ignore_children(&self) -> bool {
        true
    }
}

// TODO: Implement for Option. Where the value is None, we run the clear function so that the scope gets deleted from
// the parent scope. Or maybe not, we might want to keep the scope around, to keep the same amount of children?

// TODO: Add impl for fn(&mut Scope) -> impl Compose if possible

// ===
// AnyCompose
// ===

trait AnyCompose: Send + Sync {
    fn recompose_scope(&self, scope: &mut Scope);
}

impl<C: Compose> AnyCompose for C {
    fn recompose_scope(&self, scope: &mut Scope) {
        scope.state_index = 0;
        scope.child_index = 0;

        for state in scope.states.iter_mut() {
            if matches!(state.changed, StateChanged::Queued) {
                state.changed = StateChanged::Changed;
            }
        }

        let child = self.compose(scope);

        for state in scope.states.iter_mut() {
            if matches!(state.changed, StateChanged::Changed) {
                state.changed = StateChanged::Unchanged;
            }
        }

        if self.ignore_children() {
            return;
        }

        // TODO: In order to support vector of children, we need to implement a way to know which child is which, to be
        // able to find its scope later, because we can't use child_index after all. If we eg. remove a child in the
        // middle of a vector, the indexes will be off.
        //
        // Maybe a trait `Key` that the composable needs to implement in order to be passed into the vec? And this trait
        // could be implemented for all types that implement PartialEq or Hash. The downside of this is that we could
        // not just repeat the same child again without having a custom id added to it.
        //
        // We could keep the Key trait and just make a wrapper around it though
        //
        // The mechanic for determining whether a child was removed or not (for cleanup) could be to go through the rest
        // of children in a vec that wasn't iterated over and run the cleanup function.
        if let Some(existing_child_scope) = scope.children.get_mut(scope.child_index) {
            let child_compose = existing_child_scope.composer.clone();
            child_compose.recompose_scope(existing_child_scope);

            scope.child_index += 1;
            return;
        };

        let child_scope_id = ScopeId(scope.id.0 + 1);
        let child_compose = Arc::new(child);
        let mut child_scope = Scope::new(child_scope_id, child_compose.clone());

        child_compose.recompose_scope(&mut child_scope);

        scope.children.push(child_scope);
    }
}

// ===
// Systems
// ===

fn initial_compose(mut roots: Query<&mut Root, Added<Root>>) {
    for mut root in roots.iter_mut() {
        // TODO: This should be random and unique
        let scope_id = ScopeId(0);
        let mut scope = Scope::new(scope_id, root.compose.clone());

        root.compose.recompose_scope(&mut scope);

        root.scope = Some(scope);
    }
}

fn recompose(mut roots: Query<&mut Root>) {
    for mut root in roots.iter_mut() {
        let Some(scope) = &mut root.scope else {
            continue;
        };

        let mut scopes = VecDeque::from([scope]);

        while let Some(scope) = scopes.pop_front() {
            if scope
                .states
                .iter()
                .any(|state| matches!(state.changed, StateChanged::Queued))
            {
                let composer = scope.composer.clone();

                composer.recompose_scope(scope);
            }

            scopes.extend(scope.children.iter_mut());
        }
    }
}

// ===
// Root
// ===

#[derive(Component)]
pub struct Root {
    compose: Arc<dyn AnyCompose>,
    scope: Option<Scope<'static>>,
}

impl Root {
    pub fn new<C: Compose + Clone + 'static>(composer: C) -> Self {
        Self {
            compose: Arc::new(composer),
            scope: None,
        }
    }
}
