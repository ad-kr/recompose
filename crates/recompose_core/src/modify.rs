use crate::{
    dyn_compose::DynCompose,
    keyed::Keyed,
    state::{GetStateId, SetState, StateRef},
    Compose,
};
use bevy_ecs::{
    bundle::Bundle,
    entity::Entity,
    event::Event,
    observer::{Observer, Trigger},
    system::{EntityCommands, IntoObserverSystem},
};
use bevy_picking::events::{Out, Over, Pointer};
use std::sync::Arc;

// Storing observers directly would be better, but it's a little tricky, so for now we store a function that adds
// the observer given entity commands.
type ObserverGeneratorFn = Arc<dyn (Fn(&mut EntityCommands) -> Entity) + Send + Sync>;

#[derive(Clone)]
pub(crate) enum ObserverGenerator {
    Temporary(ObserverGeneratorFn),
    Retained(ObserverGeneratorFn),
}

impl ObserverGenerator {
    fn new<E: Event, B2: Bundle, M>(
        observer: impl IntoObserverSystem<E, B2, M> + Clone + Sync,
    ) -> Self {
        let f = Arc::new(move |entity: &mut EntityCommands| {
            let target_entity = entity.id();
            let commands = entity.commands_mut();
            let o = Observer::new(observer.clone()).with_entity(target_entity);
            commands.spawn(o).id()
        });

        Self::Temporary(f)
    }

    fn retained<E: Event, B2: Bundle, M>(
        observer: impl IntoObserverSystem<E, B2, M> + Clone + Sync,
    ) -> Self {
        let f = Arc::new(move |entity: &mut EntityCommands| {
            let target_entity = entity.id();
            let commands = entity.commands_mut();
            let o = Observer::new(observer.clone()).with_entity(target_entity);
            commands.spawn(o).id()
        });

        Self::Retained(f)
    }

    pub fn is_retained(&self) -> bool {
        matches!(self, Self::Retained(_))
    }

    pub fn generate(&self, entity: &mut EntityCommands) -> Entity {
        match self {
            Self::Temporary(f) => f(entity),
            Self::Retained(f) => f(entity),
        }
    }
}

#[derive(Clone, Default)]
pub struct Modifier {
    pub(crate) children: Option<DynCompose>,
    pub(crate) observer_generators: Vec<ObserverGenerator>,
}

impl Modifier {
    /// Joins two modifiers together. Note, the the newest children will override the old children.
    pub fn join(&mut self, other: &Modifier) {
        self.children = other.children.as_ref().or(self.children.as_ref()).cloned();
        self.observer_generators
            .extend(other.observer_generators.iter().cloned());
    }
}

pub trait Modify: Sized {
    fn modifier(&mut self) -> &mut Modifier;

    // Uses given modifier
    fn use_modifier(mut self, modifier: &Modifier) -> Self {
        self.modifier().join(modifier);
        self
    }

    /// Sets the children of the spawned entity.
    fn children(mut self, children: impl Compose + 'static) -> Self {
        let modifier = self.modifier();
        modifier.children = Some(DynCompose::new(children));
        self
    }

    /// Converts this `Compose` into `DynCompose`.
    fn to_dyn(self) -> DynCompose
    where
        Self: Compose + 'static,
    {
        DynCompose::new(self)
    }

    /// Wraps this `Compose` in a `Keyed` compose.
    fn keyed(self, key: usize) -> Keyed
    where
        Self: Compose + 'static,
    {
        Keyed::new(key, self)
    }

    /// Adds an observer to the spawned entity. Observers are created and removed each time the composable recomposes.
    /// If you want to retain the observer, use the [`observe_retained`](Modify::observe_retained) function.
    fn observe<E: Event, B2: Bundle, M>(
        mut self,
        observer: impl IntoObserverSystem<E, B2, M> + Clone + Sync,
    ) -> Self {
        let observer_generator = ObserverGenerator::new(observer);
        let modifier = self.modifier();
        modifier.observer_generators.push(observer_generator);

        self
    }

    /// Adds an observer to the spawned entity. Retained observers are only added once, when the entity is first spawned.
    fn observe_retained<E: Event, B2: Bundle, M>(
        mut self,
        observer: impl IntoObserverSystem<E, B2, M> + Clone + Sync,
    ) -> Self {
        let observer_generator = ObserverGenerator::retained(observer);
        let modifier = self.modifier();
        modifier.observer_generators.push(observer_generator);

        self
    }

    /// Binds the given `State<bool>` or `StateRef<bool>` to the hovered state of the entity.
    fn bind_hover(self, hover_state: impl GetStateId<bool>) -> Self {
        // Workaround for the sending the state to the observer without cloning or copying it.
        let state_ref = StateRef {
            id: hover_state.get_id(),
            value: false,
        };

        self.observe_retained(move |_: Trigger<Pointer<Over>>, mut state: SetState| {
            state.set(state_ref, true)
        })
        .observe_retained(move |_: Trigger<Pointer<Out>>, mut state: SetState| {
            state.set(state_ref, false)
        })
    }
}
