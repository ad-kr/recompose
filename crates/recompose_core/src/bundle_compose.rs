use crate::{dyn_compose::DynCompose, keyed::Keyed, spawn::Spawn, Compose};
use bevy_ecs::{
    bundle::Bundle,
    event::Event,
    system::{EntityCommands, IntoObserverSystem},
};
use std::sync::Arc;

pub trait BundleCompose
where
    Self: Sized,
{
    /// Converts this `Bundle` into a `Spawn`.
    fn to_compose(self) -> Spawn<impl Bundle + Clone>;

    /// Sets the children of the spawned entity.
    fn children(self, children: impl Compose + 'static) -> Spawn<impl Bundle + Clone> {
        let mut spawn = self.to_compose();
        spawn.children = DynCompose::new(children);
        spawn
    }

    /// Converts this `Compose` into `DynCompose`.
    fn to_dyn(self) -> DynCompose {
        DynCompose::new(self.to_compose())
    }

    /// Wraps this `Compose` in a `Keyed` compose.
    fn keyed(self, key: usize) -> Keyed {
        Keyed::new(key, self.to_compose())
    }

    /// Adds an observer to the spawned entity.
    fn observe<E: Event, B2: Bundle, M>(
        self,
        observer: impl IntoObserverSystem<E, B2, M> + Clone + Sync,
    ) -> Spawn<impl Bundle + Clone> {
        let f = Arc::new(move |entity: &mut EntityCommands| {
            entity.observe(observer.clone());
        });

        let mut spawn = self.to_compose();
        spawn.observer_adders.push(f);

        spawn
    }
}

impl<B: Bundle + Clone> BundleCompose for B {
    fn to_compose(self) -> Spawn<impl Bundle + Clone> {
        Spawn::new(self)
    }
}
