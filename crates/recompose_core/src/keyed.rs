use crate::{dyn_compose::DynCompose, Compose, Key, Scope};

#[derive(Clone)]
pub struct Keyed {
    key: usize,
    compose: DynCompose,
}

impl Keyed {
    pub fn new<C: Compose + 'static>(key: usize, compose: C) -> Self {
        Self {
            key,
            compose: DynCompose::new(compose),
        }
    }
}

impl Compose for Keyed {
    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a {
        self.compose.compose(cx)
    }

    fn decompose(&self, cx: &mut Scope) {
        self.compose.compose(cx);
    }

    fn ignore_children(&self) -> bool {
        self.compose.ignore_children()
    }

    fn name(&self) -> String {
        String::from("KeyedCompose")
    }
}

impl Key for Keyed {
    fn key(&self) -> usize {
        self.key
    }
}
