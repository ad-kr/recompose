use std::sync::Arc;

use bevy_app::{App, Plugin, PreUpdate};
// use bevy_ecs::{
//     component::Component,
//     system::{Query, SystemState},
//     world::World,
// };
// use std::{
//     any::Any,
//     cell::UnsafeCell,
//     collections::HashMap,
//     ops::{Deref, DerefMut},
//     sync::Arc,
// };

pub struct IslandsUiPlugin;

impl Plugin for IslandsUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, compose);
    }
}

// ===
// Scope tree
// ===

// struct ScopeTree {
//     scopes: Vec<Scope>,
// }

// // ===
// // Context
// // ===

// #[derive(Default)]
// pub struct Context {
//     current_scope_id: ScopeId,
//     scopes: HashMap<ScopeId, Scope>,
// }

// ===
// Scope
// ===

#[derive(Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct ScopeId(usize);

// impl Deref for ScopeId {
//     type Target = usize;
//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }

// impl DerefMut for ScopeId {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.0
//     }
// }

// #[derive(Clone)]
// enum Instruction {
//     World(Arc<dyn Fn(&mut World) + Send + Sync>),
//     WorldOnce(Arc<dyn Fn(&mut World) + Send + Sync>),
//     State(Arc<dyn Any + Send + Sync>),
// }

// // impl PartialEq for Instruction {
// //     fn eq(&self, other: &Self) -> bool {
// //         match (self, other) {
// //             (Instruction::UseWorld(..), Instruction::UseWorld(..)) => true, // Callbacks are never updated.
// //             (Instruction::UseWorldOnce(..), Instruction::UseWorldOnce(..)) => true, // Callbacks are never updated.
// //             // (Instruction::UseWorld(a), Instruction::UseWorld(b)) => Arc::ptr_eq(a, b),
// //             // (Instruction::UseWorldOnce(a), Instruction::UseWorldOnce(b)) => Arc::ptr_eq(a, b),
// //             _ => false,
// //         }
// //     }
// // }

struct State {}

pub struct Scope {
    id: ScopeId,
    composer: Arc<dyn AnyCompose>,
    states: Vec<State>,
    children: Vec<Scope>,
}

// #[derive(Default, Clone)]
// pub struct Scope {
//     // TODO: add queued change bool field. Initially it is true, then when the instructions first ran, it is set to false. When a state is changed, it is set to true again.
//     // is_changed: bool,
//     current_index: usize,
//     parent: Option<ScopeId>,
//     instructions: Vec<Instruction>,
// }

// // TODO: This is weird. If we want to compare the parent and instructions, we should compare them directly, rather than specifying here that is_changed is not part of the comparison.
// // impl PartialEq for Scope {
// //     fn eq(&self, other: &Self) -> bool {
// //         self.parent == other.parent && self.instructions == other.instructions
// //     }
// // }

// // impl Default for Scope {
// //     fn default() -> Self {
// //         Self {
// //             // is_changed: true,
// //             parent: Default::default(),
// //             instructions: Default::default(),
// //         }
// //     }
// // }

// impl Scope {
//     fn with_parent(parent: Option<ScopeId>) -> Self {
//         Self {
//             current_index: 0,
//             parent,
//             instructions: Vec::new(),
//         }
//     }

//     pub fn use_world<F: Fn(&mut World) + Send + Sync + 'static>(&mut self, f: F) {
//         self.instructions.push(Instruction::World(Arc::from(f)));
//     }

//     pub fn use_world_once<F: Fn(&mut World) + Send + Sync + 'static>(&mut self, f: F) {
//         self.instructions.push(Instruction::WorldOnce(Arc::from(f)));
//     }

//     pub fn use_state<T: Send + Sync + 'static>(&mut self, state: T) {
//         let state = self.instructions.last();
//         // self.instructions.push(Instruction::State(Arc::from(state)));
//     }
// }

// ===
// Compose
// ===

pub trait Compose: Send + Sync {
    fn compose(&self, cx: &mut Scope) -> impl Compose;

    /// Whether the compose should stop rendering further nodes or not.
    fn is_terminating(&self) -> bool {
        false
    }
}

impl Compose for () {
    fn compose(&self, _: &mut Scope) -> impl Compose {}

    fn is_terminating(&self) -> bool {
        true
    }
}

// ===
// AnyCompose
// ===

trait AnyCompose: Send + Sync {
    fn render(&self, id: ScopeId) -> Scope;
}

// impl<C: Compose> AnyCompose for C {
//     // TODO: Rename
//     fn render(&self, cx: &mut Context, parent: Option<ScopeId>) {
//         let scope_id = cx.current_scope_id;
//         *cx.current_scope_id += 1;

//         // TODO: This could be done in a safer way probably.
//         let scope = UnsafeCell::from(
//             cx.scopes
//                 .get(&scope_id)
//                 .unwrap_or(&Scope::with_parent(parent))
//                 .clone(),
//         );
//         // let mut scope = Rc::from(RefCell::new(Scope::with_parent(parent)));

//         // Safety: There is no safety. We're hoping that the user doesn't mutate the scope beyond the compose function.
//         let child = self.compose(unsafe { &mut *scope.get() });

//         // Safety: We're taking the scope out of the UnsafeCell and replacing it with a new one.
//         let owned_scope = std::mem::take(unsafe { &mut *scope.get() });

//         cx.scopes.insert(scope_id, owned_scope);

//         if !self.is_terminating() {
//             child.render(cx, Some(scope_id));
//         }
//     }
// }

// // ===
// // Root
// // ===

// #[derive(Component)]
// pub struct Root {
//     context: Context,
//     composable: Arc<dyn AnyCompose>,
// }

// impl Root {
//     pub fn new<C: Compose + 'static>(compose: C) -> Self {
//         Self {
//             context: Context::default(),
//             composable: Arc::from(compose),
//         }
//     }
// }

// // ===
// // Systems
// // ===

fn compose() {
    // fn compose(world: &mut World) {
    // let mut roots_system_state = SystemState::<Query<&mut Root>>::new(world);
    // let mut roots = roots_system_state.get_mut(world);

    // for mut root in roots.iter_mut() {
    //     // let mut context = Context::default();
    //     let composable = root.composable.clone();

    //     root.context.current_scope_id = ScopeId::default();

    //     composable.render(&mut root.context, None);

    //     // for (scope_id, scope) in context.scopes.iter_mut() {
    //     //     let Some(old_scope) = root.context.scopes.get(scope_id) else {
    //     //         continue;
    //     //     };

    //     // scope.is_changed = &*scope != old_scope;
    //     // }

    //     // root.context = context;
    // }

    // let instructions = roots
    //     .iter()
    //     .flat_map(|r| r.context.scopes.values())
    //     .flat_map(|s| s.instructions.iter())
    //     .cloned()
    //     .collect::<Vec<_>>();

    // // TODO: In the future, we could limit the running of the instruction to the scopes that are marked with is_changed. With signals marking the scope as changed or something like that.
    // for instruction in instructions {
    //     match instruction {
    //         Instruction::World(f) => f(world),
    //         // TODO: Add a way to remove the instruction after it's been used.
    //         Instruction::WorldOnce(f) => f(world),
    //         Instruction::State(state) => (),
    //     }
    // }
}
