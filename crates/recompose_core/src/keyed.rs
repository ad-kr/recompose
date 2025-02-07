use crate::{dyn_compose::DynCompose, Compose, Key, Scope};
use std::hash::Hash;

#[derive(Clone)]
pub struct Keyed<H: Hash + Send + Sync> {
    key: H,
    compose: DynCompose,
}

impl<H: Hash + Send + Sync> Keyed<H> {
    pub fn new<C: Compose + 'static>(key: H, compose: C) -> Self {
        Self {
            key,
            compose: DynCompose::new(compose),
        }
    }
}

impl<H: Hash + Send + Sync> Compose for Keyed<H> {
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

impl<H: Hash + Send + Sync + Clone> Key for Keyed<H> {
    fn key(&self) -> &impl Hash {
        &self.key
    }
}
