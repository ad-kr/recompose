use bevy::prelude::*;
use recompose::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RecomposePlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((Root::new(squares_or_circles), Node::default()));
}

fn squares_or_circles(cx: &mut Scope) -> impl Compose {
    let count = cx.use_state(0);

    let is_circle = (*count / 50) % 2 == 0;

    cx.set_state(&count, *count + 1);

    if is_circle {
        DynCompose::new(Circle)
    } else {
        DynCompose::new(Square)
    }
}

#[derive(Clone)]
struct Square;

impl Compose for Square {
    fn compose<'a>(&self, _: &mut Scope) -> impl Compose + 'a {
        (
            Node {
                width: Val::Px(32.0),
                height: Val::Px(32.0),
                ..default()
            },
            BackgroundColor(Srgba::RED.into()),
        )
            .to_compose()
    }
}

#[derive(Clone)]
struct Circle;

impl Compose for Circle {
    fn compose<'a>(&self, _: &mut Scope) -> impl Compose + 'a {
        (
            Node {
                width: Val::Px(32.0),
                height: Val::Px(32.0),
                ..default()
            },
            BackgroundColor(Srgba::GREEN.into()),
            BorderRadius::all(Val::Px(16.0)),
        )
            .to_compose()
    }
}
