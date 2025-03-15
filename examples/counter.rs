use bevy::{color::palettes::tailwind, prelude::*};
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
    commands.spawn((Root::new(counter), Node::default()));
}

fn counter<'a>(cx: &mut Scope) -> impl Compose + use<'a> {
    let count = cx.use_state(0);

    Node {
        display: Display::Flex,
        column_gap: Val::Px(8.0),
        ..default()
    }
    .children((
        Text::new(count.to_string()).to_compose(),
        Button {
            label: "Increment",
            modifier: Modifier::default(),
        }
        .observe(move |_: Trigger<Pointer<Click>>, mut state: SetState| {
            state.set(&count, *count + 1)
        }),
    ))
}

#[derive(Clone)]
struct Button {
    label: &'static str,
    modifier: Modifier,
}

impl Modify for Button {
    fn modifier(&mut self) -> &mut Modifier {
        &mut self.modifier
    }
}

impl Compose for Button {
    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a {
        let is_hovered = cx.use_state(false);

        let bg_color = match *is_hovered {
            true => tailwind::SLATE_400.into(),
            false => tailwind::SLATE_300.into(),
        };

        (
            Node {
                display: Display::Flex,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(bg_color),
            BorderRadius::all(Val::Px(4.0)),
        )
            .children((Text::new(self.label), TextColor(tailwind::SLATE_900.into())).to_compose())
            .use_modifier(&self.modifier)
            .bind_hover(is_hovered)
    }
}
