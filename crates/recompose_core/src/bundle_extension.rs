use crate::{
    dyn_compose::DynCompose, keyed::Keyed, modify::Modify, spawn::Spawn, state::GetStateId, Compose,
};
use bevy_ecs::{bundle::Bundle, event::Event, system::IntoObserverSystem};

// This is basically mirroring the `Modify` trait. It would be great to unify those two, but it's tricky since that
// would require a separate trait, that is implemented for both the `Modify` and `Bundle` trait, which is impossible.
pub trait BundleExtension: Sized {
    /// Converts this `Bundle` into a `Spawn`.
    fn to_compose(self) -> Spawn<impl Bundle + Clone>;

    /// Sets the children of the spawned entity.
    fn children(self, children: impl Compose + 'static) -> Spawn<impl Bundle + Clone> {
        self.to_compose().children(children)
    }

    /// Converts this `Compose` into `DynCompose`.
    fn to_dyn(self) -> DynCompose {
        self.to_compose().to_dyn()
    }

    /// Wraps this `Compose` in a `Keyed` compose.
    fn keyed(self, key: usize) -> Keyed {
        self.to_compose().keyed(key)
    }

    /// Adds an observer to the spawned entity. Observers are only added once, when the entity is first spawned.
    fn observe<E: Event, B2: Bundle, M>(
        self,
        observer: impl IntoObserverSystem<E, B2, M> + Copy + Sync,
    ) -> Spawn<impl Bundle + Clone> {
        self.to_compose().observe(observer)
    }

    /// Adds an observer to the spawned entity. Retained observers are only added once, when the entity is first spawned.
    fn observe_retained<E: Event, B2: Bundle, M>(
        self,
        observer: impl IntoObserverSystem<E, B2, M> + Copy + Sync,
    ) -> Spawn<impl Bundle + Clone> {
        self.to_compose().observe_retained(observer)
    }

    /// Binds the given `State<bool>` or `StateRef<bool>` to the hovered state of the entity.
    fn bind_hover(self, hover_state: impl GetStateId<bool>) -> Spawn<impl Bundle + Clone> {
        self.to_compose().bind_hover(hover_state)
    }
}

impl<B: Bundle + Clone> BundleExtension for B {
    fn to_compose(self) -> Spawn<impl Bundle + Clone> {
        Spawn::new(self)
    }
}
