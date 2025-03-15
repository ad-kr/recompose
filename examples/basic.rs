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
    commands.spawn((Root::new(squares), Node::default()));
}

// `Fn(&mut Scope) -> impl Compose` implements Compose, so we can use functions for simple composables.
fn squares<'a>(cx: &mut Scope) -> impl Compose + use<'a> {
    let count = cx.use_state(42);

    Node {
        display: Display::Flex,
        column_gap: Val::Px(8.0),
        ..default()
    }
    .children((
        Square(Srgba::RED.into()),
        Square(Srgba::GREEN.into()),
        Square(Srgba::BLUE.into()),
        Text::new(count.to_string()).to_compose(),
    ))
}

#[derive(Clone)]
struct Square(Color);

// For more complex composables with input, we can implement Compose directly on a struct.
impl Compose for Square {
    fn compose<'a>(&self, _: &mut Scope) -> impl Compose + 'a {
        (
            Node {
                width: Val::Px(32.0),
                height: Val::Px(32.0),
                ..default()
            },
            BackgroundColor(self.0),
        )
            .to_compose()
    }
}
