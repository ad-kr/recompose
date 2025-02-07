use bevy_app::{App, Plugin, PreUpdate};
use bevy_ecs::{
    component::{Component, ComponentHooks, ComponentId, StorageType},
    entity::Entity,
    query::{Added, Changed},
    schedule::IntoSystemConfigs,
    system::{Commands, Query, SystemState},
    world::{DeferredWorld, World},
};
use bevy_hierarchy::{BuildChildren, Parent};
use bevy_reflect::Reflect;
use dyn_compose::DynCompose;
use paste::paste;
use scope::{Scope, ScopeId};
use spawn::update_spawn_composables;
use state::{SetState, StateChanged, StateSetter, StateSetterAction};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

pub mod bundle_extension;
pub mod dyn_compose;
pub mod keyed;
pub mod modify;
pub mod scope;
pub mod spawn;
pub mod state;

pub struct RecomposePlugin;

impl Plugin for RecomposePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StateSetter>()
            .register_type::<ChildOrder>()
            .add_systems(
                PreUpdate,
                (
                    initial_compose,
                    run_queued_systems,
                    drop_decomposed_scopes,
                    set_states,
                    recompose,
                    update_spawn_composables,
                    order_children,
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

/// A trait that defines how a scope should be composed and decomposed. This trait is used to define the structure of a
/// scope. The `compose` function is called when the scope is composed or recomposed, and the `decompose` function is
/// called when the scope is decomposed.
pub trait Compose: Send + Sync {
    /// Compose the scope. This function is run when the composable is first initiated. It is then recomposed whenever
    /// it's parent scope is recomposed, or any of the states used inside the composable are changed.
    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a;

    /// Decomposes the scope. This function is run when the composable is removed from the parent scope.
    fn decompose(&self, cx: &mut Scope) {
        let _ = cx;
    }

    /// Whether the children returned by the `compose` function should be ignored. This is useful when the composable
    /// has custom logic for handling children. This is mostly used internally.
    fn ignore_children(&self) -> bool {
        false
    }

    /// Returns the name of the composable. This is mostly intended for debugging purposes.
    fn name(&self) -> String {
        String::from("AnonymousCompose")
    }
}

impl Compose for () {
    fn compose<'a>(&self, _: &mut Scope) -> impl Compose + 'a {}

    fn ignore_children(&self) -> bool {
        true
    }

    fn name(&self) -> String {
        String::from("EmptyCompose")
    }
}

impl<C: Compose + Clone + 'static> Compose for Option<C> {
    fn compose<'a>(&self, _: &mut Scope) -> impl Compose + 'a {
        match self {
            Some(inner) => DynCompose::new(inner.clone()),
            None => DynCompose::new(()),
        }
    }

    fn name(&self) -> String {
        match self {
            Some(_) => String::from("Some"),
            None => String::from("None"),
        }
    }
}

impl<C: Compose + 'static, F: (Fn(&mut Scope) -> C) + Send + Sync> Compose for F {
    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a {
        self(cx)
    }

    fn name(&self) -> String {
        String::from("ClosureCompose")
    }
}

impl<K: Compose + Key + Clone + 'static> Compose for Vec<K> {
    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a {
        let scope_ids = cx.use_state(HashMap::<usize, ScopeId>::new());

        let mut modified_scope_ids = (*scope_ids).clone();

        let keys = self.iter().map(|k| k.key()).collect::<Vec<_>>();

        let mut unique_keys = HashSet::new();
        let duplicate_key = keys.iter().find(|key| !unique_keys.insert(*key));

        if let Some(duplicate_key) = duplicate_key {
            panic!("Duplicate key found: {:?}", duplicate_key);
        }

        let parent_entity = match cx.entity {
            Some(entity) => entity,
            None => cx.parent_entity,
        };

        let parent_child_index = match cx.entity {
            Some(_) => ChildIndex::new(0),
            None => cx.child_index.clone(),
        };

        for (index, key_compose) in self.iter().enumerate() {
            let key = key_compose.key();
            let scope_id = scope_ids.get(&key);
            let scope =
                scope_id.and_then(|scope_id| cx.children.iter_mut().find(|s| s.id == *scope_id));

            if let Some(scope) = scope {
                scope.index = index;
                scope.composer = Arc::new(key_compose.clone());
                scope.parent_entity = parent_entity;
                let mut child_index = parent_child_index.clone();
                child_index.push(index);
                scope.child_index = child_index;
                scope.composer.clone().recompose_scope(scope);
                continue;
            }

            let compose = Arc::new(key_compose.clone());
            let mut scope = Scope::new(compose, index, parent_entity, parent_child_index.clone());
            key_compose.recompose_scope(&mut scope);

            modified_scope_ids.insert(key, scope.id);
            cx.children.push(scope);
        }

        for (key, scope_id) in modified_scope_ids.clone().iter() {
            if keys.contains(key) {
                continue;
            }

            modified_scope_ids.remove(key);

            let Some(scope) = cx.children.iter_mut().find(|scope| scope.id == *scope_id) else {
                continue;
            };

            scope.will_decompose = true;
        }

        cx.set_state_unchanged(&scope_ids, modified_scope_ids);
    }

    fn ignore_children(&self) -> bool {
        true
    }

    fn name(&self) -> String {
        String::from("VecCompose")
    }
}

