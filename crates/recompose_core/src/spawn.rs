use crate::{
    modify::{Modifier, Modify},
    scope::ScopeId,
    ChildOrder, Compose, Root, Scope, SetState,
};
use bevy_ecs::{
    bundle::Bundle,
    component::Component,
    entity::Entity,
    system::{Commands, Query},
};
use bevy_hierarchy::{BuildChildren, DespawnRecursiveExt};
use std::{collections::HashMap, sync::Arc};

/// A composable that takes in a bundle and spawns an entity with the bundle. When the composable is recomposed, the
/// bundle, children and observers are updated. When the composable is "decomposed", the entity is despawned from the
/// world.
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
        let should_update = cx.use_state(true);
        let bundle_updater = cx.use_state::<Box<
            dyn Fn(Entity, Entity, f64, &mut Commands, &mut SetState) + Send + Sync,
        >>(Box::new(
            |_: Entity, _: Entity, _: f64, _: &mut Commands, _: &mut SetState| {},
        ));
        let temporary_observers = cx.use_state(Vec::new());

        if let Some(entity) = *entity {
            cx.set_entity(entity);
        };

        let retained_observer_generators = self.modifier.retained_observers.clone();
        let scope_id = cx.id;

        cx.use_system_once(move |mut state: SetState, mut commands: Commands| {
            let mut ec = commands.spawn(SpawnComposable(scope_id));

            retained_observer_generators.iter().for_each(|gen| {
                gen.generate(&mut ec);
            });

            state.set(&entity, Some(ec.id()));
        });

        let generator = self.bundle_generator.clone();
        let temporary_observer_generators = self.modifier.temporary_observers.clone();
        let temporary_observer_entities = temporary_observers.clone();
        let conditional_bundles = self.modifier.conditional_bundles.clone();
        // In order to make the Spawn-composable more efficient, we're doing some trickery to avoid using `run_system`,
        // which proved itself to be very slow.
        //
        // We define the function that actualy updates the components of an entity as a state that we then read in a
        // separate system that runs right after the `recompose` system. This way we can avoid using non-cached systems,
        // as well as avoiding running certain things multiple times (like figuring out the scope parents for each
        // composable separately).
        cx.set_state_unchanged(&should_update, true);
        cx.set_state_unchanged(
            &bundle_updater,
            Box::new(
                move |entity: Entity,
                      parent_entity: Entity,
                      child_index: f64,
                      commands: &mut Commands,
                      state: &mut SetState| {
                    for observer_entity in temporary_observer_entities.iter() {
                        let Some(observer_ec) = commands.get_entity(*observer_entity) else {
                            continue;
                        };

                        observer_ec.try_despawn_recursive();
                    }

                    let bundle = generator();
                    let mut ec = commands.entity(entity);

                    for conditional_bundle in conditional_bundles.iter() {
                        conditional_bundle(&mut ec);
                    }

                    ec.try_insert((bundle, ChildOrder(child_index)))
                        .set_parent(parent_entity);

                    let observer_entities = temporary_observer_generators
                        .iter()
                        .map(|gen| gen.generate(&mut ec))
                        .collect::<Vec<_>>();

                    state.set_unchanged(&temporary_observers, observer_entities);
                },
            ),
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

    fn name(&self) -> String {
        String::from("Spawn")
    }
}

#[derive(Component, Debug)]
pub struct SpawnComposable(ScopeId);

pub fn update_spawn_composables(
    mut commands: Commands,
    mut state: SetState,
    mut roots: Query<&mut Root>,
    spawn_composables: Query<(Entity, &SpawnComposable)>,
) {
    // It would make more sense to iterate over `spawn_composables`, but it is easier to just itarate over the roots,
    // even though it iterates over scopes that are not Spawn-composables.
    for mut root in roots.iter_mut() {
        let Some(scope) = &mut root.scope else {
            continue;
        };

        let scope_entity = scope.entity.expect("Scope entity not found");
        let indices_and_parents =
            get_scope_indices_and_parent_entities(scope, vec![0], scope_entity);

        let mut scopes = Vec::from([scope]);

        while let Some(scope) = scopes.pop() {
            if let Some((entity, _)) = spawn_composables.iter().find(|(_, sc)| sc.0 == scope.id) {
                let should_update = scope.get_state_by_index::<bool>(1);

                if *should_update {
                    let (index, parent_entity) =
                        indices_and_parents.get(&scope.id).expect("Scope not found");

                    let bundle_updater = scope.get_state_by_index::<Box<
                        dyn Fn(Entity, Entity, f64, &mut Commands, &mut SetState) + Send + Sync,
                    >>(2);

                    bundle_updater(entity, *parent_entity, *index, &mut commands, &mut state);

                    scope.set_state_unchanged(&should_update, false);
                }
            }

            scopes.extend(scope.children.iter_mut());
        }
    }
}

fn get_scope_indices_and_parent_entities(
    scope: &Scope,
    mut ancestor_indices: Vec<usize>,
    parent_entity: Entity,
) -> HashMap<ScopeId, (f64, Entity)> {
    let mut indices = HashMap::new();

    ancestor_indices.push(scope.index);

    let index = ancestor_indices
        .iter()
        .enumerate()
        .map(|(i, item)| *item as f64 * 10f64.powi(-(i as i32 + 1)))
        .sum::<f64>();

    indices.insert(scope.id, (index, parent_entity));

    let ancestor_indices = match scope.entity {
        Some(_) => vec![0],
        None => ancestor_indices,
    };
    let passed_entity = match scope.entity {
        Some(e) => e,
        None => parent_entity,
    };

    for child in scope.children.iter() {
        let child_indices =
            get_scope_indices_and_parent_entities(child, ancestor_indices.clone(), passed_entity);
        indices.extend(child_indices);
    }

    indices
}
