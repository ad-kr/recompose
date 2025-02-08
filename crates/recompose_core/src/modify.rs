use crate::{
    dyn_compose::DynCompose,
    keyed::Keyed,
    state::{GetStateId, SetState, TypedStateId},
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
use std::{hash::Hash, sync::Arc};

// Storing observers directly would be better, but it's a little tricky, so for now we store a function that adds
// the observer given entity commands.
type ObserverGeneratorFn = Arc<dyn (Fn(&mut EntityCommands) -> Entity) + Send + Sync>;

#[derive(Clone)]
pub(crate) struct ObserverGenerator(ObserverGeneratorFn);

impl ObserverGenerator {
    fn new<E: Event, B: Bundle, M>(
        observer: impl IntoObserverSystem<E, B, M> + Clone + Sync,
    ) -> Self {
        let f = Arc::new(move |entity: &mut EntityCommands| {
            let target_entity = entity.id();
            let commands = entity.commands_mut();
            let o = Observer::new(observer.clone()).with_entity(target_entity);
            commands.spawn(o).id()
        });

        Self(f)
    }

    pub fn generate(&self, entity: &mut EntityCommands) -> Entity {
        self.0(entity)
    }
}

/// Modifiers hold information about children and observers. Used together with the [`Modify`](Modify) trait they enable
/// a compoosable to add ECS children and observers to the spawned entity.
///
/// We can forward the modifier to the underlying [`Spawn`](crate::spawn::Spawn) by using the [`use_modifier`](Modify::use_modifier)
/// function.
///
/// # Example
/// ```
/// fn compose(cx: &mut Scope) -> impl Compose {
///    Button {
///       label: "Click me".to_string(),
///       modifier: Modifier::default()
///    }
///    .observe(move |_: Trigger<Pointer<Click>>| {
///        println!("Button clicked!");
///    });
///
/// }
///
/// #[derive(Clone)]
/// struct Button {
///     label: String
///     modifier: Modifier,
/// }
///
/// impl Modify for Button {
///    fn modifier(&mut self) -> &mut Modifier {
///       &mut self.modifier
///   }
/// }
///
/// impl Compose for Button {
///    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a {
///       Text::new(self.label.clone())
///           .use_modifier(&self.modifier)
///    }
/// }
/// ```
#[derive(Clone, Default)]
#[allow(clippy::type_complexity)]
pub struct Modifier {
    pub(crate) children: DynCompose,
    pub(crate) bundle_modifiers: Vec<Arc<dyn Fn(&mut EntityCommands) + Send + Sync>>,
    pub(crate) temporary_observers: Vec<ObserverGenerator>,
    pub(crate) retained_observers: Vec<ObserverGenerator>,
}

impl Modifier {
    /// Joins two modifiers together. Note, the the newest children will override the old children.
    pub fn join(&mut self, other: &Modifier) {
        self.children = match other.children.is_empty() {
            true => self.children.clone(),
            false => other.children.clone(),
        };
        self.bundle_modifiers
            .extend(other.bundle_modifiers.iter().cloned());
        self.temporary_observers
            .extend(other.temporary_observers.iter().cloned());
        self.retained_observers
            .extend(other.retained_observers.iter().cloned());
    }
}

/// The `Modify` trait is used to modify entities before they are spawned or recomposed. It is used to add children and
/// observers to the entity. See the [`Modifier`](Modifier) struct for more information.
pub trait Modify: Sized {
    fn modifier(&mut self) -> &mut Modifier;
}

impl<T: Modify + Compose> ModifyFunctions<T> for T {
    type Target = T;

    fn use_modifier(mut self, modifier: &Modifier) -> Self {
        self.modifier().join(modifier);
        self
    }

    fn children(mut self, children: impl Compose + 'static) -> Self {
        let modifier = self.modifier();
        modifier.children = DynCompose::new(children);
        self
    }

    fn with_bundle<B: Bundle + Clone>(mut self, bundle: B) -> Self::Target {
        let bundle_modifier = Arc::new(move |entity: &mut EntityCommands| {
            entity.try_insert(bundle.clone());
        });

        let modifier = self.modifier();
        modifier.bundle_modifiers.push(bundle_modifier);

        self
    }

    fn with_bundle_if<B: Bundle + Clone>(mut self, condition: bool, bundle: B) -> Self::Target {
        let bundle_modifier = Arc::new(move |entity: &mut EntityCommands| {
            if condition {
                entity.try_insert(bundle.clone());
            } else {
                entity.remove::<B>();
            }
        });

        let modifier = self.modifier();
        modifier.bundle_modifiers.push(bundle_modifier);

        self
    }

    fn to_dyn(self) -> DynCompose
    where
        Self: 'static,
    {
        DynCompose::new(self)
    }

    fn some(self) -> Option<Self::Target> {
        Some(self)
    }

    fn keyed<H: Hash + Send + Sync>(self, key: H) -> Keyed<H>
    where
        Self: 'static,
    {
        Keyed::new(key, self)
    }

    fn observe<E: Event, B2: Bundle, M>(
        mut self,
        observer: impl IntoObserverSystem<E, B2, M> + Clone + Sync,
    ) -> Self {
        let observer_generator = ObserverGenerator::new(observer);
        let modifier = self.modifier();
        modifier.temporary_observers.push(observer_generator);

        self
    }

    fn observe_retained<E: Event, B2: Bundle, M>(
        mut self,
        observer: impl IntoObserverSystem<E, B2, M> + Clone + Sync,
    ) -> Self {
        let observer_generator = ObserverGenerator::new(observer);
        let modifier = self.modifier();
        modifier.retained_observers.push(observer_generator);

        self
    }

    fn bind_hover(self, hover_state: impl GetStateId<bool>) -> Self {
        let typed_state_id = TypedStateId::from_state_id(hover_state.get_id());

        self.observe_retained(move |_: Trigger<Pointer<Over>>, mut state: SetState| {
            state.set_neq(typed_state_id, true)
        })
        .observe_retained(move |_: Trigger<Pointer<Out>>, mut state: SetState| {
            state.set_neq(typed_state_id, false)
        })
    }
}

/// The `ModifyFunctions` trait provides a template for the functions of the [`Modify`](Modify) trait. The reason why
/// it is split from the actual `Modify` trait is that [`BundleExtension`](crate::bundle_extension::BundleExtension)
/// also implements it.
///
// The generic on the ModfiyFunctions trait doesn't do anything, and is here just not to have conflicting trait
// implementations for `BundleExtension` and `Modify`. Hacky but it works.
pub trait ModifyFunctions<T>: Sized {
    type Target: Compose;
    // Uses given modifier
    fn use_modifier(self, modifier: &Modifier) -> Self::Target;

    /// Sets the children of the spawned entity.
    fn children(self, children: impl Compose + 'static) -> Self::Target;

    // TODO: When `ObservedBy` is exposed, we should just retain it and SpawnComposable between each rerender and remove
    // all other components, so that we don't need to worry about removing conditional bundle components ourselves. This
    // will make this logic a lot simpler.

    /// Add a bundle to the spawned entity. This is useful when you want to extend the spawned bundle with additional
    /// components.
    ///
    /// For the [`Spawn`](crate::spawn::Spawn)-composable, the conditional bundles are always added before the main
    /// bundle, which means that the "main" bundle (of the same type) will always override the conditional bundles.
    fn with_bundle<B: Bundle + Clone>(self, bundle: B) -> Self::Target;

    /// Add a bundle to the spawned entity if the condition is true. When the condition is false, we're actively trying
    /// to remove the bundle each time the compsable recomposes.
    ///
    /// For the [`Spawn`](crate::spawn::Spawn)-composable, the conditional bundles are always added before the main
    /// bundle, which means that the "main" bundle (of the same type) will always override the conditional bundles.
    fn with_bundle_if<B: Bundle + Clone>(self, condition: bool, bundle: B) -> Self::Target;

    /// Converts this `Compose` into `DynCompose`.
    fn to_dyn(self) -> DynCompose
    where
        Self::Target: 'static;

    /// Wraps this `Compose` in `Some`.
    fn some(self) -> Option<Self::Target>;

    /// Wraps this `Compose` in `Some` if the condition is met, otherwise returns `None`.
    fn some_if(self, condition: bool) -> Option<Self::Target> {
        match condition {
            true => self.some(),
            false => None,
        }
    }

    /// Wraps this `Compose` in a `Keyed` compose.
    fn keyed<H: Hash + Send + Sync>(self, key: H) -> Keyed<H>
    where
        Self::Target: 'static;

    /// Adds an observer to the spawned entity. Observers are created and removed each time the composable recomposes.
    /// If you want to retain the observer, use the [`observe_retained`](Modify::observe_retained) function.
    fn observe<E: Event, B2: Bundle, M>(
        self,
        observer: impl IntoObserverSystem<E, B2, M> + Clone + Sync,
    ) -> Self::Target;

    /// Adds an observer to the spawned entity. Retained observers are only added once, when the entity is first spawned.
    fn observe_retained<E: Event, B2: Bundle, M>(
        self,
        observer: impl IntoObserverSystem<E, B2, M> + Clone + Sync,
    ) -> Self::Target;

    /// Binds the given state to the hovered state of the entity.
    fn bind_hover(self, hover_state: impl GetStateId<bool>) -> Self::Target;
}