macro_rules! impl_compose_for_tuple {
    ($($c:expr),*) => {
        paste! {
            impl<$([<C$c>]: Compose + Clone + 'static),*> Compose for ($([<C$c>]),*) {
                fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a {
                    let parent_entity = match cx.entity {
                        Some(entity) => entity,
                        None => cx.parent_entity,
                    };

                    let parent_child_index = match cx.entity {
                        Some(_) => ChildIndex::new(0),
                        None => cx.child_index.clone(),
                    };

                    $(
                        if let Some(existing_scope) = cx.children.get_mut($c) {
                            existing_scope.composer = Arc::new(self.$c.clone());
                            existing_scope.parent_entity = parent_entity;
                            let mut child_index = parent_child_index.clone();
                            child_index.push($c);
                            existing_scope.child_index = child_index;
                            existing_scope
                                .composer
                                .clone()
                                .recompose_scope(existing_scope);
                        } else {
                            let compose = Arc::new(self.$c.clone());
                            let mut scope = Scope::new(compose, $c, parent_entity, parent_child_index.clone());
                            self.$c.recompose_scope(&mut scope);
                            cx.children.push(scope);
                        }
                    )*
                }

                fn ignore_children(&self) -> bool {
                    true
                }

                fn name(&self) -> String {
                    String::from("TupleCompose")
                }
            }
        }
    };
}

impl_compose_for_tuple!(0, 1);
impl_compose_for_tuple!(0, 1, 2);
impl_compose_for_tuple!(0, 1, 2, 3);
impl_compose_for_tuple!(0, 1, 2, 3, 4);
impl_compose_for_tuple!(0, 1, 2, 3, 4, 5);
impl_compose_for_tuple!(0, 1, 2, 3, 4, 5, 6);
impl_compose_for_tuple!(0, 1, 2, 3, 4, 5, 6, 7);
impl_compose_for_tuple!(0, 1, 2, 3, 4, 5, 6, 7, 8);
impl_compose_for_tuple!(0, 1, 2, 3, 4, 5, 6, 7, 8, 9);

// ===
// Key
// ===

pub trait Key: Send + Sync {
    fn key(&self) -> usize;
}

/// A trait that (re)composes and decomposes a scope. It is used to act as a "wrapper" for the `Compose` trait, which
/// itself is not dyn-compatible. Since this trait is dyn-compatible, it can be stored in a `Box` or `Arc`.
pub trait AnyCompose: Send + Sync {
    /// This function is similar to the `compose` function on the `Compose` trait, but rather than returning the
    /// children, it sets the children directly to the passed scope (if having children is desirable). Doing it this
    /// way allows this trait to be dyn-compatible, which allows us to store it in a `Box` or `Arc`.
    fn recompose_scope(&self, scope: &mut Scope);

    /// This function decomposes the scope. Usually this calls the `decompose` function on the `Compose` trait directly.
    fn decompose_scope(&self, scope: &mut Scope);

    /// Returns the name of the composable. This is mostly intended for debugging purposes.
    fn get_name(&self) -> String;
}

impl<C: Compose> AnyCompose for C {
    // TODO: Make this take in the new compose value and index, since we basicall always need to set it anyways
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

        let parent_entity = match scope.entity {
            Some(entity) => entity,
            None => scope.parent_entity,
        };

        let parent_child_index = match scope.entity {
            Some(_) => ChildIndex::new(0),
            None => scope.child_index.clone(),
        };

