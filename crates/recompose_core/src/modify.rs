use crate::{dyn_compose::DynCompose, keyed::Keyed, Compose};
use bevy_ecs::{
    bundle::Bundle,
    event::Event,
    system::{EntityCommands, IntoObserverSystem},
};
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct Modifier {
    pub(crate) children: Option<DynCompose>,
    // Storing observers directly would be better, but it's a little tricky, so for now we store a function that adds
    // the observer given entity commands.
    #[allow(clippy::type_complexity)]
    pub(crate) observer_adders: Vec<Arc<dyn Fn(&mut EntityCommands) + Send + Sync>>,
}

impl Modifier {
    /// Joins two modifiers together. Note, the the newest children will override the old children.
    pub fn join(&mut self, other: &Modifier) {
        self.children = other.children.as_ref().or(self.children.as_ref()).cloned();
        self.observer_adders
            .extend(other.observer_adders.iter().cloned());
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

    /// Adds an observer to the spawned entity. Observers are only added once, when the entity is first spawned.
    fn observe<E: Event, B2: Bundle, M>(
        mut self,
        observer: impl IntoObserverSystem<E, B2, M> + Clone + Sync,
    ) -> Self {
        let f = Arc::new(move |entity: &mut EntityCommands| {
            entity.observe(observer.clone());
        });

        let modifier = self.modifier();
        modifier.observer_adders.push(f);

        self
    }
}
