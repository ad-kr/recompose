//! This example demonstrates how we can modify the state outside of the
//! [`compose`](recompose::prelude::Compose::compose) function. This is useful when we want a system or observer to
//! modify some aspect of the composable.

use bevy::prelude::*;
use recompose::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RecomposePlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, add_elapsed)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((Root::new(timer), Node::default()));
}

// We use `TypedStateId` which holds both our manually created StateId, as well as the type generic of out desired
// value.
const ELAPSED_ID: TypedStateId<f64> = TypedStateId::new(0);

fn timer(cx: &mut Scope) -> impl Compose {
    let elapsed_secs = cx.use_state_with_id(ELAPSED_ID, 0.0);

    (
        Node {
            padding: UiRect::all(Val::Px(16.0)),
            ..default()
        },
        BackgroundColor(Color::WHITE),
    )
        .children(
            (
                Text::new(format!("{} seconds since startup", *elapsed_secs)),
                TextColor(Color::BLACK),
            )
                .to_compose(),
        )
}

fn add_elapsed(time: Res<Time<Real>>, mut state: SetState) {
    let elapsed = time.elapsed_secs_f64();
    state.set_with_id(ELAPSED_ID, elapsed.round());
}
