use bevy_ecs::system::{ResMut, Resource, SystemParam};
use std::{any::Any, collections::HashMap, ops::Deref, sync::Arc};

type ArcAny = Arc<dyn Any + Send + Sync>;

pub(crate) enum StateSetterAction {
    Set(ArcAny),
    SetFn(Box<dyn (Fn(ArcAny) -> ArcAny) + Send + Sync>),
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
    pub fn set<T: Send + Sync + 'static>(&mut self, state: &State<T>, value: T) {
        self.setter
            .queued
            .insert(state.id, StateSetterAction::Set(Arc::new(value)));
    }

    pub fn set_fn<T: Send + Sync + 'static>(
        &mut self,
        state: &State<T>,
        value_fn: impl (Fn(&T) -> T) + Send + Sync + 'static,
    ) {
        self.setter.queued.insert(
            state.id,
            StateSetterAction::SetFn(Box::new(move |input| {
                let input = input.downcast_ref::<T>().unwrap();

                Arc::new((value_fn)(input))
            })),
        );
    }
}

// ===
// State
// ===

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

pub trait GetStateChanged {
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
