use bevy_app::{App, Plugin, PreUpdate};
use bevy_ecs::{
    component::Component,
    query::Added,
    schedule::IntoSystemConfigs,
    system::{BoxedSystem, IntoSystem, Query, ResMut, Resource, SystemParam, SystemState},
    world::World,
};
use std::{
    any::Any,
    collections::{HashMap, VecDeque},
    ops::Deref,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

pub struct IslandsUiPlugin;

impl Plugin for IslandsUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StateSetter>().add_systems(
            PreUpdate,
            (initial_compose, run_queued_systems, set_states, recompose).chain(),
        );
    }
}

// ===
// UniqueId
// ===

static UNIQUE_ID: AtomicUsize = AtomicUsize::new(0);

fn unique_id() -> usize {
    UNIQUE_ID.fetch_add(1, Ordering::Relaxed)
}

// ===
// Scope
// ===

pub struct Scope<'a> {
    composer: Arc<dyn AnyCompose + 'a>,
    state_index: usize,
    states: Vec<DynState>,
    child_index: usize,
    children: Vec<Scope<'a>>,
    queued_systems: Vec<BoxedSystem<(), ()>>,
}

impl Default for Scope<'_> {
    fn default() -> Self {
        Self {
            composer: Arc::new(()),
            state_index: Default::default(),
            states: Default::default(),
            child_index: Default::default(),
            children: Default::default(),
            queued_systems: Default::default(),
        }
    }
}

impl Scope<'_> {
    fn new(composer: Arc<dyn AnyCompose>) -> Self {
        Self {
            composer,
            state_index: 0,
            states: Vec::new(),
            child_index: 0,
            children: Vec::new(),
            queued_systems: Vec::new(),
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

        let dyn_state = DynState {
            id: StateId(unique_id()),
            changed: StateChanged::Changed,
            value: value.clone(),
        };

        // Safety: We just created the state, so we know it's the correct type.
        let state = dyn_state.to_state().unwrap();

        self.states.push(dyn_state);
        self.state_index += 1;

        state
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

    // TODO: I don't think i can mix different State<T> and State<U> in the same array??? Check it
    pub fn use_effect<'a>(
        &'a mut self,
        effect: impl Fn(),
        dependecies: impl IntoIterator<Item = impl GetStateChanged + 'a>,
    ) {
        let any_changed = dependecies
            .into_iter()
            .any(|dep| matches!(dep.get_state_changed(), StateChanged::Changed));

        if !any_changed {
            return;
        }

        effect();
    }

    pub fn use_mount(&mut self, callback: impl Fn()) {
        let once = self.use_state(());
        self.use_effect(callback, [once]);
    }

    // TODO: Add description about that it is not cached
    pub fn use_system<M>(&mut self, system: impl IntoSystem<(), (), M>) {
        let sys: BoxedSystem<(), ()> = Box::from(IntoSystem::into_system(system));
        self.queued_systems.push(sys);
    }

    pub fn use_system_once<M>(&mut self, system: impl IntoSystem<(), (), M>) {
        let once = self.use_state(());

        if matches!(once.changed, StateChanged::Changed) {
            self.use_system(system);
        }
    }
}

// ===
// SetState
// ===

#[derive(Resource, Default)]
struct StateSetter {
    queued: HashMap<StateId, Arc<dyn Any + Send + Sync>>,
}

#[derive(SystemParam)]
pub struct SetState<'w> {
    setter: ResMut<'w, StateSetter>,
}

impl SetState<'_> {
    pub fn set<T: Send + Sync + 'static>(&mut self, state: &State<T>, value: T) {
        self.setter.queued.insert(state.id, Arc::new(value));
    }
}

// ===
// State
// ===

#[derive(Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct StateId(usize);

#[derive(Clone, Copy)]
pub enum StateChanged {
    Unchanged,
    Queued,
    Changed,
}

struct DynState {
    id: StateId,
    changed: StateChanged,
    value: Arc<dyn Any + Send + Sync>,
}

impl DynState {
    fn to_state<T: Any + Send + Sync>(&self) -> Option<State<T>> {
        self.value.clone().downcast::<T>().ok().map(|value| State {
            id: self.id,
            changed: self.changed,
            value,
        })
    }
}

#[derive(Clone)]
pub struct State<T> {
    id: StateId,
    changed: StateChanged,
    value: Arc<T>,
}

impl<T> Deref for State<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

pub trait GetStateChanged {
    fn get_state_changed(&self) -> StateChanged;
}

impl<T> GetStateChanged for &State<T> {
    fn get_state_changed(&self) -> StateChanged {
        self.changed
    }
}
impl<T> GetStateChanged for State<T> {
    fn get_state_changed(&self) -> StateChanged {
        self.changed
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

        let child_compose = Arc::new(child);
        let mut child_scope = Scope::new(child_compose.clone());

        child_compose.recompose_scope(&mut child_scope);

        scope.children.push(child_scope);
    }
}

// ===
// Systems
// ===

fn initial_compose(mut roots: Query<&mut Root, Added<Root>>) {
    for mut root in roots.iter_mut() {
        let mut scope = Scope::new(root.compose.clone());

        root.compose.recompose_scope(&mut scope);

        root.scope = Some(scope);
    }
}

// TODO: Could this be joined with recompose?
fn run_queued_systems(world: &mut World) {
    let mut roots_system_state = SystemState::<Query<&mut Root>>::new(world);
    let mut roots = roots_system_state.get_mut(world);

    let mut queued_systems = vec![];

    for mut root in roots.iter_mut() {
        let Some(scope) = &mut root.scope else {
            continue;
        };

        let mut scopes = VecDeque::from([scope]);

        while let Some(scope) = scopes.pop_front() {
            queued_systems.append(&mut scope.queued_systems);
            scopes.extend(scope.children.iter_mut());
        }
    }

    for mut system in queued_systems {
        system.initialize(world);
        system.run((), world);
        system.apply_deferred(world);
    }
}

fn set_states(mut setter: SetState, mut roots: Query<&mut Root>) {
    for mut root in roots.iter_mut() {
        let Some(scope) = &mut root.scope else {
            continue;
        };

        let mut scopes = VecDeque::from([scope]);

        while let Some(scope) = scopes.pop_front() {
            for state in scope.states.iter_mut() {
                let Some(new_value) = setter.setter.queued.remove(&state.id) else {
                    continue;
                };

                state.value = new_value;
                state.changed = StateChanged::Queued;
            }

            scopes.extend(scope.children.iter_mut());
        }
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