        if let Some(child_scope) = scope.children.first_mut() {
            child_scope.composer = Arc::new(child);
            // TODO: Can we do this in a way that doesn't require us to remember to set these values?
            child_scope.parent_entity = parent_entity;
            let mut child_index = parent_child_index;
            child_index.push(0);
            child_scope.child_index = child_index;
            child_scope.composer.clone().recompose_scope(child_scope);
            return;
        };

        let child_compose = Arc::new(child);
        let mut child_scope =
            Scope::new(child_compose.clone(), 0, parent_entity, parent_child_index);

        child_compose.recompose_scope(&mut child_scope);

        scope.children.push(child_scope);
    }

    fn decompose_scope(&self, scope: &mut Scope) {
        self.decompose(scope);
    }

    fn get_name(&self) -> String {
        self.name()
    }
}

// ===
// Systems
// ===

fn initial_compose(mut roots: Query<(Entity, &mut Root), Added<Root>>) {
    for (entity, mut root) in roots.iter_mut() {
        let mut scope = Scope::as_root_scope(entity, root.compose.clone());

        root.compose.recompose_scope(&mut scope);

        root.scope = Some(scope);
    }
}

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

            for child in scope.children.iter_mut().rev() {
                scopes.push_front(child);
            }
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
            for child in scope.children.iter_mut().rev() {
                scopes.push_front(child);
            }
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
                let Some(state_setter_action) = setter.setter.queued.remove(&state.id) else {
                    continue;
                };

                let (value, should_change) = match state_setter_action {
                    StateSetterAction::Set(value, should_change) => (value, should_change),
                    StateSetterAction::Modify(f) => f(state.value.clone()),
                };

                state.value = value;

                if should_change {
                    state.changed = StateChanged::Queued;
                }
            }

            for child in scope.children.iter_mut().rev() {
                scopes.push_front(child);
            }
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

            for child in scope.children.iter_mut().rev() {
                scopes.push_front(child);
            }
        }
    }
}

#[derive(Component, Clone, PartialEq, PartialOrd, Reflect)]
pub(crate) struct ChildOrder(pub ChildIndex);

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Reflect)]
pub(crate) struct ChildIndex(pub Vec<usize>);

impl ChildIndex {
    pub fn new(index: usize) -> Self {
        Self(vec![index])
    }

    pub fn push(&mut self, index: usize) {
        self.0.push(index);
    }
}

fn order_children(
    mut commands: Commands,
    parents: Query<(Entity, &Parent, &ChildOrder)>,
    has_order_changed: Query<(), Changed<ChildOrder>>,
) {
    // TODO: Check if this helps with performance
    if has_order_changed.is_empty() {
        return;
    }

    let mut parent_children = HashMap::<Entity, Vec<(Entity, ChildOrder)>>::new();

    for (entity, parent, order) in parents.iter() {
        let parent_entity = parent.get();
        let entry = parent_children.get_mut(&parent_entity);

        if let Some(entry) = entry {
            entry.push((entity, order.clone()));
        } else {
            parent_children.insert(parent_entity, vec![(entity, order.clone())]);
        }
    }

    for (parent_entity, children) in parent_children.iter_mut() {
        // We never expect to have NaN values, so we can safely unwrap here
        children.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        for (entity, _) in children.iter() {
            let Some(mut ec) = commands.get_entity(*entity) else {
                continue;
            };

            ec.set_parent(*parent_entity);
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

            for child in scope.children.iter_mut().rev() {
                scopes.push_front(child);
            }
        }
    }
}

pub struct Root {
    compose: Arc<dyn AnyCompose>,
    scope: Option<Scope<'static>>,
}

impl Component for Root {
    const STORAGE_TYPE: StorageType = StorageType::Table;

    fn register_component_hooks(hooks: &mut ComponentHooks) {
        fn decompose_root(mut world: DeferredWorld, entity: Entity, _: ComponentId) {
            let Some(mut roots) = world.get_mut::<Root>(entity) else {
                return;
            };

            let Some(ref mut scope) = roots.scope else {
                return;
            };

            let mut scopes = VecDeque::from([scope]);

            while let Some(scope) = scopes.pop_front() {
                let composer = scope.composer.clone();
                composer.decompose_scope(scope);
                for child in scope.children.iter_mut().rev() {
                    scopes.push_front(child);
                }
            }
        }

        hooks.on_replace(decompose_root);
        hooks.on_remove(decompose_root);
    }
}

impl Root {
    pub fn new<C: Compose + 'static>(composer: C) -> Self {
        Self {
            compose: Arc::new(composer),
            scope: None,
        }
    }
}
