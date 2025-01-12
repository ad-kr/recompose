use crate::{
    state::{Dependency, DynState, State, StateId},
    unique_id, AnyCompose, StateChanged,
};
use bevy_ecs::{
    entity::Entity,
    system::{BoxedSystem, IntoSystem},
};
use std::{any::Any, collections::HashMap, sync::Arc};

#[derive(Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ScopeId(usize);

pub struct Scope<'a> {
    pub(crate) id: ScopeId,

    /// Indicates which index in the parent's children vector this scope is.
    pub(crate) index: usize,
    pub(crate) entity: Option<Entity>,
    pub(crate) parent: Option<ScopeId>,
    pub(crate) will_decompose: bool,
    pub(crate) composer: Arc<dyn AnyCompose + 'a>,
    pub(crate) state_index: usize,
    pub(crate) states: Vec<DynState>,
    pub(crate) children: Vec<Scope<'a>>,
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

    pub fn use_state<T: Any + Send + Sync>(&mut self, initial_value: T) -> State<T> {
        if let Some(existing_state) = self.states.get(self.state_index) {
            self.state_index += 1;
            return existing_state.to_state::<T>();
        }

        let value = Arc::new(initial_value);

        let dyn_state = DynState {
            id: StateId(unique_id()),
            changed: StateChanged::Changed,
            value: value.clone(),
        };

        let state = dyn_state.to_state();

        self.states.push(dyn_state);
        self.state_index += 1;

        state
    }

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

    pub(crate) fn get_state_by_index<T: Any + Send + Sync>(&self, index: usize) -> State<T> {
        let dyn_state = self
            .states
            .get(index)
            .unwrap_or_else(|| panic!("State not found."));

        dyn_state.to_state()
    }

    pub fn use_effect(&mut self, effect: impl Fn(), dependecies: impl Dependency) {
        if !dependecies.has_changed() {
            return;
        }

        effect();
    }

    pub fn use_mount(&mut self, callback: impl Fn()) {
        let once = self.use_state(());
        self.use_effect(callback, once);
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

    pub fn get_parent(&self) -> Option<ScopeId> {
        self.parent
    }

    pub fn set_entity(&mut self, entity: Entity) {
        self.entity = Some(entity);
    }

    pub(crate) fn get_entity(&self) -> Option<Entity> {
        self.entity
    }

    // TODO: Make this private again when taking Spawn back into the crate
    pub(crate) fn flatten_to_hashmap(&self) -> HashMap<ScopeId, &Scope> {
        let mut map = HashMap::new();

        map.insert(self.id, self);

        for child in self.children.iter() {
            map.extend(child.flatten_to_hashmap());
        }

        map
    }
}
