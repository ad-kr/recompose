use bevy::{
    prelude::*,
    render::{
        settings::{Backends, RenderCreation, WgpuSettings},
        RenderPlugin,
    },
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use islands_ui_core::{
    scope::Scope, spawn::Spawn, state::SetState, Compose, IslandsUiPlugin, Root,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(RenderPlugin {
            render_creation: RenderCreation::Automatic(WgpuSettings {
                backends: Some(Backends::VULKAN),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(MeshPickingPlugin)
        .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(IslandsUiPlugin)
        .add_systems(Startup, spawn_camera)
        .run();
}

fn spawn_camera(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn(Camera3d::default());

    let cube = Cuboid::new(1.0, 1.0, 1.0);
    let mesh = meshes.add(cube.mesh());

    let material = materials.add(StandardMaterial::default());

    commands.spawn((
        Root::new(Counter),
        Node {
            display: Display::Flex,
            column_gap: Val::Px(8.0),
            ..default()
        },
    ));
}

pub struct Counter;

impl Compose for Counter {
    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a {
        let count = cx.use_state(0);

        // cx.set_state(&count, *count + 1);
        let count_string = count.to_string();
        let count_clone = count.clone();

        Spawn::new((
            Node {
                display: Display::Flex,
                column_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Srgba::WHITE.into()),
        ))
        .children(vec![
            Spawn::new((
                Node {
                    width: Val::Px(32.0),
                    height: Val::Px(32.0),
                    ..default()
                },
                BackgroundColor(Srgba::RED.into()),
            ))
            .observe(move |_: Trigger<Pointer<Click>>, mut state: SetState| {
                state.set_fn(&count, |c| c - 100);
            })
            .keyed(0),
            Spawn::new((
                Text::new(count_string),
                BackgroundColor(Srgba::GREEN.into()),
            ))
            .keyed(1),
            Spawn::new((
                Node {
                    width: Val::Px(32.0),
                    height: Val::Px(32.0),
                    ..default()
                },
                BackgroundColor(Srgba::BLUE.into()),
            ))
            .observe(move |_: Trigger<Pointer<Click>>, mut state: SetState| {
                state.set_fn(&count_clone, |c| c + 100);
            })
            .keyed(2),
        ])
    }
}
