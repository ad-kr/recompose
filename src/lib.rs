//! # recompose
//!
//! `recompose` is a library for [Bevy](https://docs.rs/bevy/) that provides a declarative way to compose structures in
//! a way that is easy to write, and is ECS- and borrow-checker friendly.
//!
//! It is most useful for UI-building, but can be applied to other ECS-structures as well. For more information, check
//! out the [examples](https://github.com/ad-kr/recompose/tree/main/examples) and docs.
//!
//! # Example
//! ```
//! use bevy::prelude::*;
//! use recompose::prelude::*;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(RecomposePlugin)
//!         .add_systems(Startup, setup)
//!         .run();
//! }
//!
//! fn setup(mut commands: Commands) {
//!     commands.spawn(Camera2d);
//!     commands.spawn((Root::new(squares), Node::default()));
//! }
//!
//! // `Fn(&mut Scope) -> impl Compose` implements Compose, so we can use functions for simple composables.
//! fn squares(cx: &mut Scope) -> impl Compose {
//!     let count = cx.use_state(42);
//!
//!     Node {
//!         display: Display::Flex,
//!         column_gap: Val::Px(8.0),
//!         ..default()
//!     }
//!     .children((
//!         Square(Srgba::RED.into()),
//!         Square(Srgba::GREEN.into()),
//!         Square(Srgba::BLUE.into()),
//!         Text::new(count.to_string()).to_compose(),
//!     ))
//! }
//!
//! #[derive(Clone)]
//! struct Square(Color);
//!
//! // For more complex composables with input, we can implement Compose directly on a struct.
//! impl Compose for Square {
//!     fn compose<'a>(&self, _: &mut Scope) -> impl Compose + 'a {
//!         (
//!             Node {
//!                 width: Val::Px(32.0),
//!                 height: Val::Px(32.0),
//!                 ..default()
//!             },
//!             BackgroundColor(self.0),
//!         )
//!             .to_compose()
//!     }
//! }
//! ```
//! # Hooks and composing
//! If you've ever used React or similar libraries, you might be familiar with the concept of "hooks". In `recompose`,
//! "hooks" are functions that interact and/or modify the `Scope`. Read the [How it works](#how-it-works) section for
//! more details.
//!
//! Unlike React, not all hooks are required to follow the "rules of hooks" - being called in the same order, and never
//! conditionally, in a loop and so on. Functions that must obey these rules are prefixed wiht `use_`.
//!
//! Some of the hooks available:
//!
//! **State**
//! ```
//! let count = cx.use_state(42); // Get a state that persists between recompositions.
//! cx.set_state(&count, *count + 1); // Set the state to a new value.
//! ```
//!
//! **Composable lifetime**
//! ```
//! cx.use_mount(|| { /* Do something */}); // Called when the composable is first composed.
//! cx.effect(|| { /* Do something */}, (&count, &name)); // Called only when dependencies have changed.
//! ```
//!
//! **ECS World interaction**
//! ```
//! // Run a system each time the composable is recomposed.
//! cx.run_system(|names: Query<&Name>, mut state: SetState| {
//!    state.set(&count, 30); // Set the state to a new value.
//!    state.modify(&count, |count| *count + 1); // Modify the state.
//! });
//!
//! // Run a system once, when the composable is first composed.
//! cx.use_system_once(|| { /* .. */ });
//! ```
//!
//! # How it works
//!
//! `recompose` is built around the concept of composables that implement the [`Compose`](prelude::Compose) trait. The
//! [`compose`](prelude::Compose::compose) function of the `Compose` trait modifes the given [`Scope`](prelude::Scope)
//! (a `Scope` can be thought of as a node in a tree-structure) and may return a new `Compose`, which is then added as
//! the current `Scope`s' child. The compose function is called when the composable is first added, when one of the
//! scope's state changes, or when the parent composable "recomposes".
//!
//! **Some notable [`Compose`](prelude::Compose) implementations:**
//!
//! - `Fn(&mut Scope) -> impl Compose` - A function that takes a mutable reference to a `Scope` and returns a
//!   composable. Useful for simple composables that don't need any input.
//! - [`Spawn`](prelude::Spawn) - Spawns a new entity with the from a bundle.
//! - [`DynCompose`](prelude::DynCompose) - Allows for dynamic composables that "erase" their type definition.
//! - `Option<C>` - Composes `C` if the option is `Some`, otherwise does nothing.
//! - Tuples `(C0, .., C9)` - Compose multiple composables at once.
//! - `Vec<C>` - Compose any number of composables. This requires that the items implement the
//!   [`Key`](prelude::Key)-trait.
//!     - [`Keyed`](prelude::Keyed) - Implements the `Key`-trait and can be used to wrap any composable. The added
//!       advantage is that the type is "erased" so that composables of different types can be composed in the same
//!       `Vec`.
//! - `()` - Empty composable that does nothing. Implementing `Compose` for `()` lets us skip returning anything from
//!   the [`compose`](prelude::Compose::compose) function.
//!
//! **Bundle**
//! - The `Bundle`-trait does not implemented the [`Compose`](prelude::Compose)-trait, however it does implement the
//!   [`BundleExtension`](prelude::BundleExtension)-trait which lets us convert a `Bundle` into a
//!   [`Spawn`](prelude::Spawn)-composable very easily. Through `BundleExtension`, it also implements
//!   [`ModfiyFunctions`](prelude::ModifyFunctions)-trait which lets us use functions like
//!   [`children`](prelude::ModifyFunctions::children), [`observe`](prelude::ModifyFunctions::observe).
//!
//! # Compatibility with Bevy
//! | Bevy | recompose |
//! | ---- | --------- |
//! | 0.15 | 0.1-0.3   |
//!
//! # Motivation
//! Recompose is heavily inspired by the [actuate](https://docs.rs/actuate/) crate, which also provides a declarative
//! way to construct ECS structures. This crate strives to be more robust, safer and less error-prone than `actuate`.
//!
//! The goal of `recompose` is not necessarily to be the most performant solution, but rather one that is easy to use
//! and easy to understand.

pub mod prelude {
    pub use recompose_core::bundle_extension::*;
    pub use recompose_core::dyn_compose::*;
    pub use recompose_core::keyed::*;
    pub use recompose_core::modify::*;
    pub use recompose_core::scope::*;
    pub use recompose_core::spawn::*;
    pub use recompose_core::state::*;
    pub use recompose_core::*;
}
