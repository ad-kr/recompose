use bevy_ecs::system::{ResMut, Resource, SystemParam};
use paste::paste;
use std::{any::Any, collections::HashMap, ops::Deref, sync::Arc};

type ArcAny = Arc<dyn Any + Send + Sync>;

pub(crate) enum StateSetterAction {
    Set(ArcAny),
    Modify(Box<dyn (Fn(ArcAny) -> ArcAny) + Send + Sync>),
}

#[derive(Resource, Default)]
pub(crate) struct StateSetter {
    pub(crate) queued: HashMap<StateId, StateSetterAction>,
}

#[derive(SystemParam)]
pub struct SetState<'w> {
    pub(crate) setter: ResMut<'w, StateSetter>,
}

impl SetState<'_> {
    pub fn set<T: Send + Sync + 'static>(&mut self, state: impl GetStateId<T>, value: T) {
        self.setter
            .queued
            .insert(state.get_id(), StateSetterAction::Set(Arc::new(value)));
    }

    pub fn modify<T: Send + Sync + 'static>(
        &mut self,
        state: impl GetStateId<T>,
        value_fn: impl (Fn(&T) -> T) + Send + Sync + 'static,
    ) {
        self.setter.queued.insert(
            state.get_id(),
            StateSetterAction::Modify(Box::new(move |input| {
                let input = input.downcast_ref::<T>().unwrap();

                Arc::new((value_fn)(input))
            })),
        );
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct StateId(pub(crate) usize);

#[derive(Clone, Copy)]
pub enum StateChanged {
    Unchanged,
    Queued,
    Changed,
}

pub(crate) struct DynState {
    pub(crate) id: StateId,
    pub(crate) changed: StateChanged,
    pub(crate) value: Arc<dyn Any + Send + Sync>,
}

impl DynState {
    pub(crate) fn to_state<T: Any + Send + Sync>(&self) -> State<T> {
        self.value
            .clone()
            .downcast::<T>()
            .map(|value| State {
                id: self.id,
                changed: self.changed,
                value,
            })
            .unwrap_or_else(|_| panic!("State value type mismatch."))
    }
}

#[derive(Clone)]
pub struct State<T> {
    pub(crate) id: StateId,
    pub(crate) changed: StateChanged,
    pub(crate) value: Arc<T>,
}

impl<T> Deref for State<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: Copy> State<T> {
    pub fn to_ref(&self) -> StateRef<T> {
        StateRef {
            id: self.id,
            value: *self.value,
        }
    }
}

#[derive(Clone, Copy)]
pub struct StateRef<T> {
    pub(crate) id: StateId,
    pub(crate) value: T,
}

impl<T> Deref for StateRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

pub trait GetStateId<T> {
    fn get_id(self) -> StateId;
}

impl<T> GetStateId<T> for State<T> {
    fn get_id(self) -> StateId {
        self.id
    }
}

impl<T> GetStateId<T> for &State<T> {
    fn get_id(self) -> StateId {
        self.id
    }
}

impl<T> GetStateId<T> for StateRef<T> {
    fn get_id(self) -> StateId {
        self.id
    }
}

/// A trait for getting the state changed status.
trait GetStateChanged {
    fn get_state_changed(&self) -> StateChanged;
}

impl<T> GetStateChanged for &State<T> {
    fn get_state_changed(&self) -> StateChanged {
        self.changed
    }
}
impl<T> GetStateChanged for State<T> {
    fn get_state_changed(&self) -> StateChanged {
        self.changed
    }
}

/// A trait for checking if dependencies have changed.
pub trait Dependency {
    fn has_changed(&self) -> bool;
}

macro_rules! impl_dependency {
    ($($d:expr),*) => {
        paste! {
            impl<$([<D$d>]: GetStateChanged),*> Dependency for ($([<D$d>]),*) {
                fn has_changed(&self) -> bool {
                    let ($([<d$d>]),*) = self;

                    $(matches!([<d$d>].get_state_changed(), StateChanged::Changed) ||)* false
                }
            }
        }

    };
}

impl<D: GetStateChanged> Dependency for D {
    fn has_changed(&self) -> bool {
        matches!(self.get_state_changed(), StateChanged::Changed)
    }
}

impl_dependency!(0, 1);
impl_dependency!(0, 1, 2);
impl_dependency!(0, 1, 2, 3);
impl_dependency!(0, 1, 2, 3, 4);
impl_dependency!(0, 1, 2, 3, 4, 5);
impl_dependency!(0, 1, 2, 3, 4, 5, 6);
impl_dependency!(0, 1, 2, 3, 4, 5, 6, 7);
impl_dependency!(0, 1, 2, 3, 4, 5, 6, 7, 8);
impl_dependency!(0, 1, 2, 3, 4, 5, 6, 7, 8, 9);
