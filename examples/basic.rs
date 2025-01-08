use bevy::{
    prelude::*,
    render::{
        settings::{Backends, RenderCreation, WgpuSettings},
        RenderPlugin,
    },
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use islands_ui_core::{
    bundle_compose::BundleCompose, scope::Scope, state::SetState, Compose, IslandsUiPlugin, Root,
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

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera3d::default());

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
}
