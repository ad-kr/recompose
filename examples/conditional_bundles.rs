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

fn squares<'a>(cx: &mut Scope) -> impl Compose + use<'a> {
    let count = cx.use_state(0);

    cx.set_state(&count, *count + 1);

    Node {
        display: Display::Flex,
        column_gap: Val::Px(8.0),
        ..default()
    }
    .children((
        Square {
            has_border: true,
            color: Srgba::RED.into(),
        },
        Square {
            has_border: false,
            color: Srgba::GREEN.into(),
        },
        Square {
            has_border: (*count / 50) % 2 == 0,
            color: Srgba::BLUE.into(),
        },
        Text::new(count.to_string()).to_compose(),
    ))
}

#[derive(Clone)]
struct Square {
    has_border: bool,
    color: Color,
}

impl Compose for Square {
    fn compose<'a>(&self, _: &mut Scope) -> impl Compose + 'a {
        (
            Node {
                width: Val::Px(32.0),
                height: Val::Px(32.0),
                border: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(self.color),
        )
            // `.with_bundle` lets us conditionally add a bundle to the spawned entity. Note that the main bundle
            // overrides the conditional bundles, so if we added `BorderColor` to the main bundle, the conditional
            // border would be ignored.
            .with_bundle_if(self.has_border, BorderColor(Srgba::WHITE.into()))
    }
}
