use bevy::{
    prelude::*,
    render::{
        settings::{Backends, RenderCreation, WgpuSettings},
        RenderPlugin,
    },
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use islands_ui_core::{Compose, DynCompose, IslandsUiPlugin, Root, Scope, SetState};

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

    commands.spawn(Root::new(Counter));

    commands.spawn(Name::new("Sup dawg"));
}

pub struct Counter;

impl Compose for Counter {
    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a {
        let count = cx.use_state(0);

        // println!("count: {}", *count);

        cx.set_state(&count, *count + 1);

        if *count >= 200 && *count < 400 {
            // DynCompose::new(RedRect { count: *count })
            Some(GreenRect { count: *count })
        } else {
            // DynCompose::new(GreenRect { count: *count })
            None
        }
    }
}

#[derive(Clone)]
pub struct RedRect {
    count: i32,
}

impl Compose for RedRect {
    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a {
        let entity = cx.use_state(None);

        println!("red count: {}", self.count);

        cx.use_system_once(move |mut state: SetState, mut commands: Commands| {
            let mut e = commands.spawn_empty();

            state.set(&entity, Some(e.id()));

            e.try_insert((
                Name::new("RedRect"),
                Node {
                    width: Val::Px(128.0),
                    height: Val::Px(128.0),
                    margin: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Srgba::RED.into()),
            ));
        });
    }

    fn decompose(&self, cx: &mut Scope) {
        let entity = cx.get_state_by_index::<Option<Entity>>(0);

        dbg!("red decompose");

        if let Some(entity) = *entity {
            cx.use_system_once(move |mut commands: Commands| {
                commands.entity(entity).despawn_recursive();
            });
        }
    }
}

#[derive(Clone)]
pub struct GreenRect {
    count: i32,
}

impl Compose for GreenRect {
    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a {
        let entity = cx.use_state(None);

        println!("green count: {}", self.count);

        cx.use_system_once(move |mut state: SetState, mut commands: Commands| {
            let mut e = commands.spawn_empty();

            state.set(&entity, Some(e.id()));

            e.try_insert((
                Name::new("GreenRect"),
                Node {
                    width: Val::Px(128.0),
                    height: Val::Px(128.0),
                    margin: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Srgba::GREEN.into()),
            ));
        });
    }

    fn decompose(&self, cx: &mut Scope) {
        let entity = cx.get_state_by_index::<Option<Entity>>(0);

        dbg!("green decompose");

        if let Some(entity) = *entity {
            cx.use_system_once(move |mut commands: Commands| {
                commands.entity(entity).despawn_recursive();
            });
        }
    }
}
