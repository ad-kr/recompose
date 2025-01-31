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
use std::sync::Arc;

// Storing observers directly would be better, but it's a little tricky, so for now we store a function that adds
// the observer given entity commands.
type ObserverGeneratorFn = Arc<dyn (Fn(&mut EntityCommands) -> Entity) + Send + Sync>;

#[derive(Clone)]
pub(crate) struct ObserverGenerator(ObserverGeneratorFn);

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
///           .to_compose()
///           .use_modifier(&self.modifier)
///    }
/// }
/// ```
#[derive(Clone, Default)]
pub struct Modifier {
    pub(crate) children: DynCompose,
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

    fn to_dyn(self) -> DynCompose
    where
        Self: 'static,
    {
        DynCompose::new(self)
    }

    fn keyed(self, key: usize) -> Keyed
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
            state.set(typed_state_id, true)
        })
        .observe_retained(move |_: Trigger<Pointer<Out>>, mut state: SetState| {
            state.set(typed_state_id, false)
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

    /// Converts this `Compose` into `DynCompose`.
    fn to_dyn(self) -> DynCompose
    where
        Self::Target: 'static;

    /// Wraps this `Compose` in a `Keyed` compose.
    fn keyed(self, key: usize) -> Keyed
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

    /// Binds the given `State<bool>` or `StateRef<bool>` to the hovered state of the entity.
    fn bind_hover(self, hover_state: impl GetStateId<bool>) -> Self::Target;
}
