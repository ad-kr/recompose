use crate::{
    modify::{Modifier, Modify},
    ChildOrder, Compose, Root, Scope, SetState,
};
use bevy_ecs::{
    bundle::Bundle,
    entity::Entity,
    system::{Commands, Query},
};
use bevy_hierarchy::{BuildChildren, DespawnRecursiveExt};
use std::{collections::HashMap, sync::Arc};

#[derive(Clone)]
pub struct Spawn<B: Bundle + Clone> {
    pub(crate) bundle_generator: Arc<dyn (Fn() -> B) + Send + Sync>,
    pub(crate) modifier: Modifier,
}

impl<B: Bundle + Clone> Spawn<B> {
    /// Creates a new spawn with the given bundle.
    pub fn new(bundle: B) -> Self {
        Self {
            bundle_generator: Arc::new(move || bundle.clone()),
            modifier: Modifier::default(),
        }
    }
}

impl<B: Bundle + Clone> Modify for Spawn<B> {
    fn modifier(&mut self) -> &mut Modifier {
        &mut self.modifier
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
        let observers = self.modifier.observer_adders.clone();

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

                ec.try_insert(bundle.clone());

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
                // Spawn-composables are rarely the direct children of their parents, which means that the child index
                // will likely not be correct. To find out what order the children should be in, we have to accumulate
                // the child indices of all the ancestors.
                let mut accumulated_child_index = index as f64;

                while let Some(scope) = parent_scope {
                    if let Some(entity) = scope.get_entity() {
                        ec.set_parent(entity)
                            .try_insert(ChildOrder(accumulated_child_index));
                        return;
                    }

                    // Each time we go "deeper" into the hierarchy, the child index is less significant, so we multiply
                    // it by 0.1. This is a bit of a hack, but it works. This will lose precision at some point.
                    accumulated_child_index = scope.index as f64 + (accumulated_child_index * 0.1);

                    parent_scope = scope
                        .get_parent()
                        .and_then(|scope_id| scopes.get(&scope_id));
                }
            },
        );

        self.modifier.children.clone()
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
