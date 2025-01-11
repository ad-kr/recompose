use crate::{
    bundle_compose::BundleCompose, dyn_compose::DynCompose, ChildOrder, Compose, Root, Scope,
    SetState,
};
use bevy_ecs::{
    bundle::Bundle,
    entity::Entity,
    system::{Commands, EntityCommands, Query},
};
use bevy_hierarchy::{BuildChildren, DespawnRecursiveExt};
use std::{collections::HashMap, sync::Arc};

#[derive(Clone)]
pub struct Spawn<B: Bundle + Clone> {
    pub(crate) bundle_generator: Arc<dyn (Fn() -> B) + Send + Sync>,
    pub(crate) children: DynCompose,
    // Storing observers directly would be better, but it's a little tricky, so for now we store a function that adds
    // the observer given entity commands.
    #[allow(clippy::type_complexity)]
    pub(crate) observer_adders: Vec<Arc<dyn Fn(&mut EntityCommands) + Send + Sync>>,
}

impl<B: Bundle + Clone> Spawn<B> {
    /// Creates a new spawn with the given bundle.
    pub fn new(bundle: B) -> Self {
        Self {
            bundle_generator: Arc::new(move || bundle.clone()),
            children: DynCompose::new(()),
            observer_adders: vec![],
        }
    }
}

impl<B: Bundle + Clone> BundleCompose for Spawn<B> {
    fn to_compose(self) -> Spawn<impl Bundle + Clone> {
        self
    }
}

impl<B: Bundle + Clone> Compose for Spawn<B> {
    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a {
        let entity = cx.use_state(None);
        let bundle = (self.bundle_generator)();
        let index = cx.index;

        if let Some(entity) = *entity {
            cx.set_entity(entity);
        };

        let parent = cx.get_parent();
        let observers = self.observer_adders.clone();

        cx.use_system(
            move |mut state: SetState, mut commands: Commands, roots: Query<&Root>| {
                let mut ec = match *entity {
                    Some(entity) => commands.entity(entity),
                    None => {
                        let mut ec = commands.spawn_empty();
                        observers.iter().for_each(|o| o(&mut ec));
                        state.set(&entity, Some(ec.id()));
                        ec
                    }
                };

                // TODO: If `ObservedBy` was public, we could run `ec.retain::<(Parent, ObservedBy)>();` here, which
                // would enable us to change bundle types between "recomposes". This would also require that we stored
                // Bundle information more dynamically, which might be impossible, since Bundle is not dyn-compatible.
                // How a about a bundle generator? `|| -> impl Bundle`?
                ec.try_insert((ChildOrder(index), bundle.clone()));

                let Some(parent_scope_id) = parent else {
                    return;
                };

                // TODO: This is probably not the best way to get the parent scope
                let scopes = roots
                    .iter()
                    .map_while(|root| root.scope.as_ref().map(|scope| scope.flatten_to_hashmap()))
                    .flatten()
                    .collect::<HashMap<_, _>>();

                let mut parent_scope = scopes.get(&parent_scope_id);

                while let Some(scope) = parent_scope {
                    if let Some(entity) = scope.get_entity() {
                        ec.set_parent(entity);
                        return;
                    }

                    parent_scope = scope
                        .get_parent()
                        .and_then(|scope_id| scopes.get(&scope_id));
                }
            },
        );

        self.children.clone()
    }

    fn decompose(&self, cx: &mut Scope) {
        let entity = cx.get_state_by_index::<Option<Entity>>(0);

        if let Some(entity) = *entity {
            cx.use_system_once(move |mut commands: Commands| {
                let Some(ec) = commands.get_entity(entity) else {
                    return;
                };
                ec.try_despawn_recursive();
            });
        }
    }
}
