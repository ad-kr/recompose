use bevy::{
    prelude::*,
    render::{
        settings::{Backends, RenderCreation, WgpuSettings},
        RenderPlugin,
    },
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use islands_ui_core::{Compose, IslandsUiPlugin, Key, Root, Scope, SetState};

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
}

pub struct Counter;

impl Compose for Counter {
    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a {
        let count = cx.use_state(0);

        cx.set_state(&count, *count + 1);

        vec![
            Keyed {
                key: 0,
                count: *count,
                count_threshold: 100,
                color: Srgba::RED.into(),
            },
            Keyed {
                key: 1,
                count: *count,
                count_threshold: 400,
                color: Srgba::GREEN.into(),
            },
            Keyed {
                key: 2,
                count: *count,
                count_threshold: 200,
                color: Srgba::BLUE.into(),
            },
        ]
    }
}

#[derive(Clone)]
pub struct Keyed {
    key: usize,
    count: i32,
    count_threshold: i32,
    color: Color,
}

impl Compose for Keyed {
    fn compose<'a>(&self, _: &mut Scope) -> impl Compose + 'a {
        if self.count > self.count_threshold {
            Some(Rect { color: self.color })
        } else {
            None
        }
    }
}

impl Key for Keyed {
    fn key(&self) -> usize {
        self.key
    }
}

#[derive(Clone)]
pub struct Rect {
    color: Color,
}

impl Compose for Rect {
    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a {
        let entity = cx.use_state(None);
        let color = self.color;

        cx.use_system_once(move |mut state: SetState, mut commands: Commands| {
            let mut e = commands.spawn_empty();

            state.set(&entity, Some(e.id()));

            e.try_insert((
                Name::new("Rect"),
                Node {
                    width: Val::Px(64.0),
                    height: Val::Px(64.0),
                    margin: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(color),
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
