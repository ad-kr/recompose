use crate::{
    state::{Dependency, DynState, GetStateId, State, StateId, TypedStateId},
    unique_id, AnyCompose, StateChanged,
};
use bevy_ecs::{
    entity::Entity,
    system::{BoxedSystem, IntoSystem},
};
use std::{any::Any, collections::HashMap, sync::Arc};

#[derive(Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ScopeId(usize);

/// A scope can be thought of as a "sum" of all modifications done by the [`compose`](crate::Compose::compose) function
/// of the [`Compose`](crate::Compose) trait. It holds the state of the composable and its children scopes. It is the
/// "actual" node in the tree-structure of the composables.
pub struct Scope<'a> {
    pub(crate) id: ScopeId,

    /// Indicates which index in the parent's children vector this scope is.
    pub(crate) index: usize,

    /// For composables that spawn an entity, this is the field that holds the rerefence to the entity.
    pub(crate) entity: Option<Entity>,

    /// The parent scope of this scope. Root-scopes do not have parents.
    pub(crate) parent: Option<ScopeId>,

    /// Indicates if the scope will decompose on before the next recomposition.
    pub(crate) will_decompose: bool,

    /// A copy of the composer that created this scope. It is used to recompose this scope when one of the states was
    /// changed, but the parent scope was recomposed.
    pub(crate) composer: Arc<dyn AnyCompose + 'a>,

    /// The index counter of the states when running the `compose` function. It is used to keep track of the states.
    pub(crate) state_index: usize,

    /// The states of the composable.
    pub(crate) states: Vec<DynState>,

    /// The children of the composable.
    pub(crate) children: Vec<Scope<'a>>,

    /// The "collected" systems after the `compose`-function was executed. The systems are run and discarded after the
    /// recomposition.
    pub(crate) queued_systems: Vec<BoxedSystem<(), ()>>,
}

impl Default for Scope<'_> {
    fn default() -> Self {
        Self {
            id: ScopeId(unique_id()),
            index: Default::default(),
            entity: None,
            parent: None,
            will_decompose: false,
            composer: Arc::new(()),
            state_index: Default::default(),
            states: Default::default(),
            children: Default::default(),
            queued_systems: Default::default(),
        }
    }
}

impl Scope<'_> {
    pub(crate) fn new(composer: Arc<dyn AnyCompose>, parent: ScopeId, index: usize) -> Self {
        Self {
            id: ScopeId(unique_id()),
            index,
            entity: None,
            parent: Some(parent),
            will_decompose: false,
            composer: composer.clone(),
            state_index: 0,
            states: Vec::new(),
            children: Vec::new(),
            queued_systems: Vec::new(),
        }
    }

    pub(crate) fn as_root_scope(entity: Entity, composer: Arc<dyn AnyCompose>) -> Self {
        Self {
            id: ScopeId(unique_id()),
            index: 0,
            entity: Some(entity),
            parent: None,
            will_decompose: false,
            composer: composer.clone(),
            state_index: 0,
            states: Vec::new(),
            children: Vec::new(),
            queued_systems: Vec::new(),
        }
    }

    /// Creates a new state. States are persisted between each recomposition of the composable. Each time a state
    /// changes, the scope it belongs to is schedules for recomposition.
    pub fn use_state<T: Any + Send + Sync>(&mut self, initial_value: T) -> State<T> {
        if let Some(existing_state) = self.states.get(self.state_index) {
            self.state_index += 1;
            return existing_state.to_state::<T>();
        }

        let value = Arc::new(initial_value);

        let dyn_state = DynState {
            id: StateId::Generated(unique_id()),
            changed: StateChanged::Changed,
            value: value.clone(),
        };

        let state = dyn_state.to_state();

        self.states.push(dyn_state);
        self.state_index += 1;

        state
    }

    /// Creates a new state with a given id. It is useful for cases where you want to reference a state in an external
    /// system or composable.
    pub fn use_state_with_id<T: Any + Send + Sync>(
        &mut self,
        state_id: TypedStateId<T>,
        initial_value: T,
    ) -> State<T> {
        if let Some(existing_state) = self.states.iter().find(|s| s.id == state_id.get_id()) {
            self.state_index += 1;
            return existing_state.to_state::<T>();
        }

        let value = Arc::new(initial_value);

        let dyn_state = DynState {
            id: state_id.get_id(),
            changed: StateChanged::Changed,
            value: value.clone(),
        };

        let state = dyn_state.to_state();

        self.states.push(dyn_state);
        self.state_index += 1;

        state
    }

    /// Sets the value of the given state. The change happens immediately.
    pub fn set_state<T: Send + Sync + 'static>(&mut self, state: &State<T>, value: T) {
        let state = self
            .states
            .iter_mut()
            .find(|s| s.id == state.id)
            .unwrap_or_else(|| panic!("State not found."));

        if !state.value.is::<T>() {
            panic!("State value type mismatch.");
        }

        state.value = Arc::new(value);
        state.changed = StateChanged::Queued;
    }

    /// Sets the value of a state with a given id. The change happens immediately.
    pub fn set_state_with_id<T: Send + Sync + 'static>(
        &mut self,
        state_id: TypedStateId<T>,
        value: T,
    ) {
        let state = self
            .states
            .iter_mut()
            .find(|s| s.id == state_id.get_id())
            .unwrap_or_else(|| panic!("State not found."));

        if !state.value.is::<T>() {
            panic!("State value type mismatch.");
        }

        state.value = Arc::new(value);
        state.changed = StateChanged::Queued;
    }

    pub(crate) fn get_state_by_index<T: Any + Send + Sync>(&self, index: usize) -> State<T> {
        let dyn_state = self
            .states
            .get(index)
            .unwrap_or_else(|| panic!("State not found."));

        dyn_state.to_state()
    }

    /// A callback that is run only if the dependencies have changed.
    pub fn use_effect(&mut self, effect: impl Fn(), dependecies: impl Dependency) {
        if !dependecies.has_changed() {
            return;
        }

        effect();
    }

    /// Runs a callback when the component is first composed.
    pub fn use_mount(&mut self, callback: impl Fn()) {
        let once = self.use_state(());
        self.use_effect(callback, once);
    }

    /// Runs a system. The system is not cached and is "rebuilt" every time the composable recomposes. It is therefore
    /// not the most efficient way to to interact with the ECS world.
    pub fn use_system<M>(&mut self, system: impl IntoSystem<(), (), M>) {
        let sys: BoxedSystem<(), ()> = Box::from(IntoSystem::into_system(system));
        self.queued_systems.push(sys);
    }

    /// Runs a system when the composable is first composed.
    pub fn use_system_once<M>(&mut self, system: impl IntoSystem<(), (), M>) {
        let once = self.use_state(());

        if matches!(once.changed, StateChanged::Changed) {
            self.use_system(system);
        }
    }

    pub(crate) fn get_parent(&self) -> Option<ScopeId> {
        self.parent
    }

    pub(crate) fn set_entity(&mut self, entity: Entity) {
        self.entity = Some(entity);
    }

    pub(crate) fn get_entity(&self) -> Option<Entity> {
        self.entity
    }

    pub(crate) fn flatten_to_hashmap(&self) -> HashMap<ScopeId, &Scope> {
        let mut map = HashMap::new();

        map.insert(self.id, self);

        for child in self.children.iter() {
            map.extend(child.flatten_to_hashmap());
        }

        map
    }
}
