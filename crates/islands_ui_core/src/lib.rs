use bevy_app::{App, Plugin, PreUpdate};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    query::Added,
    schedule::IntoSystemConfigs,
    system::{BoxedSystem, IntoSystem, Query, ResMut, Resource, SystemParam, SystemState},
    world::World,
};
use std::{
    any::{Any, TypeId},
    collections::{HashMap, VecDeque},
    hash::Hash,
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
            (
                initial_compose,
                run_queued_systems,
                drop_decomposed_scopes,
                set_states,
                recompose,
                decompose,
            )
                .chain(),
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

#[derive(Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct ScopeId(usize);

pub struct Scope<'a> {
    id: ScopeId,
    entity: Option<Entity>,
    parent: Entity,
    will_decompose: bool,
    composer: Arc<dyn AnyCompose + 'a>,
    state_index: usize,
    states: Vec<DynState>,
    children: Vec<Scope<'a>>,
    queued_systems: Vec<BoxedSystem<(), ()>>,
}

impl Default for Scope<'_> {
    fn default() -> Self {
        Self {
            id: ScopeId(unique_id()),
            entity: None,
            parent: Entity::PLACEHOLDER,
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
    fn new(composer: Arc<dyn AnyCompose>, parent: Entity) -> Self {
        Self {
            id: ScopeId(unique_id()),
            entity: None,
            parent,
            will_decompose: false,
            composer: composer.clone(),
            state_index: 0,
            states: Vec::new(),
            children: Vec::new(),
            queued_systems: Vec::new(),
        }
    }

    fn with_entity(entity: Entity, composer: Arc<dyn AnyCompose>, parent: Entity) -> Self {
        Self {
            id: ScopeId(unique_id()),
            entity: Some(entity),
            parent,
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

    pub fn get_state_by_index<T: Any + Send + Sync>(&self, index: usize) -> State<T> {
        let dyn_state = self
            .states
            .get(index)
            .unwrap_or_else(|| panic!("State not found."));

        dyn_state.to_state()
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

    pub fn get_parent(&self) -> Entity {
        self.parent
    }

    pub fn set_entity(&mut self, entity: Entity) {
        self.entity = Some(entity);
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
    fn to_state<T: Any + Send + Sync>(&self) -> State<T> {
        self.value
            .clone()
            .downcast::<T>()
            .map(|value| State {
                id: self.id,
                changed: self.changed,
                value,
            })
            .unwrap_or_else(|_| panic!("State value type mismatch."))
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

    fn decompose(&self, cx: &mut Scope) {
        let _ = cx;
    }

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

impl<C: Compose + Clone + 'static> Compose for Option<C> {
    fn compose<'a>(&self, _: &mut Scope) -> impl Compose + 'a {
        match self {
            Some(inner) => DynCompose::new(inner.clone()),
            None => DynCompose::new(()),
        }
    }
}

// ===
// DynCompose
// ===

pub struct DynCompose {
    type_id: TypeId,
    compose: Arc<dyn AnyCompose>,
}

impl DynCompose {
    pub fn new(compose: impl Compose + 'static) -> Self {
        Self {
            type_id: compose.type_id(),
            compose: Arc::new(compose),
        }
    }
}

impl Compose for DynCompose {
    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a {
        let type_id = cx.use_state(self.type_id);
        if let Some(existing_scope) = cx.children.first_mut() {
            if *type_id != self.type_id {
                existing_scope.will_decompose = true;

                // TODO: Can this be rewritten?
                let mut scope = Scope::new(self.compose.clone(), cx.entity.unwrap_or(cx.parent));
                self.compose.recompose_scope(&mut scope);
                cx.children.push(scope);
                cx.set_state(&type_id, self.type_id);
                return;
            }

            existing_scope.composer = self.compose.clone();
            existing_scope
                .composer
                .clone()
                .recompose_scope(existing_scope);
            return;
        }

        let mut scope = Scope::new(self.compose.clone(), cx.entity.unwrap_or(cx.parent));

        self.compose.recompose_scope(&mut scope);

        cx.children.push(scope);
        cx.set_state(&type_id, self.type_id);
    }

    fn ignore_children(&self) -> bool {
        true
    }
}

impl<K: Compose + Key + Clone + 'static> Compose for Vec<K> {
    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a {
        let scope_ids = cx.use_state(HashMap::<usize, ScopeId>::new());

        // TODO: Add check for when key values are duplicated
        for key_compose in self.iter() {
            let key = key_compose.key();
            let scope_id = scope_ids.get(&key);

            if let Some(scope_id) = scope_id {
                let scope = cx
                    .children
                    .iter_mut()
                    .find(|s| s.id == *scope_id)
                    .unwrap_or_else(|| {
                        panic!(
                            "Scope with id {:?} was expected to be found, but was not.",
                            scope_id
                        )
                    });
                scope.composer = Arc::new(key_compose.clone());
                scope.composer.clone().recompose_scope(scope);
                continue;
            }

            let compose = Arc::new(key_compose.clone());
            let mut scope = Scope::new(compose, cx.entity.unwrap_or(cx.parent));
            let scope_id = scope.id;
            scope.composer.clone().recompose_scope(&mut scope);

            cx.children.push(scope);

            // TODO: Since scope ids inner value is an Arc, the state we reference here should be updated, but I'm not sure.
            let mut new_scope_ids = (*scope_ids).clone();
            new_scope_ids.insert(key, scope_id);

            cx.set_state(&scope_ids, new_scope_ids);
        }

        let keys = self.iter().map(|k| k.key()).collect::<Vec<_>>();

        // TODO: Is there a better way to order the scopes? Or just ensure that they are always in the right order?
        cx.children.sort_by_key(|scope| {
            let scope_id = scope.id;
            let Some((key, _)) = scope_ids.iter().find(|(_, &id)| id == scope_id) else {
                return usize::MAX;
            };

            keys.iter().position(|k| k == key).unwrap_or(usize::MAX)
        });

        for (key, scope_id) in scope_ids.iter() {
            if keys.contains(key) {
                continue;
            }

            let scope = cx
                .children
                .iter_mut()
                .find(|s| s.id == *scope_id)
                .unwrap_or_else(|| {
                    panic!(
                        "Scope with id {:?} was expected to be found, but was not.",
                        scope_id
                    )
                });

            scope.will_decompose = true;

            let mut new_scope_ids = (*scope_ids).clone();
            new_scope_ids.remove(key);

            cx.set_state(&scope_ids, new_scope_ids);
        }

        // If scope exists in hashmap, but does not have a key in the vec, mark it for decompose and remove from hashmap
    }

    fn ignore_children(&self) -> bool {
        true
    }
}

// ===
// Key
// ===

pub trait Key: Send + Sync {
    fn key(&self) -> usize;
}

// ===
// AnyCompose
// ===

trait AnyCompose: Send + Sync {
    fn recompose_scope(&self, scope: &mut Scope);

    fn decompose_scope(&self, scope: &mut Scope);
}

impl<C: Compose> AnyCompose for C {
    fn recompose_scope(&self, scope: &mut Scope) {
        scope.state_index = 0;

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

        if let Some(child_scope) = scope.children.first_mut() {
            child_scope.composer = Arc::new(child);
            child_scope.composer.clone().recompose_scope(child_scope);
            return;
        };

        let child_compose = Arc::new(child);
        let mut child_scope =
            Scope::new(child_compose.clone(), scope.entity.unwrap_or(scope.parent));

        child_compose.recompose_scope(&mut child_scope);

        scope.children.push(child_scope);
    }

    fn decompose_scope(&self, scope: &mut Scope) {
        self.decompose(scope);
    }
}

// ===
// Systems
// ===

fn initial_compose(mut roots: Query<(Entity, &mut Root), Added<Root>>) {
    for (entity, mut root) in roots.iter_mut() {
        let mut scope = Scope::with_entity(entity, root.compose.clone(), entity);

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

fn drop_decomposed_scopes(mut roots: Query<&mut Root>) {
    for mut root in roots.iter_mut() {
        let Some(scope) = &mut root.scope else {
            continue;
        };

        let mut scopes = VecDeque::from([scope]);

        while let Some(scope) = scopes.pop_front() {
            scope.children.retain(|child| !child.will_decompose);
            scopes.extend(scope.children.iter_mut());
        }
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
                && !scope.will_decompose
            {
                let composer = scope.composer.clone();

                composer.recompose_scope(scope);
                continue;
            }

            scopes.extend(scope.children.iter_mut());
        }
    }
}

fn decompose(mut roots: Query<&mut Root>) {
    for mut root in roots.iter_mut() {
        let Some(scope) = &mut root.scope else {
            continue;
        };

        let mut scopes = VecDeque::from([scope]);

        while let Some(scope) = scopes.pop_front() {
            if scope.will_decompose {
                let composer = scope.composer.clone();
                composer.decompose_scope(scope);

                for child in scope.children.iter_mut() {
                    child.will_decompose = true;
                }
            }

            scopes.extend(scope.children.iter_mut());
        }
    }
}

// TODO: Mark main scope as will_decompose when the root is removed

// ===
// Root
// ===

#[derive(Component)]
pub struct Root {
    compose: Arc<dyn AnyCompose>,
    scope: Option<Scope<'static>>,
}

impl Root {
    pub fn new<C: Compose + 'static>(composer: C) -> Self {
        Self {
            compose: Arc::new(composer),
            scope: None,
        }
    }
}
