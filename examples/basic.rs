use bevy::prelude::*;
use recompose::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(MeshPickingPlugin)
        .add_plugins(RecomposePlugin)
        .add_systems(Startup, spawn_camera)
        .add_systems(Update, despawn_roots)
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera3d::default());

    commands.spawn((
        Root::new(counter),
        Node {
            display: Display::Flex,
            column_gap: Val::Px(8.0),
            ..default()
        },
    ));
}

fn despawn_roots(mut commands: Commands, roots: Query<Entity, With<Root>>, time: Res<Time>) {
    if time.elapsed_secs() < 5.0 {
        return;
    }

    for entity in roots.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

fn counter(cx: &mut Scope) -> impl Compose {
    let count = cx.use_state(0);

    let count_string = count.to_string();
    let count_clone = count.clone();

    (
        Node {
            display: Display::Flex,
            column_gap: Val::Px(8.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(Srgba::WHITE.into()),
    )
        .children(vec![
            (
                Node {
                    width: Val::Px(32.0),
                    height: Val::Px(32.0),
                    ..default()
                },
                BackgroundColor(Srgba::RED.into()),
            )
                .observe(move |_: Trigger<Pointer<Click>>, mut state: SetState| {
                    state.set_fn(&count, |c| c - 100);
                })
                .keyed(0),
            (
                Text::new(count_string),
                BackgroundColor(Srgba::GREEN.into()),
            )
                .keyed(1),
            (
                Node {
                    width: Val::Px(32.0),
                    height: Val::Px(32.0),
                    ..default()
                },
                BackgroundColor(Srgba::BLUE.into()),
            )
                .observe(move |_: Trigger<Pointer<Click>>, mut state: SetState| {
                    state.set_fn(&count_clone, |c| c + 100);
                })
                .keyed(2),
        ])
}
