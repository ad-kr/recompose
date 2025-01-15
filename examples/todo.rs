use std::hash::{DefaultHasher, Hash, Hasher};

use bevy::{
    color::palettes::tailwind,
    input::{self, keyboard::KeyboardInput},
    prelude::*,
};
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
    commands.spawn((
        Root::new(todo),
        Node {
            display: Display::Flex,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            width: Val::Vw(100.0),
            height: Val::Vh(100.0),
            ..default()
        },
    ));
}

fn todo(cx: &mut Scope) -> impl Compose {
    let todos = cx.use_state(Vec::<(usize, String)>::from([
        (0, "Buy milk".to_string()),
        (1, "Clean room".to_string()),
        (2, "Do homework".to_string()),
    ]));

    let input = cx.use_state(String::from("Hello world"));

    (
        Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            width: Val::Px(300.0),
            padding: UiRect::all(Val::Px(16.0)),
            row_gap: Val::Px(16.0),
            ..default()
        },
        BackgroundColor(Srgba::WHITE.into()),
        BorderRadius::all(Val::Px(8.0)),
    )
        .children((
            (
                Text::new("Todo list"),
                TextColor(Srgba::gray(0.2).into()),
                TextFont::from_font_size(20.0),
            )
                .to_compose(),
            todos
                .iter()
                .map(|(id, label)| Todo {
                    id: *id,
                    label: label.to_string(),
                    all_todos: todos.get_typed_id(),
                })
                .collect::<Vec<_>>(),
            Node {
                display: Display::Flex,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(16.0),
                ..default()
            }
            .children((
                InputField {
                    value: (*input).clone(),
                    input_ref: input.get_typed_id(),
                },
                Button {
                    label: "Add".to_string(),
                    color: tailwind::GREEN_300.into(),
                    hover_color: tailwind::GREEN_400.into(),
                    modifier: Modifier::default(),
                }
                .observe(move |_: Trigger<Pointer<Click>>, mut state: SetState| {
                    let input_value = input.to_string();

                    if input_value.is_empty() {
                        return;
                    }

                    // This method of generating the Id assumes that the "todo" value is unique!
                    let mut hasher = DefaultHasher::new();
                    input_value.hash(&mut hasher);
                    let id = hasher.finish() as usize;

                    state.modify(&todos, move |todos| {
                        let mut todos = todos.clone();
                        todos.push((id, input_value.clone()));
                        todos
                    });

                    state.set(input.clone(), "".to_string());
                }),
            )),
        ))
}

#[derive(Clone)]
struct Todo {
    id: usize,
    label: String,
    all_todos: TypedStateId<Vec<(usize, String)>>,
}

impl Key for Todo {
    fn key(&self) -> usize {
        self.id
    }
}

impl Compose for Todo {
    fn compose<'a>(&self, _: &mut Scope) -> impl Compose + 'a {
        let all_todos = self.all_todos;
        let id = self.id;

        Node {
            display: Display::Flex,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            column_gap: Val::Px(32.0),
            ..default()
        }
        .children((
            (
                Node {
                    max_width: Val::Px(150.0),
                    ..default()
                },
                Text::new(self.label.clone()),
                TextColor(Srgba::gray(0.4).into()),
                TextFont::from_font_size(16.0),
            )
                .to_compose(),
            Button {
                label: "Remove".to_string(),
                color: tailwind::RED_300.into(),
                hover_color: tailwind::RED_400.into(),
                modifier: Modifier::default(),
            }
            .observe(move |_: Trigger<Pointer<Click>>, mut state: SetState| {
                state.modify_with_id(all_todos, move |todos| {
                    let mut todos = todos.clone();
                    todos.retain(|(todo_id, _)| *todo_id != id);
                    todos
                });
            }),
        ))
    }
}

#[derive(Clone)]
struct Button {
    label: String,
    color: Color,
    hover_color: Color,
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

        let background_color = match *is_hovered {
            true => self.hover_color,
            false => self.color,
        };

        (
            Node {
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(background_color),
            BorderRadius::all(Val::Px(4.0)),
        )
            .children(
                (
                    Text::new(self.label.clone()),
                    TextFont::from_font_size(14.0),
                )
                    .to_compose(),
            )
            .bind_hover(is_hovered)
            .use_modifier(&self.modifier)
    }
}

#[derive(Clone)]
struct InputField {
    value: String,
    input_ref: TypedStateId<String>,
}

// This is a very rudimentary (and bad!) implementation of an input field, but it works for this example.
impl Compose for InputField {
    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a {
        let is_focused = cx.use_state(false);
        let is_focused_ref = is_focused.to_ref();
        let input_ref = self.input_ref;

        if *is_focused {
            cx.run_system(
                move |mut key_events: EventReader<KeyboardInput>,
                      just_pressed: Res<ButtonInput<KeyCode>>,
                      mut state: SetState,
                      mouse_input: Res<ButtonInput<MouseButton>>| {
                    // User clicks outside
                    if mouse_input.just_pressed(MouseButton::Left) {
                        state.set(is_focused_ref, false);
                    }

                    let events = key_events.read().cloned().collect::<Vec<_>>();
                    let was_just_pressed = just_pressed.get_just_pressed().len() > 0;

                    state.modify_with_id(input_ref, move |val| {
                        let mut new_string = val.clone();

                        if was_just_pressed {
                            return new_string;
                        }

                        for e in &events {
                            if e.repeat || !e.state.is_pressed() {
                                continue;
                            }
                            match &e.logical_key {
                                input::keyboard::Key::Character(c) => {
                                    new_string.push_str(c.as_str());
                                }
                                input::keyboard::Key::Space => {
                                    new_string.push(' ');
                                }
                                input::keyboard::Key::Backspace => {
                                    new_string.pop();
                                }
                                _ => (),
                            }
                        }

                        new_string
                    });
                },
            );
        }

        (
            Node {
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                max_width: Val::Px(150.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(tailwind::SLATE_200.into()),
            BorderRadius::all(Val::Px(8.0)),
            BorderColor(if *is_focused {
                tailwind::SLATE_500.into()
            } else {
                Color::NONE
            }),
        )
            .children(
                (
                    Text::new(format!(
                        "{}{}",
                        self.value.clone(),
                        if *is_focused { "|" } else { " " }
                    )),
                    TextColor(tailwind::SLATE_900.into()),
                    TextFont::from_font_size(14.0),
                )
                    .to_compose(),
            )
            .observe(move |_: Trigger<Pointer<Click>>, mut state: SetState| {
                state.set(&is_focused, !*is_focused);
            })
    }
}
