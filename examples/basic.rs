use bevy::{
    prelude::*,
    render::{
        settings::{Backends, RenderCreation, WgpuSettings},
        RenderPlugin,
    },
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use islands_ui_core::{Compose, IslandsUiPlugin, Root, Scope};

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

    commands.spawn(Root::new(First));

    commands.spawn(Name::new("Sup dawg"));
}

#[derive(Clone)]
pub struct First;

impl Compose for First {
    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a {
        // let count = cx.use_state(0);

        // println!("first count: {}", *count);

        // cx.set_state(&count, **count + 1);

        Second
    }
}
#[derive(Clone)]
pub struct Second;

impl Compose for Second {
    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a {
        let count = cx.use_state(0);

        cx.use_system(|q: Query<&Name>| {
            println!("Querying names, len: {}", q.iter().len());
            for name in q.iter() {
                println!("Name: {:?}", name);
            }
        });

        cx.set_state(&count, *count + 1);
    }
}
