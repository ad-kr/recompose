use crate::{
    dyn_compose::DynCompose,
    keyed::Keyed,
    modify::{Modifier, ModifyFunctions},
    spawn::Spawn,
    state::GetStateId,
    Compose,
};
use bevy_ecs::{bundle::Bundle, event::Event, system::IntoObserverSystem};
use std::{hash::Hash, marker::PhantomData};

/// Trait that allows for easier conversion of `Bundle` into `Spawn`.
pub trait BundleExtension<B: Bundle + Clone>: Sized {
    /// Converts this `Bundle` into a `Spawn`.
    fn to_compose(self) -> Spawn<B>;
}

// The generic on the ModfiyFunctions trait doesn't do anything, and is here just not to have conflicting trait
// implementations for `BundleExtension` and `Modify`. Hacky but it works.
impl<B: Bundle + Clone, BE: BundleExtension<B>> ModifyFunctions<PhantomData<B>> for BE {
    type Target = Spawn<B>;

    fn children(self, children: impl Compose + 'static) -> Spawn<B> {
        self.to_compose().children(children)
    }

    fn with_bundle_if<B2: Bundle + Clone>(self, condition: bool, bundle: B2) -> Self::Target {
        self.to_compose().with_bundle_if(condition, bundle)
    }

    fn to_dyn(self) -> DynCompose {
        self.to_compose().to_dyn()
    }

    fn to_option(self) -> Option<Self::Target> {
        self.to_compose().to_option()
    }

    fn keyed<H: Hash + Send + Sync>(self, key: H) -> Keyed<H> {
        self.to_compose().keyed(key)
    }

    fn observe<E: Event, B2: Bundle, M>(
        self,
        observer: impl IntoObserverSystem<E, B2, M> + Clone + Sync,
    ) -> Spawn<B> {
        self.to_compose().observe(observer)
    }

    fn observe_retained<E: Event, B2: Bundle, M>(
        self,
        observer: impl IntoObserverSystem<E, B2, M> + Clone + Sync,
    ) -> Spawn<B> {
        self.to_compose().observe_retained(observer)
    }

    fn bind_hover(self, hover_state: impl GetStateId<bool>) -> Spawn<B> {
        self.to_compose().bind_hover(hover_state)
    }

    fn use_modifier(self, modifier: &Modifier) -> Self::Target {
        self.to_compose().use_modifier(modifier)
    }
}

impl<B: Bundle + Clone> BundleExtension<B> for B {
    fn to_compose(self) -> Spawn<B> {
        Spawn::new(self)
    }
}
